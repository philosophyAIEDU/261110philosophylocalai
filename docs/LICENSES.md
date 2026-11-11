# 소프트웨어 및 모델 라이선스

## 이 프로그램
- Philosophy Local AI — MIT License (`LICENSE`)

## 함께 사용하는 실행 엔진 (설치 파일에 포함되지 않으며, 첫 실행 때 공식 배포처에서 내려받음)
| 이름 | 버전 | 라이선스 | 출처 |
|---|---|---|---|
| Ollama | 0.40.16 | MIT | https://github.com/ollama/ollama (공식 릴리스, SHA-256 고정) |

Ollama 배포 파일에는 llama.cpp(MIT), GGML(MIT) 및 GPU 런타임 라이브러리(예: NVIDIA CUDA 재배포 라이브러리 — NVIDIA 라이선스)가 포함됩니다. 각 라이선스는 내려받은 엔진 폴더 안의 `*_LICENSE`, `*_NOTICE` 파일을 따릅니다.

## AI 모델 (사용자가 선택해 내려받음)
| 모델 | 라이선스 | 비고 |
|---|---|---|
| Qwen 3 (qwen3:4b/8b/14b/30b) | Apache-2.0 | Alibaba Cloud |
| Gemma 3 (gemma3:1b/4b/12b/27b) | Gemma Terms of Use | Google — 금지 용도 정책 포함, 사용 전 확인 필요 |

## 주요 오픈소스 구성 요소
| 구성 요소 | 라이선스 |
|---|---|
| Tauri 2, tauri-plugin-dialog, tauri-plugin-single-instance | MIT / Apache-2.0 |
| React, react-dom, react-markdown, remark-gfm | MIT |
| Tailwind CSS, Vite | MIT |
| Pretendard (글꼴) | SIL Open Font License 1.1 |
| tokio, reqwest, serde, rusqlite(SQLite: Public Domain), sysinfo | MIT / Apache-2.0 |
| chacha20poly1305 (RustCrypto) | MIT / Apache-2.0 |
| pdf-extract, lopdf, calamine, quick-xml, zip, docx-rs, rust_xlsxwriter, csv | MIT / Apache-2.0 |
| keyring | MIT / Apache-2.0 |

전체 의존성 목록은 `Cargo.lock`, `package-lock.json`에 있습니다. 배포 전 `cargo about` 등으로 전체 고지 파일을 생성하는 것을 권장합니다(아직 미생성).
