//! 실제 Ollama 공식 배포 파일로 하는 통합 테스트.
//!
//! 인터넷으로 공식 엔진(수백 MB~1.5GB)을 내려받으므로 기본으로는 건너뛴다.
//! 실행: `PLA_REAL_ENGINE_DIR=/경로 cargo test -p pla-core --test real_engine -- --nocapture`
//! (같은 폴더를 다시 쓰면 이미 설치된 엔진을 재사용한다.)
use pla_core::error::AppError;
use pla_core::models::Catalog;
use pla_core::runtime::engine::{non_loopback_ips, verify_local_only, EngineManager};
use pla_core::runtime::ollama::OllamaRuntime;
use pla_core::runtime::{AiRuntime, ChatMessage, ChatRequest};
use pla_core::security::NetGuard;
use std::net::{SocketAddr, TcpStream};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

fn can_connect(addr: SocketAddr) -> bool {
    TcpStream::connect_timeout(&addr, Duration::from_millis(700)).is_ok()
}

#[tokio::test]
async fn real_ollama_install_start_local_only_and_cleanup() {
    let Ok(dir) = std::env::var("PLA_REAL_ENGINE_DIR") else {
        eprintln!("PLA_REAL_ENGINE_DIR 미설정 — 실제 엔진 테스트를 건너뜁니다");
        return;
    };
    // 위험한 사용자 전역 설정이 있어도 무시되어야 한다.
    std::env::set_var("OLLAMA_HOST", "0.0.0.0:11434");
    std::env::remove_var("OLLAMA_NO_CLOUD");

    let guard = Arc::new(NetGuard::new());
    let engine = EngineManager::new(PathBuf::from(&dir), guard.clone());

    // 1) 동의 없이는 설치 거부
    if !engine.is_installed() {
        let r = engine.install(|_| {}, CancellationToken::new()).await;
        assert!(matches!(r, Err(AppError::ConsentRequired(_))), "{r:?}");
    }
    // 2) 동의 후 공식 파일 내려받기 → SHA-256 확인 → 설치
    guard.set_download_consent(true);
    let mut last = 0u64;
    engine
        .install(
            |p| {
                if p.completed_bytes / 200_000_000 != last {
                    last = p.completed_bytes / 200_000_000;
                    eprintln!("{} {}MB / {}MB", p.stage, p.completed_bytes / 1_000_000, p.total_bytes / 1_000_000);
                }
            },
            CancellationToken::new(),
        )
        .await
        .expect("엔진 설치");
    assert!(engine.is_installed());
    guard.set_download_consent(false);

    // 2-1) 앱 모델 폴더에 Ollama 0.40.x 방식(manifests-v2 + blob)으로 모델을 둔다.
    //      실제 엔진과 앱의 "이미 받은 AI" 확인이 같은 모델을 보는지 검증한다.
    let models = engine.models_dir();
    let fake = make_v2_model(&models, "gemma3", "4b");
    let found = pla_core::runtime::local_models::discover(&models, &Catalog::builtin());
    assert!(found.iter().any(|m| m.name == "gemma3:4b"), "엔진 없이도 이미 받은 모델을 찾아야 함: {found:?}");

    // 3) 실행
    let port = engine.start().await.expect("엔진 시작");
    let rt = OllamaRuntime::new(port, guard.clone(), Catalog::builtin().allowed_ids());
    let h = rt.health().await.unwrap();
    eprintln!("engine version = {:?}, cloud_disabled = {:?}", h.version, h.cloud_disabled);
    assert!(h.running);
    assert_eq!(h.version.as_deref(), Some(engine.manifest().version.as_str()));
    assert_eq!(h.cloud_disabled, Some(true), "실제 Ollama가 클라우드 꺼짐을 보고해야 함");

    // 4) 루프백 전용 (부모의 OLLAMA_HOST=0.0.0.0 무시)
    assert!(can_connect(SocketAddr::from(([127, 0, 0, 1], port))));
    for ip in non_loopback_ips() {
        assert!(!can_connect(SocketAddr::new(ip, port)), "외부 주소 {ip}:{port} 로 접속되면 안 됨");
    }
    let report = verify_local_only(&rt, port, None).await;
    eprintln!("{:?}", report.details);
    assert!(report.cloud_disabled && report.loopback_only);

    // 5) 실제 엔진도 같은 모델을 본다
    let listed = rt.list_models().await.unwrap();
    eprintln!("engine sees: {:?}", listed.iter().map(|m| &m.name).collect::<Vec<_>>());
    assert!(listed.iter().any(|m| m.name == "gemma3:4b" && !m.is_remote), "실제 엔진이 v2 저장 구조의 모델을 봐야 함");
    let _ = fake;

    // 6) 클라우드 모델은 엔진에 보내기 전에 차단
    let e = rt.complete(ChatRequest { model: "gpt-oss:120b-cloud".into(), messages: vec![ChatMessage::user("x")], num_ctx: None, temperature: None }, CancellationToken::new()).await;
    assert!(matches!(e, Err(AppError::Blocked(_))));
    // 7) 엔진 자체의 클라우드 프록시 경로도 꺼져 있는지 (앱은 사용하지 않지만 엔진 설정 확인용)
    let out = tokio::process::Command::new("curl")
        .args(["-s", "--noproxy", "*", "-X", "POST", "-d", r#"{"model":"gpt-oss:120b-cloud","messages":[{"role":"user","content":"x"}],"stream":false}"#, &format!("http://127.0.0.1:{port}/api/chat")])
        .output()
        .await
        .unwrap();
    let body = String::from_utf8_lossy(&out.stdout).to_lowercase();
    eprintln!("cloud chat via engine → {body}");
    assert!(body.contains("error"), "클라우드 모델 요청은 엔진에서도 실패해야 함");

    // 8) 동의 없이 모델 내려받기 거부
    let r = rt.pull_model("gemma3:1b", Box::new(|_| {}), CancellationToken::new()).await;
    assert!(matches!(r, Err(AppError::ConsentRequired(_))));

    // 9) 종료 후 프로세스·포트 정리
    let pid = engine.pid().await.unwrap();
    engine.stop().await;
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(!can_connect(SocketAddr::from(([127, 0, 0, 1], port))));
    let alive = std::fs::read_to_string(format!("/proc/{pid}/stat")).map(|s| !s.contains(") Z ")).unwrap_or(false);
    assert!(!alive, "엔진 프로세스가 남아 있음");
}

/// Ollama 0.40.x가 모델을 받았을 때와 같은 저장 구조를 만든다:
/// blobs/sha256-<해시> 에 config·가중치·매니페스트를 두고, manifests-v2/ollama.com/library/<모델>/<태그> 가 매니페스트 blob을 가리킨다.
fn make_v2_model(root: &std::path::Path, model: &str, tag: &str) -> Vec<std::path::PathBuf> {
    use sha2::{Digest, Sha256};
    let blobs = root.join("blobs");
    std::fs::create_dir_all(&blobs).unwrap();
    let put = |data: &[u8]| {
        let hex = hex::encode(Sha256::digest(data));
        let p = blobs.join(format!("sha256-{hex}"));
        std::fs::write(&p, data).unwrap();
        (format!("sha256:{hex}"), data.len(), p)
    };
    let (cd, cs, cp) = put(br#"{"model_format":"gguf","model_family":"gemma3","model_type":"4.3B","file_type":"Q4_K_M"}"#);
    let (ld, ls, lp) = put(b"not-a-real-gguf-test-weights");
    let manifest = serde_json::json!({"schemaVersion": 2, "mediaType": "application/vnd.docker.distribution.manifest.v2+json",
        "config": {"mediaType": "application/vnd.docker.container.image.v1+json", "digest": cd, "size": cs},
        "layers": [{"mediaType": "application/vnd.ollama.image.model", "digest": ld, "size": ls}]}).to_string();
    let (_md, _ms, mp) = put(manifest.as_bytes());
    let dir = root.join("manifests-v2/ollama.com/library").join(model);
    std::fs::create_dir_all(&dir).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(std::path::Path::new("../../../../blobs").join(mp.file_name().unwrap()), dir.join(tag)).unwrap();
    #[cfg(not(unix))]
    std::fs::copy(&mp, dir.join(tag)).unwrap();
    vec![cp, lp, mp]
}
