//! Philosophy Local AI — 데스크톱 앱 진입점.
//!
//! 이 파일은 얇은 연결 계층이다. 업무 로직·보안 통제는 모두 `pla-core`에 있고,
//! 여기서는 화면(웹뷰)의 요청을 받아 `pla-core`를 호출하고 결과·진행 상황을 돌려준다.
//! 웹뷰는 네트워크에 직접 접근하지 않으며(CSP), AI 엔진과도 직접 통신하지 않는다.

use pla_core::documents::{self, ExtractedDocument};
use pla_core::error::{AppError, AppResult};
use pla_core::hardware::{self, HardwareProfile, GB};
use pla_core::models::recommend::{self, Purpose, RecommendationResult};
use pla_core::models::Catalog;
use pla_core::runtime::engine::{check_free_space, verify_local_only, EngineManager, EngineStatus, LocalOnlyReport};
use pla_core::runtime::ollama::OllamaRuntime;
use pla_core::runtime::{AiRuntime, ChatMessage, ChatOutcome, ChatRequest, InstalledModel};
use pla_core::security::crypto::{Cipher, KeyStoreKind};
use pla_core::security::{AuditEntry, NetGuard};
use pla_core::storage::{ConversationMeta, Store, StoreStats, StoredMessage, StudentNote};
use pla_core::tabular::{self, Agg, GroupResult, Table, TableProfile};
use pla_core::tasks::{self, Material, TaskDef};
use pla_core::{export, AppError as E};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio_util::sync::CancellationToken;

struct AppState {
    data_dir: PathBuf,
    guard: Arc<NetGuard>,
    engine: Arc<EngineManager>,
    store: Arc<Store>,
    catalog: Catalog,
    key_store: KeyStoreKind,
    hardware: Mutex<Option<HardwareProfile>>,
    jobs: Mutex<HashMap<String, CancellationToken>>,
    /// 불러온 문서는 메모리에만 둔다 (앱을 닫으면 사라진다).
    docs: Mutex<HashMap<String, ExtractedDocument>>,
    tables: Mutex<HashMap<String, Table>>,
}

impl AppState {
    fn register_job(&self, id: &str) -> CancellationToken {
        let t = CancellationToken::new();
        self.jobs.lock().unwrap().insert(id.to_string(), t.clone());
        t
    }

    fn finish_job(&self, id: &str) {
        self.jobs.lock().unwrap().remove(id);
    }

    async fn runtime(&self) -> AppResult<OllamaRuntime> {
        let port = self.engine.start().await?;
        Ok(OllamaRuntime::new(port, self.guard.clone(), self.catalog.allowed_ids()))
    }

    fn selected_model(&self) -> AppResult<String> {
        self.store
            .get_setting("selected_model")?
            .filter(|s| !s.is_empty())
            .ok_or_else(|| E::Other("사용할 AI 모델을 먼저 선택해 주세요 ('내 AI 모델 관리' 메뉴)".into()))
    }

    /// 컴퓨터 메모리에 맞춘 컨텍스트 길이.
    fn num_ctx(&self) -> u32 {
        let ram = self.hardware.lock().unwrap().as_ref().map(|h| h.total_ram).unwrap_or(8 * GB);
        if ram >= 15 * GB {
            8192
        } else {
            4096
        }
    }

    fn materials(&self, doc_ids: &[String], pasted: &str) -> AppResult<Vec<Material>> {
        let docs = self.docs.lock().unwrap();
        let mut out = Vec::new();
        for id in doc_ids {
            let d = docs.get(id).ok_or_else(|| E::Other("문서를 찾을 수 없습니다. 다시 불러와 주세요.".into()))?;
            out.push(Material { name: d.file_name.clone(), text: d.text.clone() });
        }
        if !pasted.trim().is_empty() {
            out.push(Material { name: "직접 입력한 내용".into(), text: pasted.trim().to_string() });
        }
        Ok(out)
    }
}

// ---------------------------------------------------------------------------
// 상태·진단·추천
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct AppStatus {
    app_version: String,
    engine: EngineStatus,
    engine_name: String,
    selected_model: Option<String>,
    onboarding_done: bool,
    download_consent: bool,
    save_history: bool,
    purpose: Purpose,
}

