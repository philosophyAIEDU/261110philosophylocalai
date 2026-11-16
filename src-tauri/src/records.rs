//! 분석 기록 — 강의자료·연구자료·행정·학생 지도·Excel 분석 결과를 암호화해 이 앱에만 저장한다.
//!
//! 저장 위치는 대화 기록과 같은 암호화 저장소(SQLite, 필드 단위 XChaCha20-Poly1305)이며,
//! 앱이 잠겨 있으면 읽거나 쓸 수 없다. '기록 저장'을 끄면 아무것도 저장하지 않는다.
use crate::{save_history_on, AppState};
use pla_core::error::{AppError as E, AppResult};
use pla_core::security::pseudo::Alias;
use pla_core::storage::ConversationMeta;
use tauri::State;

/// 기록 종류 (메뉴)
const KINDS: &[&str] = &["chat", "lecture", "research", "admin", "student", "data"];
/// 한 기록의 최대 길이 (비정상적으로 큰 입력 방지)
const MAX_CHARS: usize = 2_000_000;

fn check_kind(kind: &str) -> AppResult<()> {
    if KINDS.contains(&kind) { Ok(()) } else { Err(E::Other("알 수 없는 기록 종류입니다".into())) }
}

fn check_len(s: &str) -> AppResult<()> {
    if s.chars().count() > MAX_CHARS { Err(E::Other("기록이 너무 깁니다".into())) } else { Ok(()) }
}

fn short_title(t: &str) -> String {
    let t: String = t.split_whitespace().collect::<Vec<_>>().join(" ");
    let s: String = t.chars().take(60).collect();
    if t.chars().count() > 60 { format!("{s}…") } else if s.is_empty() { "기록".into() } else { s }
}

/// 새 기록을 저장하고 기록 번호를 돌려준다. 기록 저장이 꺼져 있으면 저장하지 않고 None.
/// `aliases`(학생 가명 대응표)는 결과와 따로 저장해, 결과를 내보낼 때 실명이 섞이지 않게 한다.
#[tauri::command]
pub(crate) fn save_record(state: State<'_, AppState>, kind: String, title: String, request: String, result: String, aliases: Option<Vec<Alias>>) -> AppResult<Option<String>> {
    state.ensure_unlocked()?;
    check_kind(&kind)?;
    if !save_history_on(&state.store) || result.trim().is_empty() {
        return Ok(None);
    }
    check_len(&request)?;
    check_len(&result)?;
    let id = state.store.create_conversation(&kind, &short_title(&title))?;
    if !request.trim().is_empty() {
        state.store.append_message(&id, "user", &request)?;
    }
    state.store.append_message(&id, "assistant", &result)?;
    if let Some(a) = aliases.filter(|a| !a.is_empty()) {
        state.store.append_message(&id, "aliases", &serde_json::to_string(&a).unwrap_or_default())?;
    }
    Ok(Some(id))
}

/// 기존 기록에 질문·답을 이어 붙인다 (자료에 질문하기).
#[tauri::command]
pub(crate) fn append_record(state: State<'_, AppState>, id: String, request: String, result: String) -> AppResult<()> {
    state.ensure_unlocked()?;
    if !save_history_on(&state.store) {
        return Ok(());
    }
    check_len(&request)?;
    check_len(&result)?;
    state.store.append_message(&id, "user", &request)?;
    state.store.append_message(&id, "assistant", &result)
}

#[tauri::command]
pub(crate) fn list_records(state: State<'_, AppState>, kind: String) -> AppResult<Vec<ConversationMeta>> {
    state.ensure_unlocked()?;
    check_kind(&kind)?;
    state.store.list_conversations(Some(&kind))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_and_kinds() {
        assert_eq!(short_title("  강의자료   요약 · 인식론.pdf "), "강의자료 요약 · 인식론.pdf");
        assert_eq!(short_title(""), "기록");
        assert!(short_title(&"가".repeat(100)).ends_with('…'));
        assert!(check_kind("lecture").is_ok());
        assert!(check_kind("../x").is_err());
    }
}
