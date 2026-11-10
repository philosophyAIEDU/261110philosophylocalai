//! Model Recommendation Layer — 모델 카탈로그와 추천 엔진.
pub mod recommend;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::netguard::is_cloud_model_name;

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