fn stored_purpose(store: &Store) -> Purpose {
    store.get_setting("purpose").ok().flatten().and_then(|s| serde_json::from_str(&format!("\"{s}\"")).ok()).unwrap_or_default()
}

/// 사양 진단 결과가 없으면 한 번 진단해 둔다 (컨텍스트 길이·저장 공간 표시에 필요).
async fn ensure_hardware(state: &AppState) {
    if state.hardware.lock().unwrap().is_some() {
        return;
    }
    let dir = state.data_dir.clone();
    if let Ok(h) = tauri::async_runtime::spawn_blocking(move || hardware::detect(&dir)).await {
        *state.hardware.lock().unwrap() = Some(h);
    }
}

#[tauri::command]
async fn app_status(state: State<'_, AppState>) -> AppResult<AppStatus> {
    ensure_hardware(&state).await;
    Ok(AppStatus {
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        engine: state.engine.status().await,
        engine_name: format!("Ollama {}", state.engine.manifest().version),
        selected_model: state.store.get_setting("selected_model")?.filter(|s| !s.is_empty()),
        onboarding_done: state.store.get_bool("onboarding_done", false),
        download_consent: state.guard.download_consent(),
        save_history: state.store.get_bool("save_history", false),
        purpose: stored_purpose(&state.store),
    })
}

#[derive(Serialize)]
struct HardwareReport {
    profile: HardwareProfile,
    summary: Vec<String>,
}

#[tauri::command]
async fn detect_hardware(state: State<'_, AppState>) -> AppResult<HardwareReport> {
    let dir = state.data_dir.clone();
    let profile = tauri::async_runtime::spawn_blocking(move || hardware::detect(&dir)).await.map_err(|_| E::Other("컴퓨터 사양을 확인하지 못했습니다".into()))?;
    *state.hardware.lock().unwrap() = Some(profile.clone());
    Ok(HardwareReport { summary: profile.summary_ko(), profile })
}

#[tauri::command]
async fn recommend_models(state: State<'_, AppState>, purpose: Purpose) -> AppResult<RecommendationResult> {
    state.store.set_setting("purpose", serde_json::to_string(&purpose).unwrap().trim_matches('"'))?;
    let hw = state.hardware.lock().unwrap().clone();
    let hw = match hw {
        Some(h) => h,
        None => {
            let dir = state.data_dir.clone();
            let h = tauri::async_runtime::spawn_blocking(move || hardware::detect(&dir)).await.map_err(|_| E::Other("컴퓨터 사양을 확인하지 못했습니다".into()))?;
            *state.hardware.lock().unwrap() = Some(h.clone());
            h
        }
    };
    Ok(recommend::recommend(&state.catalog, &hw, purpose))
}

#[tauri::command]
fn get_catalog(state: State<'_, AppState>) -> Catalog {
    state.catalog.clone()
}

#[tauri::command]
fn complete_onboarding(state: State<'_, AppState>) -> AppResult<()> {
    state.store.set_setting("onboarding_done", "true")
}

// ---------------------------------------------------------------------------
// 네트워크 동의 · 엔진 · 모델
// ---------------------------------------------------------------------------

#[tauri::command]
fn set_download_consent(state: State<'_, AppState>, consent: bool) -> AppResult<()> {
    state.guard.set_download_consent(consent);
    state.store.set_setting("download_consent", if consent { "true" } else { "false" })
}

#[tauri::command]
async fn install_engine(app: AppHandle, state: State<'_, AppState>, job_id: String) -> AppResult<()> {
    let cancel = state.register_job(&job_id);
    let jid = job_id.clone();
    let res = state
        .engine
        .install(
            move |p| {
                let _ = app.emit("engine-install-progress", serde_json::json!({ "job_id": jid, "progress": p }));
            },
            cancel,
        )
        .await;
    state.finish_job(&job_id);
    res
}

#[tauri::command]
async fn start_engine(state: State<'_, AppState>) -> AppResult<u16> {
    state.engine.start().await
}

#[tauri::command]
async fn list_models(state: State<'_, AppState>) -> AppResult<Vec<InstalledModel>> {
    if !state.engine.is_installed() {
        return Ok(vec![]);
    }
    state.runtime().await?.list_models().await
}

