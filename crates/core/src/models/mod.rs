//! Model Recommendation Layer — 모델 카탈로그와 추천 엔진.
pub mod recommend;

use crate::error::{AppError, AppResult};
use crate::security::netguard::is_cloud_model_name;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogModel {
    /// Ollama 모델 이름:태그 (예: `gemma3:4b`)
    pub id: String,
    pub family: String,
    pub publisher: String,
    pub display_name: String,
    /// 파라미터 수 (십억 단위)
    pub params_b: f64,
    pub quantization: String,
    /// 다운로드 용량 (GB, 근사)
    pub download_gb: f64,
    /// 8K 컨텍스트 기준 예상 실행 메모리 (GB, 근사)
    pub est_ram_gb: f64,
    /// 모델이 지원하는 최대 컨텍스트 (천 토큰)
    pub context_k: u32,
    pub license: String,
    pub license_url: String,
    /// 편집 추정 점수 1~5 (벤치마크 검증값 아님)
    pub korean: u8,
    pub summarization: u8,
    pub reasoning: u8,
    /// "생각(thinking)" 기능이 있는 모델 — 빠른 응답을 위해 기본적으로 끈다
    pub thinking: bool,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Catalog {
    pub catalog_version: String,
    pub note: String,
    pub models: Vec<CatalogModel>,
}

impl Catalog {
    /// 앱에 내장된 카탈로그. 원격에서 불러오지 않는다.
    pub fn builtin() -> Self {
        serde_json::from_str(include_str!("catalog.json")).expect("내장 카탈로그 JSON 오류")
    }

    pub fn get(&self, id: &str) -> Option<&CatalogModel> {
        self.models.iter().find(|m| m.id == id)
    }

    pub fn allowed_ids(&self) -> Vec<String> {
        self.models.iter().map(|m| m.id.clone()).collect()
    }
}

/// ollama.com 공식 목록에서 고른 '둘러보기' 모델 (앱 내장, 인터넷에서 불러오지 않음)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryModel {
    pub id: String,
    pub display_name: String,
    pub publisher: String,
    /// 다운로드 용량 (GB, 근사)
    pub download_gb: f64,
    /// 예상 실행 메모리 (GB, 근사)
    pub est_ram_gb: f64,
    pub license: String,
    pub tags: Vec<String>,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Library {
    pub list_version: String,
    pub note: String,
    pub models: Vec<LibraryModel>,
}

impl Library {
    pub fn builtin() -> Self {
        serde_json::from_str(include_str!("library.json")).expect("내장 모델 목록 JSON 오류")
    }
}

/// 사용자가 직접 추가할 수 있는 모델 수 상한
pub const MAX_CUSTOM_MODELS: usize = 30;

fn valid_part(s: &str) -> bool {
    (1..=80).contains(&s.len())
        && s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"._-".contains(&b))
        && s.as_bytes()[0].is_ascii_alphanumeric()
        && !s.contains("..")
}

