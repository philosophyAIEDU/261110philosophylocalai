//! Philosophy Local AI — 핵심 로직.
//!
//! 계층 구성 (ARCHITECTURE.md 참조):
//! - `security`  : 네트워크 통제, 암호화, 로그 개인정보 제거
//! - `hardware`  : 컴퓨터 사양 진단
//! - `models`    : 모델 카탈로그 및 추천 엔진
//! - `runtime`   : AI Runtime Adapter (Ollama 구현, 엔진 설치/프로세스 관리)
//! - `documents` : PDF/DOCX/PPTX/TXT 텍스트 추출, 근거 검색
//! - `tabular`   : Excel/CSV 분석 (검증 가능한 수치 계산)
//! - `storage`   : SQLite 로컬 저장소 (필드 단위 암호화)
//! - `export`    : Word/Excel/Markdown 내보내기
//! - `tasks`     : 교수님 업무별 프롬프트 구성 및 긴 문서 처리

pub mod error;
pub mod export;
pub mod documents;
pub mod hardware;
pub mod models;
pub mod runtime;
pub mod security;
pub mod storage;
pub mod tabular;
pub mod tasks;

pub use error::{AppError, AppResult};