#[tauri::command]
async fn pull_model(app: AppHandle, state: State<'_, AppState>, job_id: String, name: String) -> AppResult<()> {
    let model = state.catalog.get(&name).ok_or_else(|| E::Blocked("앱에 등록된 로컬 모델만 내려받을 수 있습니다".into()))?;
    // 저장 공간 확인 (다운로드 용량 + 여유 2GB)
    let models_dir = state.engine.models_dir();
    std::fs::create_dir_all(&models_dir)?;
    check_free_space(&models_dir, ((model.download_gb * 1.1 + 2.0) * 1e9) as u64)?;
    let rt = state.runtime().await?;
    let cancel = state.register_job(&job_id);
    let jid = job_id.clone();
    let res = rt
        .pull_model(
            &name,
            Box::new(move |p| {
                let _ = app.emit("pull-progress", serde_json::json!({ "job_id": jid, "progress": p }));
            }),
            cancel,
        )
        .await;
    state.finish_job(&job_id);
    res
}

#[tauri::command]
fn cancel_job(state: State<'_, AppState>, job_id: String) {
    if let Some(t) = state.jobs.lock().unwrap().get(&job_id) {
        t.cancel();
    }
}

#[tauri::command]
async fn delete_model(state: State<'_, AppState>, name: String) -> AppResult<()> {
    state.runtime().await?.delete_model(&name).await?;
    if state.store.get_setting("selected_model")?.as_deref() == Some(name.as_str()) {
        state.store.set_setting("selected_model", "")?;
    }
    Ok(())
}

#[derive(Serialize)]
struct ModelTest {
    reply: String,
    tokens_per_sec: Option<f64>,
}

/// 내려받은 모델이 실제로 답변을 만드는지 확인한다.
#[tauri::command]
async fn test_model(state: State<'_, AppState>, name: String) -> AppResult<ModelTest> {
    let rt = state.runtime().await?;
    let out = rt
        .chat(
            ChatRequest { model: name, messages: vec![ChatMessage::user("안녕하세요. 한 문장으로 짧게 인사해 주세요.")], num_ctx: Some(2048), temperature: Some(0.2) },
            Box::new(|_| {}),
            CancellationToken::new(),
        )
        .await?;
    if out.content.trim().is_empty() {
        return Err(E::Runtime("AI 모델이 답변을 만들지 못했습니다. 다시 내려받아 주세요.".into()));
    }
    Ok(ModelTest { reply: out.content.trim().to_string(), tokens_per_sec: out.tokens_per_sec })
}

#[tauri::command]
async fn select_model(state: State<'_, AppState>, name: String) -> AppResult<()> {
    let models = state.runtime().await?.list_models().await?;
    match models.iter().find(|m| m.name == name) {
        Some(m) if !m.is_remote => state.store.set_setting("selected_model", &name),
        Some(_) => Err(E::Blocked("클라우드 모델은 사용할 수 없습니다".into())),
        None => Err(E::Other("설치되지 않은 모델입니다".into())),
    }
}

#[tauri::command]
async fn verify_local(state: State<'_, AppState>) -> AppResult<LocalOnlyReport> {
    let Some(port) = state.engine.port().await else {
        return Ok(LocalOnlyReport { details: vec!["AI 엔진이 실행 중이 아닙니다.".into()], ..Default::default() });
    };
    let rt = OllamaRuntime::new(port, state.guard.clone(), state.catalog.allowed_ids());
    let model = state.store.get_setting("selected_model")?.filter(|s| !s.is_empty());
    Ok(verify_local_only(&rt, port, model.as_deref()).await)
}

// ---------------------------------------------------------------------------
// 채팅
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct ChatResult {
    outcome: ChatOutcome,
    conversation_id: Option<String>,
}

fn title_from(text: &str) -> String {
    let t: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let short: String = t.chars().take(28).collect();
    if t.chars().count() > 28 { format!("{short}…") } else { short }
}

fn token_sink(app: &AppHandle, job_id: &str) -> pla_core::runtime::TokenSink {
    let (app, jid) = (app.clone(), job_id.to_string());
    Box::new(move |tok| {
        let _ = app.emit("ai-token", serde_json::json!({ "job_id": jid, "token": tok }));
    })
}