/// 사용자가 입력한 모델 이름을 검사하고 `모델:태그` 형태로 바꾼다.
///
/// - ollama.com 공식 라이브러리 모델만 허용한다 (`qwen3:8b`, `exaone3.5:7.8b` 등).
///   사용자 이름공간(`user/model`), 다른 저장소 주소(`hf.co/...`), 클라우드 모델(`-cloud`)은 거부한다.
///   → 엔진이 공식 모델 저장소(registry.ollama.ai) 외의 곳에 접속하지 않게 하기 위함.
/// - ollama.com 페이지에서 복사한 `ollama run qwen3:8b`, `https://ollama.com/library/qwen3:8b`도 받아 준다.
/// - 태그가 없으면 `latest`를 붙인다.
pub fn normalize_custom_model(input: &str) -> AppResult<String> {
    let mut s = input.trim().to_ascii_lowercase();
    for prefix in ["ollama run ", "ollama pull "] {
        if let Some(rest) = s.strip_prefix(prefix) {
            s = rest.trim().to_string();
        }
    }
    for prefix in ["https://ollama.com/library/", "http://ollama.com/library/", "ollama.com/library/", "https://www.ollama.com/library/"] {
        if let Some(rest) = s.strip_prefix(prefix) {
            s = rest.trim_end_matches('/').to_string();
        }
    }
    let bad = || AppError::Blocked("ollama.com 공식 목록의 모델 이름만 추가할 수 있습니다 (예: qwen3:8b)".into());
    if s.contains('/') || s.contains('\\') || s.contains("://") || s.contains('@') || s.chars().any(|c| c.is_whitespace()) {
        return Err(bad());
    }
    let (model, tag) = match s.split_once(':') {
        Some((m, t)) => (m.to_string(), t.to_string()),
        None => (s.clone(), "latest".to_string()),
    };
    if !valid_part(&model) || !valid_part(&tag) {
        return Err(bad());
    }
    let name = format!("{model}:{tag}");
    if is_cloud_model_name(&name) || tag.contains("cloud") {
        return Err(AppError::Blocked("클라우드 모델은 이 컴퓨터에서 실행되지 않으므로 추가할 수 없습니다".into()));
    }
    Ok(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_library_is_safe_and_distinct_from_catalog() {
        let lib = Library::builtin();
        let cat = Catalog::builtin();
        assert!(lib.models.len() >= 20);
        let mut ids = std::collections::HashSet::new();
        for m in &lib.models {
            assert!(ids.insert(m.id.clone()), "중복 {}", m.id);
            // 목록의 모든 이름은 '직접 추가' 검사를 그대로 통과해야 한다 (클라우드·다른 저장소 없음)
            assert_eq!(normalize_custom_model(&m.id).unwrap(), m.id);
            assert!(cat.get(&m.id).is_none(), "추천 목록과 중복: {}", m.id);
            assert!(m.download_gb > 0.0 && m.est_ram_gb > m.download_gb, "{}", m.id);
            assert!(!m.license.is_empty() && !m.publisher.is_empty() && !m.description.is_empty());
        }
    }

    #[test]
    fn custom_model_names_are_normalized_and_restricted() {
        for (input, want) in [
            ("qwen3:8b", "qwen3:8b"),
            ("  Qwen3:8B ", "qwen3:8b"),
            ("llama3.2", "llama3.2:latest"),
            ("ollama run exaone3.5:7.8b", "exaone3.5:7.8b"),
            ("ollama pull mistral-small3.2:24b", "mistral-small3.2:24b"),
            ("https://ollama.com/library/gemma3:12b-it-qat", "gemma3:12b-it-qat"),
            ("ollama.com/library/phi4/", "phi4:latest"),
        ] {
            assert_eq!(normalize_custom_model(input).unwrap(), want, "{input}");
        }
        for bad in [
            "", ":8b", "qwen3:", "user/model:1b", "hf.co/bartowski/x:q4", "https://evil.example/m:1",
            "gpt-oss:120b-cloud", "deepseek-v3.1:671b-cloud", "kimi-k2:cloud", "m:..", "../x:1", "-x:1",
            "qwen3:8b; rm", "a b:1", "qwen3:8b@sha256", "모델:1", &"a".repeat(81),
        ] {
            assert!(normalize_custom_model(bad).is_err(), "{bad:?} 는 거부되어야 함");
        }
    }

    #[test]
    fn builtin_catalog_is_valid() {
        let c = Catalog::builtin();
        assert!(c.models.len() >= 6);
        let mut ids = std::collections::HashSet::new();
        for m in &c.models {
            assert!(ids.insert(m.id.clone()), "중복 id {}", m.id);
            assert!(m.id.contains(':'), "태그가 명시되어야 함: {}", m.id);
            assert!(!is_cloud_model_name(&m.id), "클라우드 모델 금지: {}", m.id);
            assert!(m.est_ram_gb > m.download_gb, "{}: 실행 메모리는 파일 크기보다 커야 함", m.id);
            assert!((1..=5).contains(&m.korean) && (1..=5).contains(&m.summarization) && (1..=5).contains(&m.reasoning));
            assert!(!m.license.is_empty() && m.license_url.starts_with("https://"));
        }
    }
}
