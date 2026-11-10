//! 개발·테스트 전용 "가짜 엔진".
//!
//! 실제 Ollama와 같은 환경변수(`OLLAMA_HOST`, `OLLAMA_NO_CLOUD`, `OLLAMA_MODELS`)와
//! 같은 HTTP API 형태를 흉내 낸다. AI 추론은 하지 않고 정해진 문장을 돌려준다.
//! 이 프로그램은 배포용 앱에 포함되지 않는다.
use axum::{
    body::Body,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Json, Router,
};
use serde_json::{json, Value};
use std::convert::Infallible;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Default)]
struct Inner {
    models: Vec<Value>,
    requests: Vec<Value>,
    pull_attempts: u32,
}

#[derive(Clone)]
struct App {
    inner: Arc<Mutex<Inner>>,
    models_file: Option<PathBuf>,
    no_cloud: bool,
}

impl App {
    fn save(&self) {
        if let Some(f) = &self.models_file {
            let _ = std::fs::write(f, serde_json::to_vec(&self.inner.lock().unwrap().models).unwrap());
        }
    }
    fn log(&self, path: &str, body: &Value) {
        self.inner.lock().unwrap().requests.push(json!({ "path": path, "body": body }));
    }
}

fn env_u64(k: &str, d: u64) -> u64 {
    std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d)
}

fn ndjson_stream(rx: tokio::sync::mpsc::Receiver<Result<String, ()>>) -> Response {
    let s = futures_util::stream::unfold(rx, |mut rx| async move {
        match rx.recv().await {
            Some(Ok(line)) => Some((Ok::<_, std::io::Error>(line + "\n"), rx)),
            // 연결을 비정상 종료시켜 네트워크 오류를 흉내 낸다
            Some(Err(())) => Some((Err(std::io::Error::other("drop")), rx)),
            None => None,
        }
    });
    Response::builder().header("content-type", "application/x-ndjson").body(Body::from_stream(s)).unwrap()
}

async fn version() -> Json<Value> {
    Json(json!({ "version": "0.0.0-fake" }))
}

async fn status(State(app): State<App>) -> Json<Value> {
    Json(json!({ "cloud": { "disabled": app.no_cloud, "source": if app.no_cloud { "env" } else { "none" } } }))
}

async fn tags(State(app): State<App>) -> Json<Value> {
    Json(json!({ "models": app.inner.lock().unwrap().models }))
}

async fn pull(State(app): State<App>, Json(body): Json<Value>) -> Response {
    app.log("/api/pull", &body);
    let name = body["model"].as_str().unwrap_or("").to_string();
    let attempt = {
        let mut i = app.inner.lock().unwrap();
        i.pull_attempts += 1;
        i.pull_attempts
    };
    if name.contains("missing") {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": "pull model manifest: file does not exist" }))).into_response();
    }
    let fail_first = env_u64("FAKE_PULL_FAIL_FIRST", 0) as u32;
    let delay = env_u64("FAKE_PULL_DELAY_MS", 5);
    let total: u64 = env_u64("FAKE_PULL_BYTES", 1_000_000);
    let (tx, rx) = tokio::sync::mpsc::channel(8);
    tokio::spawn(async move {
        let send = |v: Value| {
            let tx = tx.clone();
            async move { tx.send(Ok(v.to_string())).await.is_ok() }
        };
        if !send(json!({ "status": "pulling manifest" })).await {
            return;
        }
        for step in 1..=10u64 {
            tokio::time::sleep(Duration::from_millis(delay)).await;
            if attempt <= fail_first && step == 4 {
                let _ = tx.send(Err(())).await;
                return;
            }
            if !send(json!({ "status": "pulling aaaabbbbcccc", "digest": "sha256:aaaabbbbcccc", "total": total, "completed": total / 10 * step })).await {
                return;
            }
        }
        if std::env::var("FAKE_PULL_DISK_FULL").is_ok() {
            let _ = send(json!({ "error": "write blob: no space left on device" })).await;
            return;
        }
        let _ = send(json!({ "status": "verifying sha256 digest" })).await;
        let _ = send(json!({ "status": "writing manifest" })).await;
        {
            let mut i = app.inner.lock().unwrap();
            i.models.retain(|m| m["name"] != name.as_str());
            i.models.push(json!({
                "name": name, "model": name, "size": total, "modified_at": "2026-01-01T00:00:00Z",
                "details": { "parameter_size": "4.3B", "quantization_level": "Q4_K_M" }
            }));
        }
        app.save();
        let _ = send(json!({ "status": "success" })).await;
    });
    ndjson_stream(rx)
}

