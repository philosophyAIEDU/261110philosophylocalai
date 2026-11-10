//! 중앙 네트워크 통제 모듈.
//!
//! 이 앱의 **모든** HTTP 클라이언트는 이 파일에서만 만들어진다
//! (`tests/security_static.rs`가 소스 전체를 검사해 강제한다).
//!
//! 허용되는 통신은 세 가지뿐이다.
//!
//! | 목적 | 대상 | 조건 |
//! |---|---|---|
//! | `LocalInference` | `http://127.0.0.1:<port>` | 항상 허용 (루프백 전용, 프록시 미사용) |
//! | `EngineDownload` | 고정된 GitHub 릴리스 호스트 (HTTPS) | 사용자의 명시적 동의 필요 |
//! | `ModelDownload`  | 로컬 엔진에 "카탈로그에 있는 모델" 내려받기 지시 | 사용자의 명시적 동의 필요 |
//!
//! 사용자 문서·질문·응답은 `LocalInference` 경로로만 흐른다.
use crate::error::{AppError, AppResult};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;
use url::Url;

/// AI 실행 엔진(Ollama 공식 배포 파일)을 내려받을 때 허용되는 호스트.
/// GitHub 릴리스는 자산 CDN으로 리다이렉트되므로 해당 호스트도 포함한다.
pub const ENGINE_DOWNLOAD_HOSTS: &[&str] = &[
    "github.com",
    "objects.githubusercontent.com",
    "release-assets.githubusercontent.com",
];

/// 모델 내려받기는 엔진(Ollama) 프로세스가 직접 수행한다. 표시·감사 목적의 호스트 이름.
pub const MODEL_REGISTRY_HOST: &str = "registry.ollama.ai";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Purpose {
    LocalInference,
    EngineDownload,
    ModelDownload,
}

/// 네트워크 감사 기록. 메타데이터만 담는다 (본문·문서 내용 없음).
#[derive(Debug, Clone, Serialize)]
pub struct AuditEntry {
    pub at: String,
    pub purpose: Purpose,
    pub host: String,
    /// 예: "엔진 v0.40.16 내려받기", "모델 gemma3:4b 내려받기"
    pub detail: String,
    pub allowed: bool,
}

pub struct NetGuard {
    download_consent: AtomicBool,
    audit: Mutex<Vec<AuditEntry>>,
    loopback: reqwest::Client,
}

impl Default for NetGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl NetGuard {
    pub fn new() -> Self {
        // 루프백 전용 클라이언트: 시스템 프록시를 쓰지 않고, 리다이렉트를 따르지 않는다.
        // (프록시를 쓰면 로컬 요청 본문이 프록시 서버로 나갈 수 있다.)
        let loopback = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(5))
            .build()
            .expect("loopback client");
        Self { download_consent: AtomicBool::new(false), audit: Mutex::new(Vec::new()), loopback }
    }

    /// 사용자가 "모델/엔진 내려받기를 위한 인터넷 사용"에 동의했는지 설정한다.
    pub fn set_download_consent(&self, v: bool) {
        self.download_consent.store(v, Ordering::SeqCst);
    }

    pub fn download_consent(&self) -> bool {
        self.download_consent.load(Ordering::SeqCst)
    }

    pub fn audit_log(&self) -> Vec<AuditEntry> {
        self.audit.lock().unwrap().clone()
    }

    fn record(&self, purpose: Purpose, host: &str, detail: &str, allowed: bool) {
        // 로컬 추론 요청은 건수가 많고 외부 통신이 아니므로 기록하지 않는다.
        if purpose == Purpose::LocalInference && allowed {
            return;
        }
        let mut a = self.audit.lock().unwrap();
        a.push(AuditEntry {
            at: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            purpose,
            host: host.to_string(),
            detail: detail.to_string(),
            allowed,
        });
        let len = a.len();
        if len > 500 {
            a.drain(0..len - 500);
        }
    }

    /// URL이 주어진 목적에 허용되는지 검사한다. 실패 시 감사 기록을 남긴다.
    pub fn check(&self, purpose: Purpose, url: &Url, detail: &str) -> AppResult<()> {
        let host = url.host_str().unwrap_or("").to_ascii_lowercase();
        let result = match purpose {
            Purpose::LocalInference => {
                if url.scheme() == "http" && is_loopback_host(url) && url.username().is_empty() {
                    Ok(())
                } else {
                    Err(AppError::Blocked("AI 추론 요청은 이 컴퓨터(127.0.0.1) 안에서만 허용됩니다".into()))
                }
            }
            Purpose::EngineDownload => {
                if !self.download_consent() {
                    Err(AppError::ConsentRequired("AI 실행 엔진을 내려받으려면 인터넷 사용에 동의해 주세요".into()))
                } else if url.scheme() != "https" || !ENGINE_DOWNLOAD_HOSTS.contains(&host.as_str()) {
                    Err(AppError::Blocked("허용되지 않은 주소에서의 내려받기입니다".into()))
                } else {
                    Ok(())
                }
            }
            Purpose::ModelDownload => Err(AppError::Blocked(
                "모델 내려받기는 authorize_model_pull을 통해서만 허용됩니다".into(),
            )),
        };
        self.record(purpose, &host, detail, result.is_ok());
        result
    }

    /// 로컬 엔진에 모델 내려받기를 지시해도 되는지 검사한다.
    /// `allowed_models`는 앱 카탈로그에 등록된 모델 이름 목록이다.
    pub fn authorize_model_pull(&self, model: &str, allowed_models: &[String]) -> AppResult<()> {
        let result = if !self.download_consent() {
            Err(AppError::ConsentRequired("AI 모델을 내려받으려면 인터넷 사용에 동의해 주세요".into()))
        } else if !allowed_models.iter().any(|m| m == model) || is_cloud_model_name(model) {
            Err(AppError::Blocked("앱에 등록된 로컬 모델만 내려받을 수 있습니다".into()))
        } else {
            Ok(())
        };
        self.record(Purpose::ModelDownload, MODEL_REGISTRY_HOST, &format!("모델 {model} 내려받기"), result.is_ok());
        result
    }

    /// 루프백 전용 HTTP 클라이언트. 호출 전에 반드시 `check(LocalInference, ..)`를 통과해야 한다.
    pub fn loopback_request(&self, method: reqwest::Method, url: &Url) -> AppResult<reqwest::RequestBuilder> {
        self.check(Purpose::LocalInference, url, "")?;
        Ok(self.loopback.request(method, url.clone()))
    }

    /// 엔진 내려받기 전용 요청. 리다이렉트는 허용 목록 안에서만 따른다.
    pub fn engine_download_request(&self, url: &Url, detail: &str) -> AppResult<reqwest::RequestBuilder> {
        self.check(Purpose::EngineDownload, url, detail)?;
        let client = reqwest::Client::builder()
            .https_only(true)
            .redirect(reqwest::redirect::Policy::custom(|attempt| {
                let ok = attempt.url().scheme() == "https"
                    && attempt
                        .url()
                        .host_str()
                        .map(|h| ENGINE_DOWNLOAD_HOSTS.contains(&h.to_ascii_lowercase().as_str()))
                        .unwrap_or(false);
                if ok && attempt.previous().len() < 5 {
                    attempt.follow()
                } else {
                    attempt.stop()
                }
            }))
            .connect_timeout(Duration::from_secs(20))
            .user_agent("PhilosophyLocalAI")
            .build()
            .map_err(|_| AppError::Network("내려받기 준비에 실패했습니다".into()))?;
        // 쿠키·인증·사용자 데이터 없이 GET만 수행한다.
        Ok(client.get(url.clone()))
    }
}

