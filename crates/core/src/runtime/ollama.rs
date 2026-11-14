//! Ollama 어댑터 — 로컬(127.0.0.1) Ollama API 클라이언트.
//!
//! 사용하는 엔드포인트는 다음으로 제한한다:
//! `/api/version`, `/api/status`, `/api/tags`, `/api/pull`, `/api/delete`, `/api/chat`.
//! 웹 검색(`/api/experimental/web_*`), push, 계정 관련 엔드포인트는 사용하지 않는다.
use super::*;
use crate::error::{AppError, AppResult};
use crate::security::netguard::is_cloud_model_name;
use crate::security::{redact::redact, NetGuard};
use futures_util::StreamExt;
use reqwest::Method;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use url::Url;

pub struct OllamaRuntime {
    base: Url,
    guard: Arc<NetGuard>,
    /// 내려받기가 허용된 모델 이름 (앱 카탈로그)
    allowed_models: Vec<String>,
}

impl OllamaRuntime {
    pub fn new(port: u16, guard: Arc<NetGuard>, allowed_models: Vec<String>) -> Self {
        let base = Url::parse(&format!("http://127.0.0.1:{port}/")).unwrap();
        Self { base, guard, allowed_models }
    }

    pub fn port(&self) -> u16 {
        self.base.port().unwrap_or(0)
    }

    fn url(&self, path: &str) -> Url {
        self.base.join(path).unwrap()
    }

    fn net_err(e: reqwest::Error) -> AppError {
        if e.is_connect() {
            AppError::Runtime("AI 실행 엔진에 연결할 수 없습니다. 엔진을 다시 시작해 주세요.".into())
        } else if e.is_timeout() {
            AppError::Runtime("AI 실행 엔진이 응답하지 않습니다.".into())
        } else {
            AppError::Runtime("AI 실행 엔진과의 통신에 실패했습니다.".into())
        }
    }

    async fn error_from_response(resp: reqwest::Response) -> AppError {
        let status = resp.status();
        let body: Value = resp.json().await.unwrap_or(Value::Null);
        let msg = body["error"].as_str().unwrap_or("");
        translate_engine_error(status.as_u16(), msg)
    }

    async fn get_json(&self, path: &str) -> AppResult<Value> {
        let resp = self
            .guard
            .loopback_request(Method::GET, &self.url(path))?
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .map_err(Self::net_err)?;
        if !resp.status().is_success() {
            return Err(Self::error_from_response(resp).await);
        }
        resp.json().await.map_err(Self::net_err)
    }

    /// 한 번의 내려받기 시도. 줄 단위 JSON 스트림을 읽는다.
    async fn pull_once(&self, name: &str, on_progress: &mut ProgressSink, cancel: &CancellationToken) -> AppResult<()> {
        let resp = self
            .guard
            .loopback_request(Method::POST, &self.url("api/pull"))?
            .json(&json!({ "model": name, "stream": true }))
            .send()
            .await
            .map_err(Self::net_err)?;
        if !resp.status().is_success() {
            return Err(Self::error_from_response(resp).await);
        }
        let mut layers: HashMap<String, (u64, u64)> = HashMap::new();
        let mut verifying = false;
        let mut success = false;
        let mut lines = LineStream::new(resp);
        loop {
            let line = tokio::select! {
                _ = cancel.cancelled() => return Err(AppError::Cancelled),
                l = lines.next_line() => l?,
            };
            let Some(line) = line else { break };
            let Ok(v) = serde_json::from_str::<Value>(&line) else { continue };
            if let Some(e) = v["error"].as_str() {
                return Err(translate_engine_error(500, e));
            }
            let status = v["status"].as_str().unwrap_or("");
            if let (Some(d), Some(t)) = (v["digest"].as_str(), v["total"].as_u64()) {
                layers.insert(d.to_string(), (t, v["completed"].as_u64().unwrap_or(0)));
            }
            if status.starts_with("verifying") {
                verifying = true;
            }
            if status == "success" {
                success = true;
            }
            let (total, completed) = layers.values().fold((0, 0), |a, l| (a.0 + l.0, a.1 + l.1));
            on_progress(PullProgress {
                stage: translate_pull_status(status).to_string(),
                total_bytes: total,
                completed_bytes: completed,
                verifying,
                done: success,
            });
        }
        if success {
            Ok(())
        } else {
            Err(AppError::Network("내려받기가 중간에 끊겼습니다".into()))
        }
    }
}

