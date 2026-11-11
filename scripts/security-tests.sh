#!/usr/bin/env bash
# (bash 필수: /dev/tcp 사용)
# 보안 검증 스크립트 (Linux 전용 — unshare, strace 필요)
#
#  A. 인터넷이 완전히 끊긴 네트워크 공간(unshare -n)에서 핵심 테스트 전체 실행
#     → 채팅·문서 분석·Excel 분석·저장·삭제가 네트워크 없이 동작하는지
#  B. strace로 테스트 중 발생한 모든 connect() 호출을 기록해
#     127.0.0.1 / ::1 / 로컬 소켓 이외의 연결 시도가 없는지 검사
#  C. 정적 검사 + 배포용 프런트엔드 빌드 검사
#  (저장 파일의 평문·삭제 후 잔여 데이터 검사는 storage 단위 테스트가 A·B 단계에서 함께 실행된다)
set -euo pipefail
cd "$(dirname "$0")/.."
OUT="${TMPDIR:-/tmp}/pla-security-$$"
mkdir -p "$OUT"
pass=0; fail=0
ok()   { echo "  [통과] $1"; pass=$((pass+1)); }
bad()  { echo "  [실패] $1"; fail=$((fail+1)); }

echo "== 준비: 테스트 빌드 (네트워크 사용 가능 상태) =="
cargo build -q -p pla-core --features fake-engine --tests --bins

echo "== A. 인터넷 차단 상태에서 전체 테스트 =="
# 실제 root면 일반 네트워크 공간을 쓴다 (사용자 네임스페이스에서는 다른 사용자 소유 폴더의 cargo에 접근 못 할 수 있음)
if unshare -n true 2>/dev/null; then UNSHARE="unshare -n"; elif unshare -rn true 2>/dev/null; then UNSHARE="unshare -rn"; else UNSHARE=""; fi
if [ -z "$UNSHARE" ]; then
  bad "unshare를 사용할 수 없어 오프라인 테스트를 실행하지 못했습니다"
else
  # 새 네트워크 공간에는 lo만 있고 꺼져 있으므로 켠다. 외부 경로는 없다.
  command -v ip >/dev/null || { echo "ip 명령(iproute2)이 필요합니다"; exit 2; }
  # 확인: 외부 주소 연결이 실제로 불가능한지 먼저 검사한 뒤 테스트를 실행한다.
  if $UNSHARE bash -c 'ip link set lo up && ! getent hosts example.com >/dev/null 2>&1 && ! (exec 3<>/dev/tcp/1.1.1.1/443) 2>/dev/null && cargo test -q --offline -p pla-core --features fake-engine' > "$OUT/offline.log" 2>&1; then
    ok "인터넷 차단 상태에서 테스트 전체 통과 ($(grep -c '^test result: ok' "$OUT/offline.log")개 묶음)"
  else
    bad "인터넷 차단 상태 테스트 실패 — $OUT/offline.log 참조"; tail -30 "$OUT/offline.log"
  fi
fi

echo "== B. 테스트 중 외부 연결 시도 감사 (strace) =="
if command -v strace >/dev/null; then
  strace -f -qq -e trace=connect -o "$OUT/connect.trace" \
    cargo test -q --offline -p pla-core --features fake-engine > "$OUT/strace-tests.log" 2>&1 || { bad "strace 하의 테스트 실패"; tail -20 "$OUT/strace-tests.log"; }
  # AF_INET/AF_INET6 연결 대상만 추출
  grep -E 'sa_family=AF_INET6?' "$OUT/connect.trace" \
    | grep -oE 'sin6?_addr=(inet_addr\("[^"]+"\)|inet_pton\(AF_INET6, "[^"]+")' \
    | grep -oE '"[^"]+"$|"[^"]+"\)' | tr -d '")' | sort | uniq -c > "$OUT/destinations.txt" || true
  echo "  연결 대상 주소:"; sed 's/^/    /' "$OUT/destinations.txt"
  # 허용: 루프백, 그리고 non_loopback_ips()의 경로 조회용 UDP connect(문서용 주소 192.0.2.1, 2001:db8::1 — 패킷 미전송)
  # 및 검증 테스트가 의도적으로 시도하는 '이 컴퓨터 자신의 외부 주소'(접속 거부 확인용)
  SELF_IPS=$(hostname -I 2>/dev/null || true)
  external=$(awk '{print $2}' "$OUT/destinations.txt" | grep -vE '^(127\.|::1$|::ffff:127\.|0\.0\.0\.0$|192\.0\.2\.1$|2001:db8::1$)' || true)
  for ip in $SELF_IPS; do external=$(echo "$external" | grep -vx "$ip" || true); done
  if [ -z "$(echo "$external" | tr -d '[:space:]')" ]; then ok "루프백·자기 주소 외 연결 시도 없음"; else bad "외부 연결 시도 발견: $external"; fi
else
  bad "strace가 없어 연결 감사를 하지 못했습니다"
fi

echo "== C. 정적 검사 =="
if cargo test -q --offline -p pla-core --test security_static > "$OUT/static.log" 2>&1; then ok "정적 보안 검사 통과"; else bad "정적 보안 검사 실패"; tail -20 "$OUT/static.log"; fi
if grep -rqE "api\.openai\.com|api\.anthropic\.com|sentry\.io|google-analytics" dist/ 2>/dev/null; then bad "프런트엔드 빌드에 외부 서비스 주소 포함"; else ok "프런트엔드 빌드에 외부 서비스 주소 없음"; fi
if [ -d dist ] && grep -rq "화면 미리보기용 예시" dist/ 2>/dev/null; then bad "배포용 빌드에 미리보기 코드 포함"; else ok "배포용 빌드에 미리보기 코드 없음"; fi

echo
echo "결과: 통과 $pass, 실패 $fail  (기록: $OUT)"
[ "$fail" -eq 0 ]
