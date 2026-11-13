# Philosophy Local AI — 필로소피 로컬 AI

> **AI로 성장하고, 그 성장을 나눈다.**

대학 교수님을 위한 **프라이버시 중심 로컬 AI 데스크톱 프로그램**입니다. 학생 정보, 강의자료, 연구자료를 인터넷으로 보내지 않고 교수님의 컴퓨터 안에서 분석합니다.

> ⚠️ **현재 상태: 개발 버전 (공개 배포 전).** 실제 AI 모델 실행과 Windows·macOS 실기 테스트가 아직 남아 있습니다.
> 자세한 내용은 [PROJECT_STATUS.md](PROJECT_STATUS.md)와 [docs/SECURITY_REPORT.md](docs/SECURITY_REPORT.md)를 보세요.

## 주요 기능
- **컴퓨터 사양 자동 진단** — CPU, 메모리, 그래픽(Apple 칩 통합 메모리 포함), 저장 공간
- **AI 모델 2~3개 추천** — 빠르고 가벼운 AI / 균형 잡힌 추천 AI / 고성능 AI (Qwen 3, Gemma 3)
- **원클릭 설치** — AI 실행 엔진(Ollama)과 모델을 자동으로 내려받고 무결성 확인, 실행 테스트까지
- **교수님 업무** — AI 대화, 강의자료 분석(요약·개념·토론 질문·퀴즈), 학생 상담 기록, 연구자료 요약·비교, 행정 문서 초안, Excel/CSV 통계(코드로 정확히 계산)
- **결과 저장** — Word(.docx), Excel(.xlsx), 텍스트
- **개인정보 보호** — 로컬 전용 실행을 실제로 검사해 표시, 암호화 저장, 대화 저장 기본 꺼짐, 인터넷 사용 기록
- **앱 잠금·자동 잠금** — 잠금 암호(Argon2id)로 기록 열쇠를 보호, 무사용 시 자동 잠금
- **화면 캡처·공유 방지, 학생 기록 가명 처리, 키 파기 초기화, 보안 점검**
- **앱 내 업데이트** — GitHub 릴리스 확인, SHA-256 검증 후 설치

## 설치
[docs/USER_GUIDE.md](docs/USER_GUIDE.md)의 *설치하기*·*제거하기*를 참고하세요.
설치 파일은 GitHub Actions의 `build` 워크플로 산출물(`installer-windows-latest`, `installer-macos-latest`)로 만들어집니다.
현재 설치 파일은 **코드 서명·공증이 되어 있지 않아** Windows SmartScreen / macOS Gatekeeper 경고가 나옵니다.

## 개발
```bash
npm install
npm run dev                                       # 브라우저 화면 미리보기 (예시 데이터)
npx tauri dev                                     # 데스크톱 앱 (Linux: libwebkit2gtk-4.1-dev 필요)
cargo test -p pla-core --features fake-engine     # 핵심 로직·통합·정적 보안 테스트
sudo bash scripts/security-tests.sh               # 오프라인 실행·연결 감사 (Linux)
PLA_REAL_ENGINE_DIR=/tmp/eng cargo test -p pla-core --test real_engine -- --nocapture  # 실제 Ollama
node scripts/pin-engine.mjs [버전]                # 엔진 SHA-256 고정
npx tauri build                                   # 설치 파일
```

## 구조
| 경로 | 내용 |
|---|---|
| `crates/core` | 핵심 로직 (보안, 진단, 추천, 엔진 관리, 문서·데이터 분석, 저장, 내보내기) |
| `src-tauri` | 데스크톱 앱 연결 계층 |
| `src` | 화면 (React + TypeScript + Tailwind) |
| `scripts` | 엔진 무결성 고정, 보안 검증 |
| `docs` | 사용자 설명서, 보안 검증 보고서, 라이선스 |

설계 문서: [ARCHITECTURE.md](ARCHITECTURE.md) · [SECURITY_MODEL.md](SECURITY_MODEL.md) · [SECURITY_CHECKLIST.md](SECURITY_CHECKLIST.md) · [DECISIONS.md](DECISIONS.md) · [MODEL_CATALOG.md](MODEL_CATALOG.md) · [TEST_RESULTS.md](TEST_RESULTS.md) · [RELEASE_CHECKLIST.md](RELEASE_CHECKLIST.md) · [NEXT_TASK.md](NEXT_TASK.md)

## 라이선스
MIT. AI 실행 엔진 Ollama(MIT)는 첫 실행 때 공식 배포처에서 내려받으며, AI 모델은 각 모델의 라이선스(Apache-2.0, Gemma Terms of Use)를 따릅니다. [docs/LICENSES.md](docs/LICENSES.md)
