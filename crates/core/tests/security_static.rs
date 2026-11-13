//! 정적 보안 검사 — 소스 코드 수준에서 보안 원칙이 지켜지는지 확인한다.
use std::path::{Path, PathBuf};

fn rust_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            if !matches!(p.file_name().and_then(|n| n.to_str()), Some("target" | "node_modules" | "gen")) {
                rust_sources(&p, out);
            }
        } else if p.extension().map(|x| x == "rs").unwrap_or(false) {
            out.push(p);
        }
    }
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap().to_path_buf()
}

fn app_sources() -> Vec<(PathBuf, String)> {
    let root = workspace_root();
    let mut files = Vec::new();
    rust_sources(&root.join("crates/core/src"), &mut files);
    if root.join("src-tauri/src").exists() {
        rust_sources(&root.join("src-tauri/src"), &mut files);
    }
    files.into_iter().map(|p| { let s = std::fs::read_to_string(&p).unwrap(); (p, s) }).collect()
}

#[test]
fn http_clients_are_created_only_in_netguard() {
    for (path, src) in app_sources() {
        let name = path.to_string_lossy().replace('\\', "/");
        if name.ends_with("security/netguard.rs") {
            continue;
        }
        for needle in ["reqwest::Client::", "Client::builder", "Client::new", "reqwest::get", "reqwest::blocking", "ureq::", "hyper::Client", "TcpStream::connect(\""] {
            assert!(!src.contains(needle), "{name}: 중앙 보안 모듈 밖에서 네트워크 클라이언트를 만들고 있습니다 ({needle})");
        }
    }
}

#[test]
fn no_external_ai_or_telemetry_endpoints_in_source() {
    let banned = [
        "api.openai.com", "api.anthropic.com", "generativelanguage.googleapis.com", "api.mistral.ai", "api.cohere",
        "openrouter.ai", "sentry.io", "segment.io", "google-analytics", "posthog", "mixpanel", "amplitude.com", "datadoghq",
    ];
    for (path, src) in app_sources() {
        let name = path.to_string_lossy().replace('\\', "/");
        for b in banned {
            // netguard.rs의 테스트는 "차단되는 주소" 예시로 일부 도메인을 포함한다.
            if name.ends_with("security/netguard.rs") && (b == "api.openai.com" || b == "api.anthropic.com") {
                continue;
            }
            assert!(!src.contains(b), "{name}: 외부 서비스 주소가 포함되어 있습니다 ({b})");
        }
    }
    // 프런트엔드 소스도 검사
    let fe = workspace_root().join("src");
    if fe.exists() {
        let mut stack = vec![fe];
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(d).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else if matches!(p.extension().and_then(|x| x.to_str()), Some("ts" | "tsx" | "css" | "html")) {
                    let s = std::fs::read_to_string(&p).unwrap();
                    for b in banned {
                        assert!(!s.contains(b), "{}: 외부 서비스 주소 ({b})", p.display());
                    }
                    assert!(!s.contains("fetch(\"http") && !s.contains("fetch('http") && !s.contains("XMLHttpRequest") && !s.contains("WebSocket("), "{}: 웹뷰에서 직접 네트워크를 사용합니다", p.display());
                }
            }
        }
    }
}

#[test]
fn engine_is_always_started_local_only() {
    let src = std::fs::read_to_string(workspace_root().join("crates/core/src/runtime/engine.rs")).unwrap();
    assert!(src.contains(r#".env("OLLAMA_NO_CLOUD", "1")"#));
    assert!(src.contains(r#"cmd.env("OLLAMA_HOST", format!("127.0.0.1:{port}"))"#));
    assert!(src.contains("env_remove"), "사용자 환경의 OLLAMA_* 변수를 제거해야 함");
    assert!(src.contains("Stdio::null()"), "엔진 로그를 저장하지 않아야 함");
}

#[test]
fn only_allowed_engine_endpoints_are_used() {
    let src = std::fs::read_to_string(workspace_root().join("crates/core/src/runtime/ollama.rs")).unwrap();
    let code = src.split("#[cfg(test)]").next().unwrap();
    for banned in ["api/push", "api/experimental", "api/me\"", "api/signout", "api/generate", "api/embed", "api/create", "api/copy", "v1/"] {
        assert!(!code.lines().filter(|l| !l.trim_start().starts_with("//")).any(|l| l.contains(banned)), "허용되지 않은 엔진 기능 사용: {banned}");
    }
}

#[test]
fn tauri_config_is_locked_down() {
    let p = workspace_root().join("src-tauri/tauri.conf.json");
    if !p.exists() {
        return;
    }
    let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
    let csp = v["app"]["security"]["csp"].as_str().expect("CSP가 설정되어야 함");
    assert!(csp.contains("default-src 'self'"));
    for bad in ["https:", "http://", "*", "unsafe-eval"] {
        let cleaned = csp.replace("http://ipc.localhost", "");
        assert!(!cleaned.contains(bad), "CSP에 허용 범위가 넓은 항목이 있습니다: {bad}");
    }
    // 원격 URL을 여는 창이 없어야 한다
    for w in v["app"]["windows"].as_array().unwrap() {
        let url = w["url"].as_str().unwrap_or("index.html");
        assert!(!url.starts_with("http"), "원격 콘텐츠를 여는 창");
    }
    // 권한 파일: shell/http/fs 플러그인 권한 금지
    let caps = workspace_root().join("src-tauri/capabilities");
    for e in std::fs::read_dir(caps).unwrap().flatten() {
        let s = std::fs::read_to_string(e.path()).unwrap();
        for bad in ["shell:", "http:", "fs:", "updater:", "opener:", "dialog:", "core:default", "core:window", "core:webview", "core:path"] {
            assert!(!s.contains(bad), "불필요한 권한: {bad}");
        }
    }
}

#[test]
fn webview_cannot_supply_file_paths() {
    // 화면(웹뷰)이 호출하는 Tauri 명령은 파일 경로를 인자로 받지 않는다.
    // 파일은 Rust 쪽 대화상자(secure.rs)에서 교수님이 고른 것만 사용한다.
    let dir = workspace_root().join("src-tauri/src");
    if !dir.exists() {
        return;
    }
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let src = std::fs::read_to_string(e.path()).unwrap();
        let mut in_cmd = false;
        for line in src.lines() {
            let t = line.trim();
            if t.starts_with("#[tauri::command]") {
                in_cmd = true;
                continue;
            }
            if in_cmd && (t.contains("fn ") ) {
                for bad in ["path: String", "path: PathBuf", "path: &str", "dir: String", "file: String", "url: String"] {
                    assert!(!t.contains(bad), "{}: 명령이 경로/주소를 화면에서 받음: {t}", e.path().display());
                }
                in_cmd = false;
            }
        }
    }
}

#[test]
fn sqlite_never_spills_temp_files() {
    let src = std::fs::read_to_string(workspace_root().join("crates/core/src/storage.rs")).unwrap();
    assert!(src.contains("PRAGMA temp_store = MEMORY"));
    assert!(src.contains("PRAGMA secure_delete = ON"));
}
