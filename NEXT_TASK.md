# NEXT_TASK — 다음 작업 (순서대로)

1. **실제 앱 실행 확인 (Linux, xvfb)**
   `cargo build -p pla-core --features fake-engine --bin pla-fake-engine` 후
   `PLA_DEV_ENGINE_BIN=$PWD/target/debug/pla-fake-engine xvfb-run target/debug/philosophy-local-ai`
   → 창이 뜨는지, 종료 시 엔진 프로세스가 사라지는지 확인. (PLA_DEV_ENGINE_BIN은 디버그 빌드에서만 동작)
2. **scripts/pin-engine.mjs 작성**: 공식 릴리스의 `sha256sum.txt`
   (`https://github.com/ollama/ollama/releases/download/v<버전>/sha256sum.txt`)를 받아
   `engine-manifest.json`의 각 자산 `sha256`을 채움. 값이 없으면 실패하도록.
3. **scripts/security-tests.sh 작성**: (a) `unshare -n`으로 네트워크 없는 상태에서 core 테스트 실행,
   (b) `strace -f -e trace=connect`로 테스트 중 127.0.0.1 이외 연결이 없는지 검사.
4. **.github/workflows/build.yml 작성**: windows-latest(NSIS), macos-latest(universal DMG)에서
   pin-engine → 테스트 → `tauri build`. 산출물 업로드. (서명·공증은 시크릿이 있을 때만)
5. **실제 Ollama 검증** (네트워크가 허용된 환경 필요): 엔진 설치, `gemma3:4b` 내려받기, 채팅,
   `/api/status`의 cloud.disabled 확인, 오프라인 채팅, 카탈로그 태그 실존 여부 확인.
6. 한국어 PDF 픽스처 추가 및 추출 테스트.
7. 문서 완성: 사용자 설명서(docs/USER_GUIDE.md), 설치·제거 안내, 보안 검증 보고서, 라이선스 목록, README.
8. Windows/macOS 실기 수동 테스트 후 TEST_RESULTS.md 갱신.

작은 개선 메모: 무결성 확인 단계에서는 '취소' 버튼 숨기기 (`src/lib/download.tsx`).