/// 응답 본문을 줄 단위로 읽는다 (NDJSON).
struct LineStream {
    stream: futures_util::stream::BoxStream<'static, Result<Vec<u8>, ()>>,
    buf: Vec<u8>,
    ended: bool,
}

impl LineStream {
    fn new(resp: reqwest::Response) -> Self {
        Self { stream: resp.bytes_stream().map(|r| r.map(|b| b.to_vec()).map_err(|_| ())).boxed(), buf: Vec::new(), ended: false }
    }

    async fn next_line(&mut self) -> AppResult<Option<String>> {
        loop {
            if let Some(pos) = self.buf.iter().position(|b| *b == b'\n') {
                let line: Vec<u8> = self.buf.drain(..=pos).collect();
                let s = String::from_utf8_lossy(&line[..line.len() - 1]).trim().to_string();
                if s.is_empty() {
                    continue;
                }
                return Ok(Some(s));
            }
            if self.ended {
                if self.buf.is_empty() {
                    return Ok(None);
                }
                let s = String::from_utf8_lossy(&self.buf).trim().to_string();
                self.buf.clear();
                return Ok(if s.is_empty() { None } else { Some(s) });
            }
            match self.stream.next().await {
                Some(Ok(chunk)) => self.buf.extend_from_slice(&chunk),
                Some(Err(_)) => return Err(AppError::Network("연결이 끊겼습니다".into())),
                None => self.ended = true,
            }
        }
    }
}

fn translate_pull_status(s: &str) -> &'static str {
    if s.starts_with("pulling manifest") {
        "모델 정보를 확인하는 중"
    } else if s.starts_with("pulling") || s.starts_with("downloading") {
        "AI 모델을 내려받는 중"
    } else if s.starts_with("verifying") {
        "내려받은 파일이 올바른지 확인하는 중"
    } else if s.starts_with("writing") {
        "설치하는 중"
    } else if s.starts_with("removing") {
        "정리하는 중"
    } else if s == "success" {
        "설치 완료"
    } else {
        "준비하는 중"
    }
}

/// 엔진 오류 문자열을 한국어 안내로 바꾼다. 원문은 개인정보 제거 후 덧붙인다.
pub fn translate_engine_error(status: u16, msg: &str) -> AppError {
    let m = msg.to_ascii_lowercase();
    if m.contains("no space left") || m.contains("not enough space") || m.contains("disk full") {
        AppError::DiskSpace("AI 모델을 저장할 공간이 부족합니다. 저장 공간을 확보한 뒤 다시 시도해 주세요.".into())
    } else if m.contains("requires more system memory") || m.contains("out of memory") {
        AppError::Runtime("이 AI 모델을 실행하기에 메모리가 부족합니다. 더 가벼운 모델을 선택하거나 다른 프로그램을 닫아 주세요.".into())
    } else if m.contains("no such host") || m.contains("dial tcp") || m.contains("timeout") || m.contains("connection re") || m.contains("eof") {
        AppError::Network("인터넷 연결을 확인해 주세요. 연결이 복구되면 이어서 내려받을 수 있습니다.".into())
    } else if m.contains("file does not exist") || m.contains("not found") || status == 404 {
        AppError::Runtime("요청한 AI 모델을 찾을 수 없습니다.".into())
    } else if m.contains("cloud") || m.contains("remote") {
        AppError::Blocked("클라우드 모델은 사용할 수 없습니다. 이 앱은 내 컴퓨터에서 실행되는 모델만 사용합니다.".into())
    } else if msg.is_empty() {
        AppError::Runtime(format!("AI 실행 엔진이 오류를 반환했습니다 (코드 {status})."))
    } else {
        AppError::Runtime(format!("AI 실행 엔진이 오류를 반환했습니다: {}", redact(msg)))
    }
}

