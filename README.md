# Philosophy Local AI — 필로소피 로컬 AI

대학 교수님을 위한 프라이버시 중심 로컬 AI 데스크톱 프로그램 (Tauri 2 · React · Rust · Ollama).

> **개발 중입니다. 아직 배포 가능한 상태가 아닙니다.** 실제 Ollama 연동과 Windows/macOS 실행은
> 검증되지 않았습니다. 현재 상태는 [PROJECT_STATUS.md](PROJECT_STATUS.md)를 보세요.

## 개발
```bash
npm install
cargo test -p pla-core --features fake-engine   # 핵심 로직 테스트
npm run dev                                     # 브라우저 화면 미리보기(예시 데이터)
npx tauri dev                                   # 데스크톱 앱 (Linux는 webkit2gtk-4.1 필요)
```
문서: ARCHITECTURE.md · SECURITY_MODEL.md · SECURITY_CHECKLIST.md · DECISIONS.md · MODEL_CATALOG.md · TEST_RESULTS.md · NEXT_TASK.md

라이선스: MIT. AI 실행 엔진 Ollama(MIT), 모델은 각 모델의 라이선스(Apache-2.0, Gemma Terms of Use)를 따릅니다.
