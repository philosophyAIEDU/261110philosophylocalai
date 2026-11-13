# PROJECT_STATUS — 현재 개발 상태 (2026-11-13, v0.2.0)

## v0.2.0 변경 요약
- 보안 점검 9건 수정·추가 (docs/SECURITY_REPORT.md 0절): 앱 잠금·자동 잠금, 웹뷰 파일 경로 차단,
  가명 처리, 키 파기 초기화, 보안 점검 패널, 크기 상한, 임시 파일 차단, 한국어 번호 가림 수정
- 앱 내 업데이트 ('앱 업데이트' 메뉴): GitHub 릴리스 확인 → SHA-256 검증 내려받기 → 설치 파일 실행
- 화면 곳곳의 보안 유의사항, 온보딩 '안전 설정' 단계, 교수님 보안 수칙 9개
- 릴리스 v0.2.0 게시 완료 (사전 출시): https://github.com/philosophyAIEDU/261110philosophylocalai/releases/tag/v0.2.0

## (이전) 2026-11-11 상태

## 최종 결과물 현황 (요구사항 14절 기준)

| # | 결과물 | 상태 |
|---|---|---|
| 1 | 실행 가능한 전체 소스코드 | ✅ 완료 |
| 2 | Windows 설치 파일 | 🟡 CI 빌드 성공(NSIS, 6.8MB), Windows 단위 테스트 통과. 서명 없음, 실기 설치 미검증 |
| 3 | macOS 설치 파일 | 🟡 CI 빌드 성공(universal DMG, 18MB), macOS 단위 테스트 통과. 서명·공증 없음, 실기 설치 미검증 |
| 4 | Ollama 관리형 실행 환경 | ✅ 실제 Ollama 0.40.16으로 설치·무결성·실행·클라우드 차단·정리 검증 (Linux) |
| 5 | 하드웨어 진단 | ✅ Linux 실측, macOS CI 실행, Windows는 출력 파서 단위 테스트 |
| 6 | 모델 추천 및 원클릭 다운로드 | 🟡 추천·내려받기 흐름 완성. **실제 모델 내려받기 미검증**(개발 환경 차단) |
| 7 | 로컬 AI 채팅 | 🟡 완성, 가짜 엔진으로 검증. 실제 모델 미검증 |
| 8 | 교수님 전용 업무 기능 | 🟡 완성(강의·학생·연구·행정·Excel). 실제 모델 응답 품질 미검증 |
| 9 | 개인정보 보호 기능 | ✅ 완료, 보안 테스트 통과 (실제 모델 관련 3항목 부분) |
| 10 | 사용자 설명서 | ✅ docs/USER_GUIDE.md |
| 11 | 설치 및 제거 안내 | ✅ docs/USER_GUIDE.md 1·6절 |
| 12 | 테스트 결과 보고서 | ✅ TEST_RESULTS.md |
| 13 | 보안 검증 보고서 | ✅ docs/SECURITY_REPORT.md |
| 14 | 소프트웨어·모델 라이선스 목록 | ✅ docs/LICENSES.md (자동 생성 전체 고지는 미작성) |
| 15 | GitHub README | ✅ README.md |

## 배포 가능 여부
**아직 공개 배포 불가.** 남은 검증(NEXT_TASK.md):
1. 실제 AI 모델 내려받기와 오프라인 사용 (모델 저장소 접근 가능한 환경 필요)
2. Windows·macOS 실기 설치·실행·제거
3. 코드 서명·공증

## 코드 구성
- `crates/core` (pla-core): 보안·진단·추천·엔진·문서·통계·저장·내보내기·업무 프롬프트
- `src-tauri`: Tauri 2 명령 계층(38개), CSP·권한 최소화, 단일 실행, 강제 종료 시 엔진 정리
- `src`: React 화면 — 온보딩 + 9개 메뉴, 다크 모드
- `scripts`: `pin-engine.mjs`, `security-tests.sh` · `.github/workflows/build.yml`
