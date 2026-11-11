# DECISIONS — 주요 기술 결정

1. **Tauri 2 + React + Rust 워크스페이스**. 업무·보안 로직은 `crates/core`(UI 비의존)에 두고 `src-tauri`는 얇게 유지 → 웹뷰 없이 테스트 가능.
2. **Ollama는 설치 파일에 넣지 않고 첫 실행 때 내려받음**. 이유: Windows용 독립 실행 zip이 GPU 라이브러리 포함으로 매우 커서 NSIS 2GB 한계 위험. Ollama는 MIT 라이선스이며 공식 문서(docs/windows.mdx "Standalone CLI")가 앱 내장 용도의 zip을 안내함. 버전·SHA-256을 앱에 고정하고 값이 없으면 설치 거부.
3. **시스템에 설치된 Ollama와 분리**: 앱 데이터 폴더의 전용 실행 파일, 임의의 루프백 포트, 전용 모델 폴더 사용. 사용자 환경의 `OLLAMA_*` 변수는 모두 제거 후 실행.
4. **클라우드 차단**: `OLLAMA_NO_CLOUD=1`(공식 FAQ) + 실행 후 `/api/status`로 실제 확인 + 클라우드 모델 이름·`remote_host` 응답 차단. 환경변수만 믿지 않음.
5. **'로컬 전용' 표시**는 (클라우드 꺼짐 확인) ∧ (외부 주소로 접속 불가를 실제 연결로 확인) ∧ (선택 모델이 로컬)일 때만 켜짐.
6. **HTTP 클라이언트는 `security/netguard.rs`에서만 생성** (정적 테스트로 강제). 웹뷰는 CSP로 네트워크 차단, Tauri 권한은 core + 파일 대화상자뿐.
7. **저장 암호화는 필드 단위 XChaCha20-Poly1305**, 키는 OS 키체인(macOS/Windows). SQLCipher는 Windows 빌드 복잡도 때문에 보류. Linux는 개발용 키 파일(약함, 정식 지원 아님).
8. **불러온 문서는 디스크에 저장하지 않음**(메모리만). 대화 저장은 기본 꺼짐.
9. **수치는 Rust 코드로 계산**, LLM에는 집계 통계표만 전달(개별 행 미전달).
10. **임베딩 없이 글자 2-gram 검색**으로 문서 Q&A — 추가 모델 다운로드 불필요.
11. 모델 `think: false` 고정(Ollama는 비지원 모델에 false를 보내도 오류 없음 — server/routes.go 확인).
12. reqwest 0.12 / keyring 3 / zip 2.4.2 고정(최신 메이저와 API·호환성 차이).
