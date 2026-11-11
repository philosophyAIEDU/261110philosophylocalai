# PROJECT_STATUS — 현재 개발 상태 (2026-11-11)

## 완료 (코드 작성 + 이 Linux 개발 환경에서 테스트 통과)
- `crates/core` (pla-core): 보안(NetGuard·암호화·개인정보 제거), 하드웨어 진단, 모델 카탈로그·추천,
  AiRuntime 어댑터 + Ollama 클라이언트, 관리형 엔진 설치·프로세스 관리, 문서 추출(PDF/DOCX/PPTX/TXT),
  근거 검색, Excel/CSV 통계, SQLite 암호화 저장소, Word/Excel 내보내기, 업무 프롬프트(17종)
- 테스트: `cargo test -p pla-core --features fake-engine` → 단위 54 + 통합 11 + 정적 보안 5 = 70개 통과
- `src-tauri`: Tauri 2 명령 계층(38개 명령), CSP·권한 최소화, 단일 실행, 종료 시 엔진 정리 — 컴파일 통과
- `src/`: React 화면 9개 메뉴 + 온보딩. `npm run build` 통과, 브라우저 미리보기로 전 화면 동작 확인(콘솔 오류 0)
- `npx tauri build --debug --no-bundle` 성공 (Linux 디버그 실행 파일 생성)

## 아직 하지 않은 것 / 검증하지 못한 것 (중요)
- **실제 Ollama로 한 번도 실행하지 못함.** 이 개발 환경은 ollama.com / registry.ollama.ai / GitHub 릴리스
  자산 접근이 막혀 있음. 모든 엔진 테스트는 가짜 엔진(`pla-fake-engine`)으로 수행함.
- **엔진 SHA-256 미등록**: `crates/core/src/runtime/engine-manifest.json`의 `sha256`이 모두 `null`.
  이 상태에서는 앱이 엔진 설치를 거부함(fail-closed). `scripts/pin-engine.mjs`(미작성)로 채워야 함.
- **Windows / macOS에서 빌드·실행·설치 테스트 전혀 못 함.** 설치 파일(NSIS, DMG) 미생성. CI 워크플로 미작성.
- 실제 Tauri 앱 창을 띄운 E2E 확인(xvfb) 미수행 — 빌드만 확인.
- 모델 카탈로그의 이름·크기는 개발자 지식 기준이며 이번 세션에서 ollama.com으로 재확인하지 못함.
- 한국어 PDF 실물 파일 추출 테스트 없음(손상 PDF 처리만 테스트). 스캔 PDF(OCR), HWP 미지원.
- 코드 서명·공증 미수행.
- 문서: SECURITY_MODEL/ARCHITECTURE 등은 요약본만 있음. 사용자 설명서·보안 검증 보고서 미작성.