fn stage_sink(app: &AppHandle, job_id: &str) -> tasks::StageSink {
    let (app, jid) = (app.clone(), job_id.to_string());
    Box::new(move |stage| {
        let _ = app.emit("ai-stage", serde_json::json!({ "job_id": jid, "stage": stage }));
    })
}

/// 화면이 보낸 대화 내용에서 system 역할을 제거한다 (시스템 지시는 앱만 넣는다).
fn user_visible_only(messages: Vec<ChatMessage>) -> Vec<ChatMessage> {
    messages.into_iter().filter(|m| m.role == "user" || m.role == "assistant").collect()
}

#[tauri::command]
async fn chat_send(app: AppHandle, state: State<'_, AppState>, job_id: String, conversation_id: Option<String>, messages: Vec<ChatMessage>) -> AppResult<ChatResult> {
    let model = state.selected_model()?;
    let rt = state.runtime().await?;
    let history = user_visible_only(messages);
    let last_user = history.iter().rev().find(|m| m.role == "user").map(|m| m.content.clone()).unwrap_or_default();
    let mut msgs = vec![tasks::chat_system_prompt()];
    msgs.extend(history);
    let cancel = state.register_job(&job_id);
    let res = rt.chat(ChatRequest { model, messages: msgs, num_ctx: Some(state.num_ctx()), temperature: Some(0.6) }, token_sink(&app, &job_id), cancel).await;
    state.finish_job(&job_id);
    let outcome = res?;

    // 대화 기록 저장은 사용자가 켠 경우에만
    let mut conv = conversation_id;
    if state.store.get_bool("save_history", false) && !last_user.is_empty() {
        let id = match &conv {
            Some(id) => id.clone(),
            None => state.store.create_conversation("chat", &title_from(&last_user))?,
        };
        state.store.append_message(&id, "user", &last_user)?;
        if !outcome.content.is_empty() {
            state.store.append_message(&id, "assistant", &outcome.content)?;
        }
        conv = Some(id);
    }
    Ok(ChatResult { outcome, conversation_id: conv })
}

#[tauri::command]
fn list_conversations(state: State<'_, AppState>) -> AppResult<Vec<ConversationMeta>> {
    state.store.list_conversations(Some("chat"))
}

#[tauri::command]
fn get_conversation(state: State<'_, AppState>, id: String) -> AppResult<Vec<StoredMessage>> {
    state.store.get_messages(&id)
}

#[tauri::command]
fn delete_conversation(state: State<'_, AppState>, id: String) -> AppResult<()> {
    state.store.delete_conversation(&id)
}

// ---------------------------------------------------------------------------
// 문서 · 업무 작업
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct DocInfo {
    id: String,
    file_name: String,
    kind: String,
    char_count: usize,
    units: Option<usize>,
    warnings: Vec<String>,
    preview: String,
}

#[tauri::command]
async fn load_document(state: State<'_, AppState>, path: String) -> AppResult<DocInfo> {
    let doc = tauri::async_runtime::spawn_blocking(move || documents::extract(Path::new(&path))).await.map_err(|_| E::File("문서를 읽는 중 오류가 발생했습니다".into()))??;
    let id = uuid::Uuid::new_v4().to_string();
    let info = DocInfo {
        id: id.clone(),
        file_name: doc.file_name.clone(),
        kind: doc.kind.clone(),
        char_count: doc.char_count,
        units: doc.units,
        warnings: doc.warnings.clone(),
        preview: doc.text.chars().take(600).collect(),
    };
    state.docs.lock().unwrap().insert(id, doc);
    Ok(info)
}

#[tauri::command]
fn remove_document(state: State<'_, AppState>, id: String) {
    state.docs.lock().unwrap().remove(&id);
}

#[tauri::command]
fn list_tasks() -> Vec<TaskDef> {
    tasks::TASKS.to_vec()
}

#[tauri::command]
async fn run_task(app: AppHandle, state: State<'_, AppState>, job_id: String, task_id: String, doc_ids: Vec<String>, pasted: String, extra: String) -> AppResult<ChatOutcome> {
    let task = tasks::find_task(&task_id).ok_or_else(|| E::Other("알 수 없는 작업입니다".into()))?;
    let model = state.selected_model()?;
    let materials = state.materials(&doc_ids, &pasted)?;
    let rt = state.runtime().await?;
    let cancel = state.register_job(&job_id);
    let res = tasks::run_task(&rt, &model, task, &materials, &extra, state.num_ctx(), stage_sink(&app, &job_id), token_sink(&app, &job_id), cancel).await;
    state.finish_job(&job_id);
    res
}

