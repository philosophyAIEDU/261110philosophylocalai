# ARCHITECTURE

```
src/ (React, Presentation)  ──IPC(Tauri 명령)──▶  src-tauri/src/lib.rs (Application, 얇은 연결 계층)
                                                      │
crates/core (pla-core)                                ▼
  security/   netguard(네트워크 통제) · crypto · redact
  hardware/   사양 진단 + OS별 파서
  models/     catalog.json · recommend
  runtime/    mod(AiRuntime 트레이트) · ollama(API) · engine(설치·프로세스) · engine-manifest.json
  documents/  추출 · retrieve(근거 검색)
  tabular.rs  Excel/CSV 통계      storage.rs  SQLite+암호화
  export.rs   docx/xlsx/md        tasks.rs    업무 프롬프트·긴 문서 처리
  bin/fake_engine.rs  개발·테스트 전용 (feature = fake-engine)
```
- 웹뷰는 네트워크·엔진에 직접 접근하지 않음. 데이터 흐름: 화면 → Rust → 127.0.0.1의 엔진.
- llama.cpp 추가 시 `AiRuntime` 구현체만 추가하면 됨.
- 앱 데이터: `app_local_data_dir()/` 아래 `app.db`, `engine/<버전>/`, `models/`.
