# SECURITY_CHECKLIST (요구사항 4.4 기준)
상세 근거: [docs/SECURITY_REPORT.md](docs/SECURITY_REPORT.md) · 실행: `bash scripts/security-tests.sh` (Linux, root 또는 user namespace 필요)

| # | 항목 | 상태 |
|---|---|---|
| 1 | 인터넷 차단 상태 AI 채팅 | 부분 — 차단 공간에서 흐름 통과(가짜 엔진). **실제 모델 미검증** |
| 2 | 인터넷 차단 상태 PDF 분석 | 통과(한국어 PDF 추출 + 작업 흐름). 실제 모델 응답 미검증 |
| 3 | 인터넷 차단 상태 Excel 분석 | 통과 |
| 4 | 민감 가상 데이터 네트워크 통신 검사 | 통과 (strace 연결 감사) |
| 5 | 외부 AI API 호출 여부 | 통과 |
| 6 | Ollama 클라우드 차단 | **통과 — 실제 Ollama 0.40.16** |
| 7 | 로컬 서버 외부 접근 차단 | **통과 — 실제 Ollama 0.40.16** (Linux) |
| 8 | 삭제 후 잔여 데이터 | 통과 |
| 9 | 로그·오류 메시지 개인정보 | 통과 |
| 10 | 악의적 문서·프롬프트 인젝션 | 부분 — 구조적 방어 통과, 실제 모델 응답 미검증 |

1·2·10의 실제 모델 검증과 Windows/macOS 실행 검증이 남아 있으므로 **공개 배포 전 추가 검증 필요**.
