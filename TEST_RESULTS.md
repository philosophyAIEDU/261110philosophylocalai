# TEST_RESULTS — 테스트 결과

## v0.2.0 (2026-11-13, Linux 개발 컨테이너)
- `cargo test -p pla-core --features fake-engine`: 단위 77 · 통합 11 · 한국어 PDF 1 · 정적 보안 7 · 기타 통과
- `cargo test -p philosophy-local-ai`: 저장 파일 이름 정화 테스트 통과
- `bash scripts/security-tests.sh`: 5/5 (네트워크 차단 공간에서 9개 테스트 묶음 통과, strace 외부 연결 없음)
- 실제 GitHub 업데이트 테스트(`PLA_REAL_UPDATE=1 … --test real_update`): 확인·내려받기·SHA-256 검증 통과
- 실제 앱(xvfb): 잠금 시작/틀린 암호/해제/기록 표시/1분 자동 잠금, Rust 파일 선택 창으로 CSV 불러오기 통과
- 브라우저 미리보기: 잠금 화면, 보안 점검, 앱 업데이트, 학생 가명 처리, 온보딩 안전 설정 — 콘솔 오류 없음
- 미검증: Windows·macOS 실기(화면 캡처 방지, 키체인, Job Object, 설치 파일 실행), 실제 AI 모델

## (이전) 2026-11-11

## 1. 자동 테스트

### Linux (개발 컨테이너 Ubuntu 24.04 + GitHub Actions ubuntu-24.04) — 통과
`cargo test -p pla-core --features fake-engine` — **72개 통과**
- 단위 54: 네트워크 정책, 암호화·변조 감지, 개인정보 가림, OS별 GPU 출력 파서(Windows/macOS 샘플 포함), 추천 엔진,
  문서 추출(DOCX/PPTX/TXT/EUC-KR), Excel/CSV 통계 정확성, 저장소 암호화·삭제 후 잔여 데이터, Word/Excel 내보내기 왕복
- 통합 11 (가짜 엔진 프로세스): 엔진 시작·종료·중복 방지, 루프백 전용, 외부 노출·클라우드 켜짐 감지, 동의 없는 내려받기 차단,
  카탈로그 외·클라우드 모델 차단, 진행률·재시도·취소·디스크 부족, 스트리밍·응답 중지, 잔여 프로세스 정리, 민감정보+인젝션 문서
- 한국어 PDF 1: Chromium으로 만든 실제 PDF(글꼴 내장, 2쪽)에서 한국어 텍스트·쪽 수 추출
- 정적 보안 5
- 실제 엔진 1 (`PLA_REAL_ENGINE_DIR` 설정 시): 아래 2절

`sudo bash scripts/security-tests.sh` — **5/5 통과** (로컬 + CI)
- 네트워크 차단 공간에서 72개 테스트 전체 통과 (대조: 차단 공간 밖에서는 외부 연결 성공, 안에서는 실패 확인)
- strace 연결 감사: 127.0.0.1, 자기 IP(접속 거부 확인용), 192.0.2.1(경로 조회용 UDP, 패킷 미전송) 외 연결 없음

### macOS (GitHub Actions macos-latest, Apple 칩) — 단위 테스트 통과
`cargo test -p pla-core --lib` — 실제 macOS에서 하드웨어 진단(`system_profiler`), 키체인 코드 컴파일, 저장소·추출·통계 테스트 통과

### Windows (GitHub Actions windows-latest) — 단위 테스트 통과
`cargo test -p pla-core --lib` 56개 통과. 첫 실행에서 **실제 버그 1건 발견·수정**:
Windows 경로(`\\?\D:\…`)와 디스크 마운트 지점 비교 실패로 저장 공간 표시·공간 부족 검사가 동작하지 않던 문제 (커밋 40c6680).

## 2. 실제 Ollama 엔진 검증 (Linux, v0.40.16 공식 배포 파일) — 통과
- 1,545,767,640 바이트 내려받기 → 공식 SHA-256 일치 → 설치 (약 90초)
- 동의 없는 설치·모델 내려받기 거부
- `OLLAMA_HOST=0.0.0.0`이 부모 환경에 있어도 127.0.0.1에만 바인딩, 외부 주소 접속 거부
- `/api/status` → cloud.disabled=true, 엔진에 클라우드 모델 직접 요청 → `ollama cloud is disabled` 오류
- 종료 후 포트·프로세스 정리

## 3. 실제 데스크톱 앱 (Linux, xvfb 가상 화면) — 통과
- 디버그 빌드: 창 표시, 한국어 글꼴 렌더링, 창 닫기/SIGTERM/SIGKILL 모두 엔진 정리
- 릴리스 `.deb`(8.6MB) 설치 후 실행: 실제 Ollama 자동 시작, 엔진 환경 `OLLAMA_HOST=127.0.0.1:<임의>`·`OLLAMA_NO_CLOUD=1` 확인,
  개인정보 화면에 "클라우드 꺼짐·내부 전용 확인됨", 모델 없음 → '로컬 전용' 배지 미확인(의도대로), 창 닫으면 엔진 종료
- 릴리스 바이너리에 개발용 훅(`PLA_DEV_ENGINE_BIN`)·미리보기 코드 없음 확인

## 4. CI 설치 파일 빌드 — 성공 (서명 없음)
| 플랫폼 | 산출물 | 크기(압축) | CI 실행 |
|---|---|---|---|
| Windows x64 | NSIS 설치 파일 `installer-windows-latest` | 6.8MB | run #4 (커밋 40c6680) |
| macOS (Apple 칩 + Intel, universal) | DMG `installer-macos-latest` | 18.0MB | run #2 (커밋 37676fd) |
| Linux | `.deb` (로컬 빌드, 참고용) | 8.6MB | — |

산출물은 GitHub Actions 실행 페이지에서 내려받을 수 있습니다 (보관 기간 90일).
**설치·실행은 실기에서 아직 확인하지 않았습니다.**

## 5. 수동 확인 (브라우저 미리보기, 예시 데이터)
온보딩 4단계, 9개 메뉴, 다크 모드, 960×640 최소 창 — 콘솔 오류 없음

## 6. 수행하지 못한 테스트
- **실제 AI 모델 내려받기·실행**: 개발 환경에서 registry.ollama.ai 차단 → 오프라인 채팅·문서 요약 품질·인젝션 응답 미검증
- **Windows·macOS 실제 실행**(설치 → 첫 실행 → 엔진 설치 → 강제 종료 → 제거): 실기 없음
- 코드 서명·공증
