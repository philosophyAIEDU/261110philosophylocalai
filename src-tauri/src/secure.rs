//! 보안 기능 명령: 앱 잠금, 전체 초기화, 보안 점검, 파일 선택/저장, 가명 처리, 업데이트.
//!
//! 파일 경로 원칙: 화면(웹뷰)은 파일 경로를 지정할 수 없다. 파일을 열고 저장할 때는
//! 여기서 운영체제 대화상자를 직접 띄우고, 교수님이 고른 경로만 사용한다.
//! 화면에는 파일 이름만 돌려준다.
use crate::{compute_group, load_document_path, load_table_path, now_secs, stage_sink, token_sink, AppState, DocInfo, GroupSpec, TableInfo};
use pla_core::error::{AppError as E, AppResult};
use pla_core::export;
use pla_core::runtime::ChatOutcome;
use pla_core::security::keystore::{KeyState, KeyStoreKind};
use pla_core::security::posture::{self, DiskEncryption};
use pla_core::security::pseudo::{self, Alias};
use pla_core::storage::Store;
use pla_core::tabular;
use pla_core::tasks;
use pla_core::update::{self, UpdateInfo};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;

pub(crate) const DEFAULT_AUTO_LOCK_MINUTES: u32 = 10;

pub(crate) fn auto_lock_minutes(store: &Store) -> u32 {
    store.get_setting("auto_lock_minutes").ok().flatten().and_then(|v| v.parse().ok()).unwrap_or(DEFAULT_AUTO_LOCK_MINUTES)
}

/// 자동 잠금 감시: 잠금 암호가 있고, 정해진 시간 동안 사용하지 않으면 잠근다.
pub(crate) fn spawn_auto_lock(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(15)).await;
            let Some(state) = app.try_state::<AppState>() else { continue };
            let minutes = auto_lock_minutes(&state.store) as u64;
            let has_pass = state.store.get_bool("has_passcode", false);
            if !has_pass || minutes == 0 || state.store.is_locked() {
                continue;
            }
            let idle = now_secs().saturating_sub(state.last_activity.load(Ordering::Relaxed));
            if idle >= minutes * 60 {
                state.lock_now(&app);
            }
        }
    });
}

// ---------------------------------------------------------------------------
// 앱 잠금
// ---------------------------------------------------------------------------

#[tauri::command]
pub(crate) async fn unlock_app(state: State<'_, AppState>, passcode: String) -> AppResult<()> {
    // (async 명령은 화면 스레드가 아닌 작업 스레드에서 실행되므로 Argon2 계산이 화면을 멈추지 않는다)
    let st = state.inner();
    let cipher = st.keystore.open_with_passcode(&passcode)?;
    st.store.unlock(cipher);
    st.touch();
    Ok(())
}

#[tauri::command]
pub(crate) fn lock_app(app: AppHandle, state: State<'_, AppState>) -> AppResult<()> {
    if state.keystore.state()? != KeyState::Passcode {
        return Err(E::Other("잠금 암호를 먼저 설정해 주세요".into()));
    }
    state.lock_now(&app);
    Ok(())
}

/// 화면에서 키보드·마우스 사용이 있을 때 호출 (자동 잠금 시간 기준).
#[tauri::command]
pub(crate) fn user_activity(state: State<'_, AppState>) {
    if !state.store.is_locked() {
        state.touch();
    }
}

#[tauri::command]
pub(crate) async fn set_passcode(state: State<'_, AppState>, current: Option<String>, new_passcode: String) -> AppResult<()> {
    let st = state.inner();
    let cipher = match st.keystore.state()? {
        KeyState::Passcode => {
            let cur = current.ok_or_else(|| E::Other("현재 잠금 암호를 입력해 주세요".into()))?;
            st.keystore.open_with_passcode(&cur)?
        }
        _ => st.store.current_cipher()?,
    };
    st.keystore.set_passcode(&cipher, &new_passcode)?;
    st.store.set_setting("has_passcode", "true")?;
    st.touch();
    Ok(())
}