#[derive(Serialize)]
struct QaResult {
    outcome: ChatOutcome,
    evidence: Vec<pla_core::documents::retrieve::Chunk>,
}

#[tauri::command]
async fn doc_qa(app: AppHandle, state: State<'_, AppState>, job_id: String, doc_ids: Vec<String>, question: String, history: Vec<ChatMessage>) -> AppResult<QaResult> {
    let model = state.selected_model()?;
    let materials = state.materials(&doc_ids, "")?;
    if materials.is_empty() {
        return Err(E::Other("먼저 문서를 불러와 주세요".into()));
    }
    let num_ctx = state.num_ctx();
    let qa = tasks::build_qa(&materials, &question, &user_visible_only(history), num_ctx);
    let rt = state.runtime().await?;
    let cancel = state.register_job(&job_id);
    let res = rt.chat(ChatRequest { model, messages: qa.messages, num_ctx: Some(num_ctx), temperature: Some(0.2) }, token_sink(&app, &job_id), cancel).await;
    state.finish_job(&job_id);
    Ok(QaResult { outcome: res?, evidence: qa.evidence })
}

// ---------------------------------------------------------------------------
// 학생 상담 기록
// ---------------------------------------------------------------------------

#[tauri::command]
fn add_student_note(state: State<'_, AppState>, student: String, date: String, content: String) -> AppResult<String> {
    if student.trim().is_empty() || content.trim().is_empty() {
        return Err(E::Other("학생 이름(또는 별칭)과 상담 내용을 입력해 주세요".into()));
    }
    state.store.add_student_note(student.trim(), date.trim(), content.trim())
}

#[tauri::command]
fn list_student_notes(state: State<'_, AppState>, query: Option<String>) -> AppResult<Vec<StudentNote>> {
    state.store.list_student_notes(query.as_deref())
}

#[tauri::command]
fn delete_student_note(state: State<'_, AppState>, id: String) -> AppResult<()> {
    state.store.delete_student_note(&id)
}

// ---------------------------------------------------------------------------
// Excel / CSV
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct TableInfo {
    id: String,
    file_name: String,
    sheet: Option<String>,
    sheets: Vec<String>,
    headers: Vec<String>,
    preview_rows: Vec<Vec<String>>,
    truncated: bool,
    profile: TableProfile,
}

#[tauri::command]
async fn load_table(state: State<'_, AppState>, path: String, sheet: Option<String>) -> AppResult<TableInfo> {
    let table = tauri::async_runtime::spawn_blocking(move || tabular::load(Path::new(&path), sheet.as_deref())).await.map_err(|_| E::File("파일을 읽는 중 오류가 발생했습니다".into()))??;
    let profile = tabular::profile(&table);
    let id = uuid::Uuid::new_v4().to_string();
    let info = TableInfo {
        id: id.clone(),
        file_name: table.file_name.clone(),
        sheet: table.sheet.clone(),
        sheets: table.sheets.clone(),
        headers: table.headers.clone(),
        preview_rows: table.rows.iter().take(15).cloned().collect(),
        truncated: table.truncated,
        profile,
    };
    let mut tables = state.tables.lock().unwrap();
    tables.clear(); // 한 번에 하나의 표만 메모리에 둔다
    tables.insert(id, table);
    Ok(info)
}

#[derive(Deserialize, Clone)]
struct GroupSpec {
    group_column: String,
    value_column: Option<String>,
    agg: Agg,
}

fn compute_group(state: &AppState, table_id: &str, spec: &GroupSpec) -> AppResult<GroupResult> {
    let tables = state.tables.lock().unwrap();
    let t = tables.get(table_id).ok_or_else(|| E::Other("데이터를 찾을 수 없습니다. 파일을 다시 불러와 주세요.".into()))?;
    tabular::group_by(t, &spec.group_column, spec.value_column.as_deref(), spec.agg)
}

#[tauri::command]
fn group_table(state: State<'_, AppState>, table_id: String, spec: GroupSpec) -> AppResult<GroupResult> {
    compute_group(&state, &table_id, &spec)
}

