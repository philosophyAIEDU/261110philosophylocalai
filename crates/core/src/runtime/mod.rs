//! Inference Layer — AI Runtime Adapter.
//!
//! 앱의 나머지 부분은 `AiRuntime` 트레이트만 사용한다. 현재 구현은 Ollama 하나이며,
//! 향후 llama.cpp(`llama-server`) 어댑터를 같은 트레이트로 추가할 수 있다.
pub mod engine;
pub mod ollama;

use crate::error::AppResult;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatMessage {
    /// "system" | "user" | "assistant"
    pub role: String,
    pub content: String,
}

impl ChatMessage {
    pub fn system(c: impl Into<String>) -> Self {
        Self { role: "system".into(), content: c.into() }
    }
    pub fn user(c: impl Into<String>) -> Self {
        Self { role: "user".into(), content: c.into() }
    }
    pub fn assistant(c: impl Into<String>) -> Self {
        Self { role: "assistant".into(), content: c.into() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    /// 컨텍스트 길이 (토큰). None이면 엔진 기본값.
    pub num_ctx: Option<u32>,
    pub temperature: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct ChatOutcome {
    pub content: String,
    /// 사용자가 중지했는지
    pub cancelled: bool,
    pub eval_tokens: Option<u64>,
    pub tokens_per_sec: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledModel {
    pub name: String,
    pub size_bytes: u64,
    pub modified_at: String,
    pub parameter_size: Option<String>,
    pub quantization: Option<String>,
    /// 원격(클라우드)에서 실행되는 모델인지 — true이면 앱에서 사용을 거부한다
    pub is_remote: bool,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct PullProgress {
    /// 사용자에게 보여 줄 단계 (한국어)
    pub stage: String,
    pub total_bytes: u64,
    pub completed_bytes: u64,
    /// 무결성(sha256) 확인 단계에 도달했는지
    pub verifying: bool,
    pub done: bool,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct RuntimeHealth {
    pub running: bool,
    pub version: Option<String>,
    /// 클라우드 기능이 꺼져 있음을 엔진이 확인해 주었는지
    pub cloud_disabled: Option<bool>,
}

pub type TokenSink = Box<dyn FnMut(&str) + Send>;
pub type ProgressSink = Box<dyn FnMut(PullProgress) + Send>;

#[async_trait]
pub trait AiRuntime: Send + Sync {
    fn id(&self) -> &'static str;
    async fn health(&self) -> AppResult<RuntimeHealth>;
    async fn list_models(&self) -> AppResult<Vec<InstalledModel>>;
    async fn pull_model(&self, name: &str, on_progress: ProgressSink, cancel: CancellationToken) -> AppResult<()>;
    async fn delete_model(&self, name: &str) -> AppResult<()>;
    async fn chat(&self, req: ChatRequest, on_token: TokenSink, cancel: CancellationToken) -> AppResult<ChatOutcome>;

    /// 스트리밍 없이 전체 응답만 받는 편의 함수.
    async fn complete(&self, req: ChatRequest, cancel: CancellationToken) -> AppResult<String> {
        let out = self.chat(req, Box::new(|_| {}), cancel).await?;
        if out.cancelled {
            return Err(crate::error::AppError::Cancelled);
        }
        Ok(out.content)
    }
}