#[tauri::command]
pub(crate) async fn remove_passcode(state: State<'_, AppState>, current: String) -> AppResult<()> {
    let st = state.inner();
    let cipher = st.keystore.open_with_passcode(&current)?;
    st.keystore.remove_passcode(&cipher)?;
    st.store.set_setting("has_passcode", "false")?;
    Ok(())
}

#[tauri::command]
pub(crate) fn set_auto_lock(state: State<'_, AppState>, minutes: u32) -> AppResult<()> {
    if ![0, 3, 5, 10, 15, 30, 60].contains(&minutes) {
        return Err(E::Other("지원하지 않는 시간입니다".into()));
    }
    state.store.set_setting("auto_lock_minutes", &minutes.to_string())
}

#[tauri::command]
pub(crate) fn set_pseudonymize(state: State<'_, AppState>, enabled: bool) -> AppResult<()> {
    state.store.set_setting("pseudonymize_students", if enabled { "true" } else { "false" })
}

/// 전체 초기화: 암호화 키를 파기하고(백업 사본까지 읽을 수 없게 됨) 모든 기록과 설정을 지운다.
/// 내려받은 AI 실행 엔진과 AI 모델은 개인정보가 아니므로 남긴다.
#[tauri::command]
pub(crate) fn factory_reset(app: AppHandle, state: State<'_, AppState>, confirm: String) -> AppResult<()> {
    if confirm.trim() != "초기화" {
        return Err(E::Other("확인 문구로 '초기화'를 정확히 입력해 주세요".into()));
    }
    state.lock_now(&app);
    state.keystore.destroy()?;
    state.store.erase_all()?;
    state.store.unlock(state.keystore.create_plain()?);
    state.guard.set_download_consent(false);
    state.guard.set_update_consent(false);
    *state.update_asset.lock().unwrap() = None;
    if let Some(p) = state.update_file.lock().unwrap().take() {
        let _ = std::fs::remove_file(p);
    }
    state.touch();
    Ok(())
}

#[derive(Serialize)]
pub(crate) struct SecurityCheckup {
    disk_encryption: DiskEncryption,
    key_store: KeyStoreKind,
    has_passcode: bool,
    auto_lock_minutes: u32,
    save_history: bool,
    pseudonymize_students: bool,
    download_consent: bool,
    update_consent: bool,
}

#[tauri::command]
pub(crate) async fn security_checkup(state: State<'_, AppState>) -> AppResult<SecurityCheckup> {
    let disk = tauri::async_runtime::spawn_blocking(posture::disk_encryption).await.unwrap_or(DiskEncryption::Unknown);
    Ok(SecurityCheckup {
        disk_encryption: disk,
        key_store: state.keystore.kind(),
        has_passcode: state.keystore.state()? == KeyState::Passcode,
        auto_lock_minutes: auto_lock_minutes(&state.store),
        save_history: state.store.get_bool("save_history", false),
        pseudonymize_students: state.store.get_bool("pseudonymize_students", true),
        download_consent: state.guard.download_consent(),
        update_consent: state.guard.update_consent(),
    })
}

// ---------------------------------------------------------------------------
// 파일 열기·저장 (운영체제 대화상자를 Rust에서 직접 띄움)
// ---------------------------------------------------------------------------

async fn pick_path(app: &AppHandle, title: &str, filter_name: &str, exts: &'static [&'static str]) -> AppResult<Option<PathBuf>> {
    let dialog = app.dialog().file().set_title(title).add_filter(filter_name, exts);
    let picked = tauri::async_runtime::spawn_blocking(move || dialog.blocking_pick_file()).await.map_err(|_| E::Other("파일 선택 창을 열지 못했습니다".into()))?;
    Ok(picked.and_then(|p| p.into_path().ok()))
}

