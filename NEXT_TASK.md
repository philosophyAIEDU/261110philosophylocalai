# NEXT_TASK — 다음 작업 (순서대로)

1. **실제 모델 검증** (registry.ollama.ai 접근 가능한 환경, 이 개발 컨테이너에서는 차단됨)
   - 앱 온보딩으로 `gemma3:4b` 또는 `qwen3:4b` 내려받기 → 실행 테스트 통과 확인
   - 인터넷을 끊고: 채팅, 한국어 PDF 강의자료 요약, Excel 보고서 생성
   - `crates/core/tests/fixtures`의 PDF와 프롬프트 인젝션 문서로 답변 내용 확인
   - 카탈로그 모델 태그가 모두 실존하는지 확인 → 없으면 `catalog.json` 수정
2. **Windows/macOS 실기 테스트**: CI 산출물(installer-windows-latest, installer-macos-latest) 설치 →
   첫 실행 → 엔진 설치 → 작업 관리자/활동 모니터에서 앱 강제 종료 시 `ollama` 프로세스가 사라지는지 →
   `netstat -ano | findstr <포트>` / `lsof -iTCP -sTCP:LISTEN`으로 127.0.0.1 바인딩 확인 → 제거
3. 코드 서명·공증 설정 (인증서 확보 후 CI 시크릿 사용)
4. 오픈소스 고지 파일 자동 생성 (`cargo about`, `license-checker`)
5. 개선 후보: HWP 지원(hwp → 텍스트), 스캔 PDF OCR(로컬), 엔진 버전 업데이트 흐름(새 매니페스트 + pin-engine)

엔진 버전을 올릴 때: `node scripts/pin-engine.mjs <새버전>` → 테스트 → 실제 엔진 테스트
(`PLA_REAL_ENGINE_DIR=/경로 cargo test -p pla-core --test real_engine -- --nocapture`).