/// 호스트가 루프백 IP 리터럴인지 검사한다. `localhost` 같은 이름은 DNS/hosts 조작
/// 가능성이 있으므로 허용하지 않는다.
pub fn is_loopback_host(url: &Url) -> bool {
    match url.host() {
        Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
        Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
        _ => false,
    }
}

/// Ollama 클라우드 모델 이름 규칙(`-cloud`, `:cloud`)에 해당하는지.
pub fn is_cloud_model_name(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.ends_with("-cloud") || n.ends_with(":cloud") || n.contains("://") || n.contains('/')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u(s: &str) -> Url {
        Url::parse(s).unwrap()
    }

    #[test]
    fn local_inference_only_loopback_ip() {
        let g = NetGuard::new();
        assert!(g.check(Purpose::LocalInference, &u("http://127.0.0.1:41434/api/chat"), "").is_ok());
        for bad in [
            "http://localhost:11434/api/chat",
            "https://api.openai.com/v1/chat/completions",
            "https://api.anthropic.com/v1/messages",
            "https://ollama.com/api/chat",
            "http://192.168.0.10:11434/api/chat",
            "http://0.0.0.0:11434/api/chat",
            "http://127.0.0.1.evil.example/api/chat",
            "http://user@127.0.0.1:1/api/chat",
            "https://127.0.0.1:1/api/chat",
        ] {
            assert!(g.check(Purpose::LocalInference, &u(bad), "").is_err(), "{bad} 는 차단되어야 함");
        }
        // 차단된 시도는 감사 기록에 남는다.
        assert!(g.audit_log().iter().all(|e| !e.allowed));
        assert!(!g.audit_log().is_empty());
    }

    #[test]
    fn engine_download_requires_consent_and_allowlist() {
        let g = NetGuard::new();
        let ok = u("https://github.com/ollama/ollama/releases/download/v0.40.16/ollama-darwin.tgz");
        assert!(matches!(g.check(Purpose::EngineDownload, &ok, ""), Err(AppError::ConsentRequired(_))));
        g.set_download_consent(true);
        assert!(g.check(Purpose::EngineDownload, &ok, "").is_ok());
        assert!(g.check(Purpose::EngineDownload, &u("http://github.com/x"), "").is_err());
        assert!(g.check(Purpose::EngineDownload, &u("https://github.com.evil.example/x"), "").is_err());
        assert!(g.check(Purpose::EngineDownload, &u("https://example.com/ollama.zip"), "").is_err());
    }

    #[test]
    fn model_pull_requires_consent_and_catalog() {
        let g = NetGuard::new();
        let allowed = vec!["gemma3:4b".to_string()];
        assert!(matches!(g.authorize_model_pull("gemma3:4b", &allowed), Err(AppError::ConsentRequired(_))));
        g.set_download_consent(true);
        assert!(g.authorize_model_pull("gemma3:4b", &allowed).is_ok());
        assert!(g.authorize_model_pull("gpt-oss:120b-cloud", &allowed).is_err());
        assert!(g.authorize_model_pull("unknown:1b", &allowed).is_err());
        assert!(g.authorize_model_pull("hf.co/someone/model", &allowed).is_err());
        // 동의 철회 즉시 차단
        g.set_download_consent(false);
        assert!(g.authorize_model_pull("gemma3:4b", &allowed).is_err());
    }

    #[test]
    fn cloud_name_detection() {
        assert!(is_cloud_model_name("qwen3-coder:480b-cloud"));
        assert!(is_cloud_model_name("glm-4.6:cloud"));
        assert!(!is_cloud_model_name("gemma3:4b"));
    }
}