/// 저장 기본 이름에서 경로 구분자·특수문자를 없앤다.
fn clean_file_name(name: &str, ext: &str) -> String {
    let base: String = name.chars().filter(|c| !matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') && !c.is_control()).collect();
    let base = base.trim().trim_matches('.').chars().take(80).collect::<String>();
    let base = if base.is_empty() { "결과".to_string() } else { base };
    if base.to_lowercase().ends_with(&format!(".{ext}")) { base } else { format!("{base}.{ext}") }
}

async fn save_path(app: &AppHandle, default_name: &str, filter_name: &str, ext: &'static str) -> AppResult<Option<PathBuf>> {
    let dialog = app.dialog().file().set_title("저장할 위치 선택").set_file_name(clean_file_name(default_name, ext)).add_filter(filter_name, &[ext]);
    let picked = tauri::async_runtime::spawn_blocking(move || dialog.blocking_save_file()).await.map_err(|_| E::Other("저장 창을 열지 못했습니다".into()))?;
    Ok(picked.and_then(|p| p.into_path().ok()).map(|p| if p.extension().is_none() { p.with_extension(ext) } else { p }))
}

fn file_name_of(p: &Path) -> String {
    p.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}

#[tauri::command]
pub(crate) async fn pick_document(app: AppHandle, state: State<'_, AppState>) -> AppResult<Option<DocInfo>> {
    state.ensure_unlocked()?;
    match pick_path(&app, "분석할 문서 선택", "문서 (PDF, Word, PowerPoint, 텍스트)", &["pdf", "docx", "pptx", "txt", "md"]).await? {
        Some(p) => Ok(Some(load_document_path(state.inner(), p).await?)),
        None => Ok(None),
    }
}

#[tauri::command]
pub(crate) async fn pick_table(app: AppHandle, state: State<'_, AppState>) -> AppResult<Option<TableInfo>> {
    state.ensure_unlocked()?;
    match pick_path(&app, "분석할 표 파일 선택", "표 데이터 (Excel, CSV)", &["xlsx", "xls", "xlsm", "csv", "tsv"]).await? {
        Some(p) => Ok(Some(load_table_path(state.inner(), p, None).await?)),
        None => Ok(None),
    }
}

#[tauri::command]
pub(crate) async fn reload_table_sheet(state: State<'_, AppState>, sheet: String) -> AppResult<TableInfo> {
    state.ensure_unlocked()?;
    let path = state.last_table_path.lock().unwrap().clone().ok_or_else(|| E::Other("파일을 다시 불러와 주세요".into()))?;
    load_table_path(state.inner(), path, Some(sheet)).await
}

/// 저장한 파일 이름을 돌려준다 (취소하면 None).
#[tauri::command]
pub(crate) async fn save_docx(app: AppHandle, state: State<'_, AppState>, title: String, markdown: String) -> AppResult<Option<String>> {
    state.ensure_unlocked()?;
    let Some(path) = save_path(&app, &title, "Word 문서", "docx").await? else { return Ok(None) };
    let p2 = path.clone();
    tauri::async_runtime::spawn_blocking(move || export::markdown_to_docx(&title, &markdown, &p2)).await.map_err(|_| E::File("저장 중 오류가 발생했습니다".into()))??;
    Ok(Some(file_name_of(&path)))
}

#[tauri::command]
pub(crate) async fn save_text(app: AppHandle, state: State<'_, AppState>, title: String, content: String) -> AppResult<Option<String>> {
    state.ensure_unlocked()?;
    let Some(path) = save_path(&app, &title, "텍스트 문서", "md").await? else { return Ok(None) };
    export::save_text(&content, &path)?;
    Ok(Some(file_name_of(&path)))
}

#[tauri::command]
pub(crate) async fn save_analysis_xlsx(app: AppHandle, state: State<'_, AppState>, table_id: String, spec: Option<GroupSpec>) -> AppResult<Option<String>> {
    state.ensure_unlocked()?;
    let default = state.tables.lock().unwrap().get(&table_id).map(|t| format!("{}_분석결과", t.file_name.rsplit_once('.').map(|x| x.0).unwrap_or(&t.file_name))).unwrap_or_else(|| "분석결과".into());
    let group = match &spec {
        Some(s) => Some(compute_group(state.inner(), &table_id, s)?),
        None => None,
    };
    let Some(path) = save_path(&app, &default, "Excel 파일", "xlsx").await? else { return Ok(None) };
    {
        let tables = state.tables.lock().unwrap();
        let t = tables.get(&table_id).ok_or_else(|| E::Other("데이터를 찾을 수 없습니다. 파일을 다시 불러와 주세요.".into()))?;
        export::analysis_to_xlsx(t, &tabular::profile(t), group.as_ref(), &path)?;
    }
    Ok(Some(file_name_of(&path)))
}

// ---------------------------------------------------------------------------
// 학생 지도: 저장된 기록을 Rust에서 직접 읽어 가명 처리 후 AI에 전달
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub(crate) struct StudentTaskResult {
    outcome: ChatOutcome,
    /// 가명 대응표 (화면에만 표시, AI에는 보내지 않음)
    aliases: Vec<Alias>,
}

#[tauri::command]
pub(crate) async fn run_student_task(app: AppHandle, state: State<'_, AppState>, job_id: String, task_id: String, note_ids: Vec<String>, pseudonymize: bool) -> AppResult<StudentTaskResult> {
    state.ensure_unlocked()?;
    let task = tasks::find_task(&task_id).filter(|t| t.category == "student").ok_or_else(|| E::Other("알 수 없는 작업입니다".into()))?;
    let notes: Vec<_> = state.store.list_student_notes(None)?.into_iter().filter(|n| note_ids.contains(&n.id)).collect();
    if notes.is_empty() {
        return Err(E::Other("상담 기록을 선택해 주세요".into()));
    }
    let records: Vec<(String, String)> = notes.iter().map(|n| (n.student.clone(), n.content.clone())).collect();
    let (records, aliases) = if pseudonymize { pseudo::pseudonymize(&records) } else { (records, vec![]) };
    let pasted = notes
        .iter()
        .zip(records.iter())
        .map(|(n, (who, body))| format!("[{}] {}\n{}", n.date, who, body))
        .collect::<Vec<_>>()
        .join("\n\n---\n\n");
    let model = state.selected_model()?;
    let rt = state.runtime().await?;
    let cancel = state.register_job(&job_id);
    let res = tasks::run_task(
        &rt,
        &model,
        task,
        &[tasks::Material { name: "상담 기록".into(), text: pasted }],
        "",
        state.num_ctx(),
        stage_sink(&app, &job_id),
        token_sink(&app, &job_id),
        cancel,
    )
    .await;
    state.finish_job(&job_id);
    Ok(StudentTaskResult { outcome: res?, aliases })
}

// ---------------------------------------------------------------------------
// 업데이트
// ---------------------------------------------------------------------------

#[tauri::command]
pub(crate) fn set_update_consent(state: State<'_, AppState>, consent: bool) -> AppResult<()> {
    state.guard.set_update_consent(consent);
    state.store.set_setting("update_consent", if consent { "true" } else { "false" })
}

#[tauri::command]
pub(crate) fn set_include_prerelease(state: State<'_, AppState>, enabled: bool) -> AppResult<()> {
    state.store.set_setting("include_prerelease", if enabled { "true" } else { "false" })
}

#[tauri::command]
pub(crate) async fn check_update(state: State<'_, AppState>) -> AppResult<UpdateInfo> {
    let include_pre = state.store.get_bool("include_prerelease", true);
    let info = update::check(&state.guard, env!("CARGO_PKG_VERSION"), include_pre).await?;
    *state.update_asset.lock().unwrap() = if info.update_available { info.asset.clone() } else { None };
    Ok(info)
}

/// 확인한 업데이트의 설치 파일을 내려받아 무결성을 확인한다. 화면은 주소를 지정할 수 없다.
#[tauri::command]
pub(crate) async fn download_update(app: AppHandle, state: State<'_, AppState>, job_id: String) -> AppResult<String> {
    let asset = state.update_asset.lock().unwrap().clone().ok_or_else(|| E::Other("먼저 업데이트를 확인해 주세요".into()))?;
    let dir = state.data_dir.join("updates");
    let cancel = state.register_job(&job_id);
    let jid = job_id.clone();
    let res = update::download(
        &state.guard,
        &asset,
        &dir,
        move |p| {
            let _ = app.emit("update-progress", serde_json::json!({ "job_id": jid, "progress": p }));
        },
        &cancel,
    )
    .await;
    state.finish_job(&job_id);
    let path = res?;
    let name = file_name_of(&path);
    *state.update_file.lock().unwrap() = Some(path);
    Ok(name)
}

fn verified_installer(state: &AppState) -> AppResult<PathBuf> {
    let p = state.update_file.lock().unwrap().clone().ok_or_else(|| E::Other("먼저 새 버전을 내려받아 주세요".into()))?;
    let dir = state.data_dir.join("updates");
    if !p.starts_with(&dir) || !p.is_file() {
        return Err(E::Other("내려받은 설치 파일을 찾을 수 없습니다. 다시 내려받아 주세요".into()));
    }
    Ok(p)
}

fn spawn_detached(cmd: &str, args: &[&std::ffi::OsStr]) -> AppResult<()> {
    let mut c = std::process::Command::new(cmd);
    c.args(args).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    c.spawn().map(|_| ()).map_err(|_| E::Other("프로그램을 실행하지 못했습니다".into()))
}

/// 설치 파일을 실행한다. Windows에서는 설치가 앱 파일을 바꿀 수 있도록 앱을 종료한다.
#[tauri::command]
pub(crate) fn open_installer(app: AppHandle, state: State<'_, AppState>) -> AppResult<()> {
    let p = verified_installer(state.inner())?;
    #[cfg(target_os = "windows")]
    {
        let mut c = std::process::Command::new(&p);
        c.spawn().map_err(|_| E::Other("설치 프로그램을 실행하지 못했습니다".into()))?;
        let handle = app.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(Duration::from_millis(800)).await;
            handle.exit(0);
        });
        Ok(())
    }
    #[cfg(target_os = "macos")]
    {
        let _ = app;
        spawn_detached("open", &[p.as_os_str()])
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = app;
        spawn_detached("xdg-open", &[p.as_os_str()])
    }
}