async fn delete_model(State(app): State<App>, Json(body): Json<Value>) -> Response {
    app.log("/api/delete", &body);
    let name = body["model"].as_str().unwrap_or("");
    let removed = {
        let mut i = app.inner.lock().unwrap();
        let before = i.models.len();
        i.models.retain(|m| m["name"] != name);
        before != i.models.len()
    };
    app.save();
    if removed {
        StatusCode::OK.into_response()
    } else {
        (StatusCode::NOT_FOUND, Json(json!({ "error": format!("model '{name}' not found") }))).into_response()
    }
}

async fn chat(State(app): State<App>, Json(body): Json<Value>) -> Response {
    app.log("/api/chat", &body);
    let model = body["model"].as_str().unwrap_or("").to_string();
    if !app.inner.lock().unwrap().models.iter().any(|m| m["name"] == model.as_str()) {
        return (StatusCode::NOT_FOUND, Json(json!({ "error": format!("model '{model}' not found") }))).into_response();
    }
    let n = body["messages"].as_array().map(|a| a.len()).unwrap_or(0);
    let last = body["messages"].as_array().and_then(|a| a.last()).and_then(|m| m["content"].as_str()).unwrap_or("");
    let text = format!(
        "## 가짜 엔진 응답\n\n이것은 **개발·테스트용 가짜 엔진**이 돌려주는 정해진 문장입니다. 실제 AI의 답변이 아닙니다.\n\n- 받은 메시지 수: {n}\n- 마지막 메시지 글자 수: {}\n",
        last.chars().count()
    );
    let delay = env_u64("FAKE_TOKEN_DELAY_MS", 3);
    let (tx, rx) = tokio::sync::mpsc::channel(8);
    tokio::spawn(async move {
        let chars: Vec<char> = text.chars().collect();
        for piece in chars.chunks(4) {
            tokio::time::sleep(Duration::from_millis(delay)).await;
            let tok: String = piece.iter().collect();
            let line = json!({ "model": model, "message": { "role": "assistant", "content": tok }, "done": false });
            if tx.send(Ok(line.to_string())).await.is_err() {
                return; // 클라이언트가 연결을 끊음 (응답 중지)
            }
        }
        let _ = tx
            .send(Ok(json!({ "model": model, "message": { "role": "assistant", "content": "" }, "done": true, "eval_count": chars.len() / 4, "eval_duration": 1_000_000_000u64 }).to_string()))
            .await;
    });
    ndjson_stream(rx)
}

async fn test_requests(State(app): State<App>) -> Json<Value> {
    Json(json!(app.inner.lock().unwrap().requests))
}

async fn not_supported() -> Response {
    (StatusCode::NOT_FOUND, Json(json!({ "error": "not supported by fake engine" }))).into_response()
}

#[tokio::main]
async fn main() -> Result<(), Infallible> {
    if std::env::args().nth(1).as_deref() != Some("serve") {
        eprintln!("usage: pla-fake-engine serve");
        std::process::exit(2);
    }
    let host = std::env::var("OLLAMA_HOST").unwrap_or_else(|_| "127.0.0.1:11434".into());
    let models_file = std::env::var("OLLAMA_MODELS").ok().map(|d| {
        let _ = std::fs::create_dir_all(&d);
        PathBuf::from(d).join("fake-models.json")
    });
    let models = models_file.as_ref().and_then(|f| std::fs::read(f).ok()).and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    let app = App {
        inner: Arc::new(Mutex::new(Inner { models, ..Default::default() })),
        models_file,
        no_cloud: matches!(std::env::var("OLLAMA_NO_CLOUD").as_deref(), Ok("1") | Ok("true")),
    };
    let router = Router::new()
        .route("/api/version", get(version))
        .route("/api/status", get(status))
        .route("/api/tags", get(tags))
        .route("/api/pull", post(pull))
        .route("/api/delete", delete(delete_model))
        .route("/api/chat", post(chat))
        .route("/__test/requests", get(test_requests))
        .fallback(not_supported)
        .with_state(app);
    let listener = tokio::net::TcpListener::bind(&host).await.expect("bind");
    axum::serve(listener, router).await.expect("serve");
    Ok(())
}