/// 코드로 계산한 통계표(Markdown)를 만든다. LLM을 거치지 않는다.
fn stats_markdown(state: &AppState, table_id: &str, spec: Option<&GroupSpec>) -> AppResult<String> {
    let mut md = {
        let tables = state.tables.lock().unwrap();
        let t = tables.get(table_id).ok_or_else(|| E::Other("데이터를 찾을 수 없습니다. 파일을 다시 불러와 주세요.".into()))?;
        tabular::profile_markdown(t, &tabular::profile(t))
    };
    if let Some(s) = spec {
        md.push_str(&tabular::group_markdown(&compute_group(state, table_id, s)?));
    }
    Ok(md)
}

#[derive(Serialize)]
struct DataReport {
    /// 프로그램이 계산한 통계표 (검증 가능)
    stats_markdown: String,
    /// AI가 작성한 해석 (통계표의 숫자만 인용)
    outcome: ChatOutcome,
}

#[tauri::command]
async fn data_report(app: AppHandle, state: State<'_, AppState>, job_id: String, table_id: String, spec: Option<GroupSpec>, focus: String) -> AppResult<DataReport> {
    let model = state.selected_model()?;
    let stats = stats_markdown(&state, &table_id, spec.as_ref())?;
    let rt = state.runtime().await?;
    let cancel = state.register_job(&job_id);
    let res = rt
        .chat(
            ChatRequest { model, messages: tasks::build_data_report_messages(&stats, &focus), num_ctx: Some(state.num_ctx()), temperature: Some(0.2) },
            token_sink(&app, &job_id),
            cancel,
        )
        .await;
    state.finish_job(&job_id);
    Ok(DataReport { stats_markdown: stats, outcome: res? })
}

// ---------------------------------------------------------------------------
// 내보내기
// ---------------------------------------------------------------------------

#[tauri::command]
async fn export_docx(title: String, markdown: String, path: String) -> AppResult<()> {
    tauri::async_runtime::spawn_blocking(move || export::markdown_to_docx(&title, &markdown, Path::new(&path))).await.map_err(|_| E::File("저장 중 오류가 발생했습니다".into()))?
}

#[tauri::command]
fn export_text(content: String, path: String) -> AppResult<()> {
    export::save_text(&content, Path::new(&path))
}

#[tauri::command]
fn export_analysis_xlsx(state: State<'_, AppState>, table_id: String, spec: Option<GroupSpec>, path: String) -> AppResult<()> {
    let group = match &spec {
        Some(s) => Some(compute_group(&state, &table_id, s)?),
        None => None,
    };
    let tables = state.tables.lock().unwrap();
    let t = tables.get(&table_id).ok_or_else(|| E::Other("데이터를 찾을 수 없습니다. 파일을 다시 불러와 주세요.".into()))?;
    export::analysis_to_xlsx(t, &tabular::profile(t), group.as_ref(), Path::new(&path))
}

// ---------------------------------------------------------------------------
// 개인정보 보호 설정
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct PrivacyInfo {
    data_dir: String,
    database_path: String,
    models_dir: String,
    key_store: KeyStoreKind,
    save_history: bool,
    download_consent: bool,
    stats: StoreStats,
    audit_log: Vec<AuditEntry>,
    loaded_documents: usize,
    disk_free: u64,
    models_bytes: u64,
}

fn dir_size(p: &Path) -> u64 {
    std::fs::read_dir(p)
        .map(|rd| rd.flatten().map(|e| if e.path().is_dir() { dir_size(&e.path()) } else { e.metadata().map(|m| m.len()).unwrap_or(0) }).sum())
        .unwrap_or(0)
}

#[tauri::command]
async fn privacy_info(state: State<'_, AppState>) -> AppResult<PrivacyInfo> {
    let models_dir = state.engine.models_dir();
    let md = models_dir.clone();
    let models_bytes = tauri::async_runtime::spawn_blocking(move || dir_size(&md)).await.unwrap_or(0);
    let disk_free = state.hardware.lock().unwrap().as_ref().map(|h| h.disk_free).unwrap_or(0);
    Ok(PrivacyInfo {
        data_dir: state.data_dir.to_string_lossy().into_owned(),
        database_path: state.store.path().to_string_lossy().into_owned(),
        models_dir: models_dir.to_string_lossy().into_owned(),
        key_store: state.key_store,
        save_history: state.store.get_bool("save_history", false),
        download_consent: state.guard.download_consent(),
        stats: state.store.stats()?,
        audit_log: state.guard.audit_log(),
        loaded_documents: state.docs.lock().unwrap().len() + state.tables.lock().unwrap().len(),
        disk_free,
        models_bytes,
    })
}