#[tauri::command]
pub(crate) fn reveal_installer(state: State<'_, AppState>) -> AppResult<()> {
    let p = verified_installer(state.inner())?;
    #[cfg(target_os = "windows")]
    {
        let arg = std::ffi::OsString::from(format!("/select,{}", p.display()));
        spawn_detached("explorer.exe", &[arg.as_os_str()])
    }
    #[cfg(target_os = "macos")]
    {
        spawn_detached("open", &[std::ffi::OsStr::new("-R"), p.as_os_str()])
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        spawn_detached("xdg-open", &[p.parent().unwrap_or(&p).as_os_str()])
    }
}

/// 공식 릴리스 페이지를 기본 브라우저로 연다 (고정된 주소만).
#[tauri::command]
pub(crate) fn open_release_page() -> AppResult<()> {
    let url = update::releases_page_url();
    let u = std::ffi::OsString::from(&url);
    #[cfg(target_os = "windows")]
    {
        spawn_detached("rundll32.exe", &[std::ffi::OsStr::new("url.dll,FileProtocolHandler"), u.as_os_str()])
    }
    #[cfg(target_os = "macos")]
    {
        spawn_detached("open", &[u.as_os_str()])
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        spawn_detached("xdg-open", &[u.as_os_str()])
    }
}

#[cfg(test)]
mod tests {
    use super::clean_file_name;

    #[test]
    fn save_names_cannot_escape_folder() {
        assert_eq!(clean_file_name("../../etc/passwd", "docx"), "etcpasswd.docx");
        assert_eq!(clean_file_name("C:\\Windows\\x", "md"), "CWindowsx.md");
        assert_eq!(clean_file_name("회의록 요약", "docx"), "회의록 요약.docx");
        assert_eq!(clean_file_name("결과.docx", "docx"), "결과.docx");
        assert_eq!(clean_file_name("...", "xlsx"), "결과.xlsx");
    }
}
