//! Local Storage Layer — SQLite + 필드 단위 암호화.
//!
//! - 대화 제목·메시지, 학생 이름·상담 내용은 XChaCha20-Poly1305로 암호화해 저장한다.
//! - 설정값(선택한 모델, 동의 여부 등)은 민감정보가 아니므로 평문으로 저장한다.
//! - `secure_delete`를 켜서 삭제된 행의 내용이 파일에 남지 않게 한다.
use crate::error::{AppError, AppResult};
use crate::security::crypto::Cipher;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, RwLock};

pub struct Store {
    conn: Mutex<Connection>,
    /// 잠겨 있으면 None. 잠금 시 키를 메모리에서 지운다.
    cipher: RwLock<Option<Cipher>>,
    path: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConversationMeta {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct StoredMessage {
    pub role: String,
    pub content: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct StudentNote {
    pub id: String,
    pub student: String,
    pub date: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct StoreStats {
    pub conversations: usize,
    pub messages: usize,
    pub student_notes: usize,
    pub db_bytes: u64,
}

fn now() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

impl Store {
    pub fn open(path: &Path, cipher: Cipher) -> AppResult<Self> {
        let s = Self::open_locked(path)?;
        s.unlock(cipher);
        Ok(s)
    }

    /// 키 없이(잠긴 상태로) 연다. 설정값만 읽고 쓸 수 있다.
    pub fn open_locked(path: &Path) -> AppResult<Self> {
        if let Some(p) = path.parent() {
            std::fs::create_dir_all(p)?;
        }
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "PRAGMA secure_delete = ON;
             PRAGMA journal_mode = DELETE;
             PRAGMA temp_store = MEMORY;
             PRAGMA foreign_keys = ON;
             CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS conversations (
                 id TEXT PRIMARY KEY, kind TEXT NOT NULL, title_enc TEXT NOT NULL,
                 created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS messages (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
                 role TEXT NOT NULL, content_enc TEXT NOT NULL, created_at TEXT NOT NULL);
             CREATE INDEX IF NOT EXISTS idx_messages_conv ON messages(conversation_id);
             CREATE TABLE IF NOT EXISTS student_notes (
                 id TEXT PRIMARY KEY, student_enc TEXT NOT NULL, date TEXT NOT NULL,
                 content_enc TEXT NOT NULL, created_at TEXT NOT NULL);",
        )?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
        }
        Ok(Self { conn: Mutex::new(conn), cipher: RwLock::new(None), path: path.to_path_buf() })
    }

    pub fn unlock(&self, cipher: Cipher) {
        *self.cipher.write().unwrap() = Some(cipher);
    }

    /// 키를 메모리에서 지운다. 이후 암호화된 자료는 잠금 해제 전까지 읽을 수 없다.
    pub fn lock(&self) {
        *self.cipher.write().unwrap() = None;
    }

    pub fn is_locked(&self) -> bool {
        self.cipher.read().unwrap().is_none()
    }

    /// 현재 키 (앱 잠금 암호 설정 시 감싸기 위해 사용).
    pub fn current_cipher(&self) -> AppResult<Cipher> {
        self.cipher.read().unwrap().clone().ok_or(AppError::Locked)
    }

    fn ensure_unlocked(&self) -> AppResult<()> {
        if self.is_locked() { Err(AppError::Locked) } else { Ok(()) }
    }

    fn enc(&self, s: &str) -> AppResult<String> {
        self.current_cipher()?.encrypt(s)
    }

    fn dec(&self, s: &str) -> AppResult<String> {
        self.current_cipher()?.decrypt(s)
    }

    /// 설정을 포함한 모든 기록을 지우고 파일을 정리한다 (전체 초기화용).
    /// 키 파기와 함께 쓰면, 이전에 만들어진 백업 사본도 읽을 수 없게 된다(암호학적 삭제).
    pub fn erase_all(&self) -> AppResult<()> {
        let c = self.conn.lock().unwrap();
        c.execute_batch("DELETE FROM messages; DELETE FROM conversations; DELETE FROM student_notes; DELETE FROM settings; VACUUM;")?;
        Ok(())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    // ---- 설정 ----
    pub fn get_setting(&self, key: &str) -> AppResult<Option<String>> {
        let c = self.conn.lock().unwrap();
        Ok(c.query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| r.get(0)).optional()?)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> AppResult<()> {
        let c = self.conn.lock().unwrap();
        c.execute("INSERT INTO settings(key, value) VALUES(?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value", params![key, value])?;
        Ok(())
    }

    pub fn get_bool(&self, key: &str, default: bool) -> bool {
        self.get_setting(key).ok().flatten().map(|v| v == "true").unwrap_or(default)
    }

    // ---- 대화 ----
    pub fn create_conversation(&self, kind: &str, title: &str) -> AppResult<String> {
        let id = uuid::Uuid::new_v4().to_string();
        let t = now();
        let c = self.conn.lock().unwrap();
        c.execute(
            "INSERT INTO conversations(id, kind, title_enc, created_at, updated_at) VALUES(?1, ?2, ?3, ?4, ?4)",
            params![id, kind, self.enc(title)?, t],
        )?;
        Ok(id)
    }

    pub fn rename_conversation(&self, id: &str, title: &str) -> AppResult<()> {
        let c = self.conn.lock().unwrap();
        c.execute("UPDATE conversations SET title_enc = ?2 WHERE id = ?1", params![id, self.enc(title)?])?;
        Ok(())
    }

    pub fn append_message(&self, conversation_id: &str, role: &str, content: &str) -> AppResult<()> {
        let t = now();
        let c = self.conn.lock().unwrap();
        let n = c.execute(
            "INSERT INTO messages(conversation_id, role, content_enc, created_at) VALUES(?1, ?2, ?3, ?4)",
            params![conversation_id, role, self.enc(content)?, t],
        )?;
        if n == 0 {
            return Err(AppError::Storage("대화를 찾을 수 없습니다".into()));
        }
        c.execute("UPDATE conversations SET updated_at = ?2 WHERE id = ?1", params![conversation_id, t])?;
        Ok(())
    }

    pub fn list_conversations(&self, kind: Option<&str>) -> AppResult<Vec<ConversationMeta>> {
        self.ensure_unlocked()?;
        let c = self.conn.lock().unwrap();
        let mut st = c.prepare("SELECT id, kind, title_enc, updated_at FROM conversations WHERE (?1 IS NULL OR kind = ?1) ORDER BY updated_at DESC, rowid DESC")?;
        let rows = st.query_map([kind], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?)))?;
        let mut out = Vec::new();
        for row in rows {
            let (id, kind, title_enc, updated_at) = row?;
            out.push(ConversationMeta { id, kind, title: self.dec(&title_enc)?, updated_at });
        }
        Ok(out)
    }

    pub fn get_messages(&self, conversation_id: &str) -> AppResult<Vec<StoredMessage>> {
        self.ensure_unlocked()?;
        let c = self.conn.lock().unwrap();
        let mut st = c.prepare("SELECT role, content_enc, created_at FROM messages WHERE conversation_id = ?1 ORDER BY id")?;
        let rows = st.query_map([conversation_id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)))?;
        let mut out = Vec::new();
        for row in rows {
            let (role, enc, created_at) = row?;
            out.push(StoredMessage { role, content: self.dec(&enc)?, created_at });
        }
        Ok(out)
    }

    pub fn delete_conversation(&self, id: &str) -> AppResult<()> {
        self.ensure_unlocked()?;
        let c = self.conn.lock().unwrap();
        c.execute("DELETE FROM conversations WHERE id = ?1", [id])?;
        Ok(())
    }

    // ---- 학생 상담 기록 ----
    pub fn add_student_note(&self, student: &str, date: &str, content: &str) -> AppResult<String> {
        let id = uuid::Uuid::new_v4().to_string();
        let c = self.conn.lock().unwrap();
        c.execute(
            "INSERT INTO student_notes(id, student_enc, date, content_enc, created_at) VALUES(?1, ?2, ?3, ?4, ?5)",
            params![id, self.enc(student)?, date, self.enc(content)?, now()],
        )?;
        Ok(id)
    }

    /// 상담 기록을 불러온다. `query`가 있으면 학생 이름·내용에서 검색한다
    /// (복호화는 메모리에서만 이루어지며 검색 색인을 디스크에 만들지 않는다).
    pub fn list_student_notes(&self, query: Option<&str>) -> AppResult<Vec<StudentNote>> {
        self.ensure_unlocked()?;
        let c = self.conn.lock().unwrap();
        let mut st = c.prepare("SELECT id, student_enc, date, content_enc FROM student_notes ORDER BY date DESC, created_at DESC")?;
        let rows = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?)))?;
        let q = query.map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty());
        let mut out = Vec::new();
        for row in rows {
            let (id, s_enc, date, c_enc) = row?;
            let note = StudentNote { id, student: self.dec(&s_enc)?, date, content: self.dec(&c_enc)? };
            if let Some(q) = &q {
                if !note.student.to_lowercase().contains(q) && !note.content.to_lowercase().contains(q) {
                    continue;
                }
            }
            out.push(note);
        }
        Ok(out)
    }

    pub fn delete_student_note(&self, id: &str) -> AppResult<()> {
        self.ensure_unlocked()?;
        let c = self.conn.lock().unwrap();
        c.execute("DELETE FROM student_notes WHERE id = ?1", [id])?;
        Ok(())
    }

    // ---- 삭제·통계 ----
    /// 모든 대화와 상담 기록을 지우고 파일을 정리(VACUUM)한다.
    pub fn delete_all_user_data(&self) -> AppResult<()> {
        let c = self.conn.lock().unwrap();
        c.execute_batch("DELETE FROM messages; DELETE FROM conversations; DELETE FROM student_notes; VACUUM;")?;
        Ok(())
    }

    pub fn delete_all_conversations(&self) -> AppResult<()> {
        let c = self.conn.lock().unwrap();
        c.execute_batch("DELETE FROM messages; DELETE FROM conversations; VACUUM;")?;
        Ok(())
    }

    pub fn stats(&self) -> AppResult<StoreStats> {
        let c = self.conn.lock().unwrap();
        let count = |t: &str| -> AppResult<usize> { Ok(c.query_row(&format!("SELECT COUNT(*) FROM {t}"), [], |r| r.get::<_, i64>(0))? as usize) };
        Ok(StoreStats {
            conversations: count("conversations")?,
            messages: count("messages")?,
            student_notes: count("student_notes")?,
            db_bytes: std::fs::metadata(&self.path).map(|m| m.len()).unwrap_or(0),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (tempfile::TempDir, Store) {
        let d = tempfile::tempdir().unwrap();
        let s = Store::open(&d.path().join("app.db"), Cipher::generate()).unwrap();
        (d, s)
    }

    fn file_contains(path: &Path, needle: &str) -> bool {
        let dir = path.parent().unwrap();
        std::fs::read_dir(dir).unwrap().flatten().any(|e| {
            let bytes = std::fs::read(e.path()).unwrap_or_default();
            bytes.windows(needle.len()).any(|w| w == needle.as_bytes())
        })
    }

    #[test]
    fn conversations_roundtrip_encrypted_at_rest() {
        let (_d, s) = store();
        let id = s.create_conversation("chat", "김민수 학생 진로 상담").unwrap();
        s.append_message(&id, "user", "학번 20231234 김민수의 성적을 요약해줘").unwrap();
        s.append_message(&id, "assistant", "요약입니다").unwrap();
        let list = s.list_conversations(Some("chat")).unwrap();
        assert_eq!(list[0].title, "김민수 학생 진로 상담");
        assert_eq!(s.get_messages(&id).unwrap().len(), 2);
        assert!(s.list_conversations(Some("lecture")).unwrap().is_empty());
        // 디스크의 어떤 파일에도 평문이 없어야 한다
        assert!(!file_contains(s.path(), "김민수"));
        assert!(!file_contains(s.path(), "20231234"));
    }

    #[test]
    fn delete_leaves_no_rows_and_no_plaintext() {
        let (_d, s) = store();
        let id = s.create_conversation("chat", "삭제될 대화").unwrap();
        s.append_message(&id, "user", "비밀 내용 SECRET-CANARY-12345").unwrap();
        let before_size = s.stats().unwrap();
        assert_eq!(before_size.messages, 1);
        s.delete_conversation(&id).unwrap();
        let st = s.stats().unwrap();
        assert_eq!((st.conversations, st.messages), (0, 0)); // CASCADE 확인
        assert!(!file_contains(s.path(), "SECRET-CANARY-12345"));
    }

    #[test]
    fn deleted_ciphertext_is_scrubbed_from_file() {
        let (_d, s) = store();
        let id = s.create_conversation("chat", "t").unwrap();
        s.append_message(&id, "user", &"가나다라".repeat(500)).unwrap();
        // 저장된 암호문 일부를 표식으로 삼는다
        let marker: String = {
            let c = s.conn.lock().unwrap();
            let enc: String = c.query_row("SELECT content_enc FROM messages LIMIT 1", [], |r| r.get(0)).unwrap();
            enc[20..60].to_string()
        };
        assert!(file_contains(s.path(), &marker));
        s.delete_all_user_data().unwrap();
        assert!(!file_contains(s.path(), &marker), "삭제 후 암호문 잔여 데이터가 남아 있음");
    }

    #[test]
    fn student_notes_search() {
        let (_d, s) = store();
        s.add_student_note("이서연", "2026-03-02", "진로 고민, 대학원 진학 희망").unwrap();
        s.add_student_note("박준호", "2026-03-05", "수업 결석이 잦음, 아르바이트 병행").unwrap();
        assert_eq!(s.list_student_notes(None).unwrap().len(), 2);
        assert_eq!(s.list_student_notes(Some("이서연")).unwrap().len(), 1);
        assert_eq!(s.list_student_notes(Some("결석")).unwrap()[0].student, "박준호");
        assert!(s.list_student_notes(Some("없는말")).unwrap().is_empty());
        assert!(!file_contains(s.path(), "이서연"));
        let id = s.list_student_notes(Some("박준호")).unwrap()[0].id.clone();
        s.delete_student_note(&id).unwrap();
        assert_eq!(s.stats().unwrap().student_notes, 1);
    }

    #[test]
    fn locked_store_refuses_sensitive_data_but_allows_settings() {
        let (_d, s) = store();
        s.create_conversation("chat", "상담").unwrap();
        let key = s.current_cipher().unwrap();
        s.lock();
        assert!(s.is_locked());
        assert!(matches!(s.list_conversations(None), Err(AppError::Locked)));
        assert!(matches!(s.add_student_note("a", "2026-01-01", "b"), Err(AppError::Locked)));
        assert!(matches!(s.list_student_notes(None), Err(AppError::Locked)));
        s.set_setting("k", "v").unwrap();
        assert_eq!(s.get_setting("k").unwrap().as_deref(), Some("v"));
        s.unlock(key);
        assert_eq!(s.list_conversations(None).unwrap().len(), 1);
    }

    #[test]
    fn erase_all_with_new_key_leaves_nothing_readable() {
        let (_d, s) = store();
        s.add_student_note("김학생", "2026-01-01", "ERASE-CANARY").unwrap();
        s.set_setting("onboarding_done", "true").unwrap();
        s.erase_all().unwrap();
        assert_eq!(s.stats().unwrap().student_notes, 0);
        assert!(s.get_setting("onboarding_done").unwrap().is_none());
        assert!(!file_contains(s.path(), "ERASE-CANARY"));
    }

    #[test]
    fn settings_and_wrong_key() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("app.db");
        {
            let s = Store::open(&p, Cipher::generate()).unwrap();
            s.set_setting("selected_model", "gemma3:4b").unwrap();
            s.set_setting("selected_model", "qwen3:8b").unwrap();
            assert_eq!(s.get_setting("selected_model").unwrap().as_deref(), Some("qwen3:8b"));
            assert!(!s.get_bool("save_history", false));
            s.create_conversation("chat", "x").unwrap();
        }
        // 다른 키로 열면 내용을 읽을 수 없다
        let s2 = Store::open(&p, Cipher::generate()).unwrap();
        assert!(s2.list_conversations(None).is_err());
    }
}
