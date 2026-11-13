use serde::Serialize;

/// 사용자에게 보여 줄 수 있는 오류.
///
/// `Display` 메시지는 한국어이며 문서 원문·개인정보를 포함하지 않는다.
/// (내부 원인 문자열은 `security::redact`를 거친 뒤에만 포함한다.)
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("보안 정책에 의해 차단되었습니다: {0}")]
    Blocked(String),
    #[error("네트워크 사용 동의가 필요합니다: {0}")]
    ConsentRequired(String),
    #[error("AI 실행 엔진 오류: {0}")]
    Runtime(String),
    #[error("네트워크 오류: {0}")]
    Network(String),
    #[error("파일을 처리할 수 없습니다: {0}")]
    File(String),
    #[error("지원하지 않는 형식입니다: {0}")]
    Unsupported(String),
    #[error("저장 공간이 부족합니다: {0}")]
    DiskSpace(String),
    #[error("저장소 오류: {0}")]
    Storage(String),
    #[error("작업이 취소되었습니다")]
    Cancelled,
    #[error("앱이 잠겨 있습니다. 잠금 암호를 입력해 주세요")]
    Locked,
    #[error("잠금 암호가 맞지 않습니다")]
    WrongPasscode,
    #[error("잠금 암호를 여러 번 틀렸습니다. {0}")]
    Throttled(String),
    #[error("{0}")]
    Other(String),
}

pub type AppResult<T> = Result<T, AppError>;

impl AppError {
    pub fn code(&self) -> &'static str {
        match self {
            AppError::Blocked(_) => "blocked",
            AppError::ConsentRequired(_) => "consent_required",
            AppError::Runtime(_) => "runtime",
            AppError::Network(_) => "network",
            AppError::File(_) => "file",
            AppError::Unsupported(_) => "unsupported",
            AppError::DiskSpace(_) => "disk_space",
            AppError::Storage(_) => "storage",
            AppError::Cancelled => "cancelled",
            AppError::Locked => "locked",
            AppError::WrongPasscode => "wrong_passcode",
            AppError::Throttled(_) => "throttled",
            AppError::Other(_) => "other",
        }
    }
}

impl Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut st = s.serialize_struct("AppError", 2)?;
        st.serialize_field("code", self.code())?;
        st.serialize_field("message", &self.to_string())?;
        st.end()
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        // io::Error 메시지에는 경로가 포함될 수 있으므로 종류만 전달한다.
        AppError::File(format!("입출력 오류 ({:?})", e.kind()))
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(_: rusqlite::Error) -> Self {
        // SQL 오류 문자열에는 바인딩 값이 섞일 수 있으므로 노출하지 않는다.
        AppError::Storage("로컬 데이터베이스 작업에 실패했습니다".into())
    }
}