#[tauri::command]
fn set_save_history(state: State<'_, AppState>, enabled: bool) -> AppResult<()> {
    state.store.set_setting("save_history", if enabled { "true" } else { "false" })
}

#[tauri::command]
fn delete_all_conversations(state: State<'_, AppState>) -> AppResult<()> {
    state.store.delete_all_conversations()
}

/// 저장된 대화·상담 기록과 메모리에 불러온 문서를 모두 지운다 (AI 모델은 유지).
#[tauri::command]
fn delete_all_data(state: State<'_, AppState>) -> AppResult<()> {
    state.docs.lock().unwrap().clear();
    state.tables.lock().unwrap().clear();
    state.store.delete_all_user_data()
}

#[tauri::command]
fn clear_loaded_files(state: State<'_, AppState>) {
    state.docs.lock().unwrap().clear();
    state.tables.lock().unwrap().clear();
}

// ---------------------------------------------------------------------------
// 앱 시작
// ---------------------------------------------------------------------------

fn build_state(data_dir: PathBuf) -> AppResult<AppState> {
    std::fs::create_dir_all(&data_dir)?;
    let guard = Arc::new(NetGuard::new());
    let (cipher, key_store) = Cipher::load_or_create(&data_dir)?;
    let store = Arc::new(Store::open(&data_dir.join("app.db"), cipher)?);
    guard.set_download_consent(store.get_bool("download_consent", false));
    #[allow(unused_mut)]
    let mut engine = EngineManager::new(data_dir.clone(), guard.clone());
    // 개발 빌드에서만: 가짜 엔진으로 화면 흐름을 시험할 수 있다. 릴리스 빌드에는 이 코드가 없다.
    #[cfg(debug_assertions)]
    if let Ok(bin) = std::env::var("PLA_DEV_ENGINE_BIN") {
        engine = engine.with_dev_binary(PathBuf::from(bin));
    }
    Ok(AppState {
        data_dir,
        guard,
        engine: Arc::new(engine),
        store,
        catalog: Catalog::builtin(),
        key_store,
        hardware: Mutex::new(None),
        jobs: Mutex::new(HashMap::new()),
        docs: Mutex::new(HashMap::new()),
        tables: Mutex::new(HashMap::new()),
    })
}

pub fn run() {
    let app = tauri::Builder::default()
        // 앱을 두 번 실행하면 기존 창을 앞으로 가져온다 (엔진 중복 실행 방지).
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.unminimize();
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = app.path().app_local_data_dir().map_err(|_| AppError::Storage("앱 데이터 폴더를 찾을 수 없습니다".into()))?;
            let state = build_state(data_dir)?;
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_status,
            detect_hardware,
            recommend_models,
            get_catalog,
            complete_onboarding,
            set_download_consent,
            install_engine,
            start_engine,
            list_models,
            pull_model,
            cancel_job,
            delete_model,
            test_model,
            select_model,
            verify_local,
            chat_send,
            list_conversations,
            get_conversation,
            delete_conversation,
            load_document,
            remove_document,
            list_tasks,
            run_task,
            doc_qa,
            add_student_note,
            list_student_notes,
            delete_student_note,
            load_table,
            group_table,
            data_report,
            export_docx,
            export_text,
            export_analysis_xlsx,
            privacy_info,
            set_save_history,
            delete_all_conversations,
            delete_all_data,
            clear_loaded_files,
        ])
        .build(tauri::generate_context!())
        .expect("앱을 시작하지 못했습니다");

    app.run(|handle, event| {
        if let tauri::RunEvent::Exit = event {
            // 앱 종료 시 AI 엔진 프로세스를 정리한다.
            if let Some(state) = handle.try_state::<AppState>() {
                state.engine.stop_blocking();
            }
        }
    });
}