#[async_trait]
impl AiRuntime for OllamaRuntime {
    fn id(&self) -> &'static str {
        "ollama"
    }

    async fn health(&self) -> AppResult<RuntimeHealth> {
        let version = match self.get_json("api/version").await {
            Ok(v) => v["version"].as_str().map(|s| s.to_string()),
            Err(_) => return Ok(RuntimeHealth::default()),
        };
        // `/api/status`가 없는 구버전이면 None (= 확인 불가 → '로컬 전용' 표시 비활성)
        let cloud_disabled = self.get_json("api/status").await.ok().and_then(|v| v["cloud"]["disabled"].as_bool());
        Ok(RuntimeHealth { running: true, version, cloud_disabled })
    }

    async fn list_models(&self) -> AppResult<Vec<InstalledModel>> {
        let v = self.get_json("api/tags").await?;
        Ok(v["models"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|m| {
                        let name = m["name"].as_str().or_else(|| m["model"].as_str())?.to_string();
                        let is_remote = m["remote_host"].as_str().map(|s| !s.is_empty()).unwrap_or(false)
                            || m["remote_model"].as_str().map(|s| !s.is_empty()).unwrap_or(false)
                            || is_cloud_model_name(&name);
                        Some(InstalledModel {
                            is_remote,
                            size_bytes: m["size"].as_u64().unwrap_or(0),
                            modified_at: m["modified_at"].as_str().unwrap_or("").to_string(),
                            parameter_size: m["details"]["parameter_size"].as_str().map(|s| s.to_string()),
                            quantization: m["details"]["quantization_level"].as_str().map(|s| s.to_string()),
                            name,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default())
    }

    async fn pull_model(&self, name: &str, mut on_progress: ProgressSink, cancel: CancellationToken) -> AppResult<()> {
        if !self.allowed_models.iter().any(|m| m == name) || is_cloud_model_name(name) {
            return Err(AppError::Blocked("앱에 등록된 로컬 모델만 내려받을 수 있습니다".into()));
        }
        if cancel.is_cancelled() { return Err(AppError::Cancelled); }
        if self.list_models().await?.iter().any(|m| m.name == name && !m.is_remote) {
            on_progress(PullProgress { stage: "이미 받은 AI를 재사용합니다".into(), done: true, ..Default::default() });
            return Ok(());
        }
        const MAX_ATTEMPTS: u32 = 4;
        let mut attempt = 0;
        loop {
            // 매 시도마다 동의·카탈로그 검사를 다시 한다 (도중에 동의를 철회할 수 있음).
            self.guard.authorize_model_pull(name, &self.allowed_models)?;
            attempt += 1;
            match self.pull_once(name, &mut on_progress, &cancel).await {
                Ok(()) => return Ok(()),
                // 네트워크 오류만 재시도한다. Ollama는 받은 부분부터 이어받는다.
                Err(AppError::Network(_)) if attempt < MAX_ATTEMPTS => {
                    on_progress(PullProgress { stage: format!("연결이 끊겨 다시 시도하는 중 ({attempt}/{})", MAX_ATTEMPTS - 1), ..Default::default() });
                    tokio::select! {
                        _ = cancel.cancelled() => return Err(AppError::Cancelled),
                        _ = tokio::time::sleep(Duration::from_millis(retry_delay_ms(attempt))) => {}
                    }
                }
                Err(e) => return Err(e),
            }
        }
    }

    async fn delete_model(&self, name: &str) -> AppResult<()> {
        let resp = self
            .guard
            .loopback_request(Method::DELETE, &self.url("api/delete"))?
            .json(&json!({ "model": name }))
            .timeout(Duration::from_secs(30))
            .send()
            .await
            .map_err(Self::net_err)?;
        if resp.status().is_success() {
            Ok(())
        } else {
            Err(Self::error_from_response(resp).await)
        }
    }

    async fn chat(&self, req: ChatRequest, mut on_token: TokenSink, cancel: CancellationToken) -> AppResult<ChatOutcome> {
        if is_cloud_model_name(&req.model) {
            return Err(AppError::Blocked("클라우드 모델은 사용할 수 없습니다".into()));
        }
        let mut options = serde_json::Map::new();
        if let Some(n) = req.num_ctx {
            options.insert("num_ctx".into(), json!(n));
        }
        if let Some(t) = req.temperature {
            options.insert("temperature".into(), json!(t));
        }
        let body = json!({
            "model": req.model,
            "messages": req.messages,
            "stream": true,
            // 생각(thinking) 과정은 끈다: 응답이 빨라지고 화면에 불필요한 내용이 나오지 않는다.
            "think": false,
            "options": options,
        });
        let send = self.guard.loopback_request(Method::POST, &self.url("api/chat"))?.json(&body).send();
        let resp = tokio::select! {
            _ = cancel.cancelled() => return Ok(ChatOutcome { cancelled: true, ..Default::default() }),
            r = send => r.map_err(Self::net_err)?,
        };
        if !resp.status().is_success() {
            return Err(Self::error_from_response(resp).await);
        }
        let mut out = ChatOutcome::default();
        let mut lines = LineStream::new(resp);
        loop {
            let line = tokio::select! {
                // 연결을 끊으면 Ollama가 생성을 중단한다.
                _ = cancel.cancelled() => { out.cancelled = true; break; }
                l = lines.next_line() => l?,
            };
            let Some(line) = line else { break };
            let Ok(v) = serde_json::from_str::<Value>(&line) else { continue };
            if let Some(e) = v["error"].as_str() {
                return Err(translate_engine_error(500, e));
            }
            // 원격 실행 응답이면 즉시 중단 (방어적 검사)
            if v["remote_host"].as_str().map(|s| !s.is_empty()).unwrap_or(false) {
                return Err(AppError::Blocked("원격에서 실행된 응답이 감지되어 중단했습니다".into()));
            }
            if let Some(tok) = v["message"]["content"].as_str() {
                if !tok.is_empty() {
                    out.content.push_str(tok);
                    on_token(tok);
                }
            }
            if v["done"].as_bool() == Some(true) {
                out.eval_tokens = v["eval_count"].as_u64();
                if let (Some(c), Some(d)) = (v["eval_count"].as_f64(), v["eval_duration"].as_f64()) {
                    if d > 0.0 {
                        out.tokens_per_sec = Some((c / (d / 1e9) * 10.0).round() / 10.0);
                    }
                }
                break;
            }
        }
        Ok(out)
    }
}

fn retry_delay_ms(attempt: u32) -> u64 {
    if cfg!(test) || std::env::var("PLA_FAST_RETRY").is_ok() {
        20
    } else {
        2000 * 2u64.pow(attempt.saturating_sub(1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_translation_is_korean_and_redacted() {
        assert!(matches!(translate_engine_error(500, "write /x: no space left on device"), AppError::DiskSpace(_)));
        assert!(matches!(translate_engine_error(500, "dial tcp: lookup registry.ollama.ai: no such host"), AppError::Network(_)));
        assert!(matches!(translate_engine_error(404, ""), AppError::Runtime(_)));
        let e = translate_engine_error(500, "weird failure for hong@univ.ac.kr").to_string();
        assert!(!e.contains("hong@univ.ac.kr"));
    }
}
