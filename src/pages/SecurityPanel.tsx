// 개인정보 보호 설정 화면의 보안 기능 묶음: 보안 점검, 앱 잠금, 화면 보호, 전체 초기화, 보안 수칙
import { FormEvent, useCallback, useEffect, useState } from "react";
import { SecurityCheckup, call, errMsg } from "../lib/api";
import { Confirm, ErrorNotice, Icon, Notice } from "../lib/ui";
import { useApp } from "../App";

export const PROFESSOR_RULES: { title: string; body: string }[] = [
  { title: "앱 잠금 암호를 설정하세요", body: "학생 기록을 저장한다면 필수입니다. 암호 없이는 기록을 열 수 없고, 정해진 시간 동안 쓰지 않으면 자동으로 잠깁니다." },
  { title: "자리를 비울 때는 잠그세요", body: "왼쪽 아래 ‘지금 잠그기’를 누르거나, 컴퓨터 화면 잠금(Windows+L / Control+Command+Q)을 사용하세요." },
  { title: "컴퓨터 디스크 암호화를 켜세요", body: "Windows는 BitLocker(또는 ‘장치 암호화’), Mac은 FileVault를 켜면 노트북을 잃어버려도 저장한 Word·Excel 결과 파일까지 보호됩니다." },
  { title: "필요한 정보만 넣으세요", body: "주민등록번호, 건강·가정 형편 같은 민감정보는 입력하지 마세요. 실명 대신 ‘학생 A’ 같은 별칭을 쓰면 더 안전합니다." },
  { title: "저장한 파일은 따로 보호하세요", body: "Word·Excel로 저장하거나 복사한 내용은 이 앱의 암호화 밖에 있습니다. 공유 폴더·메신저·이메일로 보내기 전에 꼭 필요한지 확인하세요." },
  { title: "화면 공유에 주의하세요", body: "화상회의 중에는 상담 기록 화면을 열지 마세요. ‘화면 캡처 방지’를 켜 두면 이 앱 창은 화면 공유·캡처에서 사라져 보이거나(투명하게 빠짐) 검게 나옵니다." },
  { title: "다 쓴 기록은 지우세요", body: "필요 없어진 상담 기록과 대화는 바로 삭제하세요. 컴퓨터를 반납·폐기할 때는 ‘전체 초기화’를 먼저 하세요." },
  { title: "공식 경로로만 업데이트하세요", body: "업데이트는 이 앱의 ‘앱 업데이트’ 메뉴나 공식 릴리스 페이지에서만 받으세요. 메일·메신저로 받은 설치 파일은 열지 마세요." },
  { title: "학교 규정을 따르세요", body: "학생 개인정보의 수집·이용·보관 기간은 소속 대학의 개인정보 보호 규정과 지침을 따라야 합니다." },
];

const DISK_LABEL: Record<string, { text: string; ok: boolean | null }> = {
  on: { text: "켜짐", ok: true },
  in_progress: { text: "암호화 진행 중", ok: true },
  off: { text: "꺼짐", ok: false },
  unknown: { text: "확인할 수 없음 (직접 확인해 주세요)", ok: null },
};

function CheckRow({ ok, title, desc, action }: { ok: boolean | null; title: string; desc: string; action?: React.ReactNode }) {
  return (
    <li className="flex items-start gap-3 py-3">
      <span className={`mt-0.5 flex h-6 w-6 shrink-0 items-center justify-center rounded-full ${ok === true ? "bg-ok-soft text-ok" : ok === false ? "bg-warn-soft text-warn" : "bg-surface-2 text-muted"}`}>
        <Icon name={ok === true ? "check" : ok === false ? "alert" : "info"} className="h-4 w-4" />
      </span>
      <div className="min-w-0 flex-1">
        <div className="font-semibold">{title}</div>
        <div className="text-[13px] text-muted">{desc}</div>
      </div>
      {action}
    </li>
  );
}

export function PasscodeForm({ hasPasscode, onDone }: { hasPasscode: boolean; onDone: () => void }) {
  const [current, setCurrent] = useState("");
  const [next, setNext] = useState("");
  const [again, setAgain] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [ack, setAck] = useState(false);
  const submit = async (e: FormEvent) => {
    e.preventDefault();
    setError(null);
    if (next !== again) { setError("새 암호와 확인 암호가 다릅니다"); return; }
    setBusy(true);
    try { await call("set_passcode", { current: hasPasscode ? current : null, newPasscode: next }); setCurrent(""); setNext(""); setAgain(""); onDone(); }
    catch (err) { setError(errMsg(err)); }
    setBusy(false);
  };
  return (
    <form onSubmit={submit} className="mt-3 space-y-3 rounded-xl bg-surface-2 p-4">
      {hasPasscode && (
        <div><label className="label" htmlFor="pc-cur">현재 암호</label><input id="pc-cur" type="password" autoComplete="current-password" className="input" value={current} onChange={(e) => setCurrent(e.target.value)} /></div>
      )}
      <div className="grid gap-3 sm:grid-cols-2">
        <div><label className="label" htmlFor="pc-new">{hasPasscode ? "새 암호" : "잠금 암호"} (6자 이상)</label><input id="pc-new" type="password" autoComplete="new-password" className="input" value={next} onChange={(e) => setNext(e.target.value)} /></div>
        <div><label className="label" htmlFor="pc-again">한 번 더 입력</label><input id="pc-again" type="password" autoComplete="new-password" className="input" value={again} onChange={(e) => setAgain(e.target.value)} /></div>
      </div>
      <p className="text-[13px] text-muted">다른 곳에서 쓰지 않는, 남이 짐작하기 어려운 문장형 암호를 권합니다. (예: ‘연구실커피는3시’)</p>
      {!hasPasscode && (
        <label className="flex cursor-pointer items-start gap-2.5 text-[13.5px]">
          <input type="checkbox" className="mt-1 h-4 w-4 accent-[var(--purple)]" checked={ack} onChange={(e) => setAck(e.target.checked)} />
          <span><b>암호를 잊으면 저장된 기록을 되찾을 수 없다</b>는 것을 이해했습니다. 암호는 이 컴퓨터 밖 어디에도 보관되지 않습니다.</span>
        </label>
      )}
      <ErrorNotice error={error} />
      <button className="btn-primary" disabled={busy || !next || !again || (!hasPasscode && !ack) || (hasPasscode && !current)}>{busy ? "처리하는 중…" : hasPasscode ? "암호 바꾸기" : "잠금 암호 설정"}</button>
    </form>
  );
}

export default function SecurityPanel() {
  const { status, refresh } = useApp();
  const [check, setCheck] = useState<SecurityCheckup | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [msg, setMsg] = useState<string | null>(null);
  const [showPass, setShowPass] = useState(false);
  const [removeCode, setRemoveCode] = useState("");
  const [showRemove, setShowRemove] = useState(false);
  const [resetOpen, setResetOpen] = useState(false);
  const [resetText, setResetText] = useState("");

  const load = useCallback(() => { call<SecurityCheckup>("security_checkup").then(setCheck).catch((e) => setError(errMsg(e))); }, []);
  useEffect(() => { load(); }, [load]);
  const act = async (fn: () => Promise<unknown>, done?: string) => {
    setError(null); setMsg(null);
    try { await fn(); await refresh(); load(); if (done) setMsg(done); } catch (e) { setError(errMsg(e)); }
  };

  const items = check ? [
    check.has_passcode,
    check.has_passcode && check.auto_lock_minutes > 0,
    check.disk_encryption === "on" || check.disk_encryption === "in_progress",
    !check.screen_protection_supported || check.screen_protection,
    !check.save_history || check.has_passcode,
    check.pseudonymize_students,
    check.key_store === "os_keychain",
  ] : [];
  const score = items.filter(Boolean).length;
  const disk = check ? DISK_LABEL[check.disk_encryption] : null;

  return (
    <>
      <ErrorNotice error={error} onDismiss={() => setError(null)} />
      {msg && <Notice tone="ok">{msg}</Notice>}

      <section className="card">
        <div className="mb-1 flex flex-wrap items-center justify-between gap-3">
          <h2 className="h2">보안 점검</h2>
          {check && <span className={`chip ${score === items.length ? "bg-ok-soft text-ok" : "bg-warn-soft text-warn"}`}>{items.length}개 중 {score}개 충족</span>}
        </div>
        <p className="sub mb-1 text-[13.5px]">학생 개인정보를 다루기 전에 아래 항목을 모두 초록색으로 만들어 주세요.</p>
        {check && (
          <ul className="divide-y divide-border">
            <CheckRow ok={check.has_passcode} title="앱 잠금 암호" desc={check.has_passcode ? "설정됨 — 암호 없이는 기록을 열 수 없습니다." : "설정 안 됨 — 이 컴퓨터를 쓰는 누구나 앱을 열면 기록을 볼 수 있습니다."}
              action={!check.has_passcode ? <button className="btn-primary btn-sm shrink-0" onClick={() => setShowPass(true)}>설정하기</button> : undefined} />
            <CheckRow ok={check.has_passcode && check.auto_lock_minutes > 0} title="자동 잠금" desc={check.has_passcode ? (check.auto_lock_minutes > 0 ? `${check.auto_lock_minutes}분 동안 사용하지 않으면 잠급니다.` : "꺼져 있습니다.") : "앱 잠금 암호를 설정하면 쓸 수 있습니다."} />
            <CheckRow ok={disk?.ok ?? null} title="컴퓨터 디스크 암호화 (BitLocker / FileVault)" desc={`${disk?.text ?? "-"} — 저장한 Word·Excel 파일과 다른 문서를 노트북 분실·도난에서 보호합니다.`} />
            <CheckRow ok={!check.screen_protection_supported ? null : check.screen_protection} title="화면 캡처·화면 공유 방지" desc={!check.screen_protection_supported ? "이 운영체제에서는 지원되지 않습니다." : check.screen_protection ? "켜짐 — 캡처·화면 공유를 하면 이 앱 창이 사라져 보입니다(정상)." : "꺼짐"} />
            <CheckRow ok={!check.save_history || check.has_passcode} title="대화 기록 저장" desc={check.save_history ? (check.has_passcode ? "켜짐 (앱 잠금으로 보호됨)" : "켜짐 — 앱 잠금 없이 대화가 저장되고 있습니다.") : "꺼짐 — 창을 닫으면 대화가 사라집니다."} />
            <CheckRow ok={check.pseudonymize_students} title="학생 기록 가명 처리" desc={check.pseudonymize_students ? "켜짐 — AI에 보낼 때 이름을 ‘학생1’처럼 바꾸고 번호들을 가립니다." : "꺼짐"} />
            <CheckRow ok={check.key_store === "os_keychain"} title="암호화 키 보관" desc={check.key_store === "os_keychain" ? "운영체제 보안 저장소(키체인 / 자격 증명 관리자)에 보관 중" : "파일에 보관 중 (개발용 환경)"} />
          </ul>
        )}
        {check && check.disk_encryption !== "on" && (
          <p className="mt-2 text-[13px] text-muted">디스크 암호화 켜는 방법 — Windows: <b>설정 → 개인 정보 및 보안 → 장치 암호화</b>(또는 제어판의 BitLocker 드라이브 암호화). Mac: <b>시스템 설정 → 개인정보 보호 및 보안 → FileVault</b>. 학교 전산 부서에 문의하셔도 됩니다.</p>
        )}
      </section>

      <section className="card">
        <h2 className="h2 mb-1">앱 잠금</h2>
        <p className="sub text-[13.5px]">잠금 암호는 저장된 기록을 여는 열쇠를 한 번 더 감쌉니다. 이 컴퓨터에 로그인한 사람이라도 암호 없이는 대화·상담 기록을 볼 수 없습니다.</p>
        {status.has_passcode ? (
          <div className="mt-3 space-y-3">
            <div className="flex flex-wrap items-center gap-3">
              <span className="chip bg-ok-soft text-ok"><Icon name="lock" className="h-3.5 w-3.5" />잠금 켜짐</span>
              <label className="flex items-center gap-2 text-[14px]">자동 잠금
                <select className="input w-auto py-1.5" value={status.auto_lock_minutes} onChange={(e) => act(() => call("set_auto_lock", { minutes: Number(e.target.value) }))}>
                  {[3, 5, 10, 15, 30, 60].map((m) => <option key={m} value={m}>{m}분 동안 사용하지 않으면</option>)}
                  <option value={0}>사용 안 함 (권장하지 않음)</option>
                </select>
              </label>
              <button className="btn-ghost btn-sm" onClick={() => setShowPass((v) => !v)}>암호 바꾸기</button>
              <button className="btn-ghost btn-sm" onClick={() => setShowRemove((v) => !v)}>잠금 끄기</button>
            </div>
            {showPass && <PasscodeForm hasPasscode onDone={() => { setShowPass(false); void act(async () => {}, "잠금 암호를 바꿨습니다."); }} />}
            {showRemove && (
              <form className="flex flex-wrap items-end gap-2 rounded-xl bg-surface-2 p-4" onSubmit={(e) => { e.preventDefault(); void act(() => call("remove_passcode", { current: removeCode }), "앱 잠금을 껐습니다.").then(() => { setRemoveCode(""); setShowRemove(false); }); }}>
                <div className="min-w-[200px] flex-1"><label className="label" htmlFor="rm-code">현재 암호</label><input id="rm-code" type="password" className="input" value={removeCode} onChange={(e) => setRemoveCode(e.target.value)} /></div>
                <button className="btn-danger" disabled={!removeCode}>잠금 끄기</button>
                <p className="w-full text-[13px] text-warn">잠금을 끄면 이 컴퓨터에 로그인한 사람은 누구나 기록을 볼 수 있습니다. 학생 기록을 저장한다면 끄지 마세요.</p>
              </form>
            )}
          </div>
        ) : (
          <>
            {!showPass && <button className="btn-primary mt-3" onClick={() => setShowPass(true)}><Icon name="lock" className="h-4 w-4" /> 잠금 암호 설정하기</button>}
            {showPass && <PasscodeForm hasPasscode={false} onDone={() => { setShowPass(false); void act(async () => {}, "앱 잠금을 켰습니다. 다음에 앱을 열 때부터 암호를 물어봅니다."); }} />}
          </>
        )}
      </section>

      <section className="card">
        <h2 className="h2 mb-1">화면 보호</h2>
        <label className="flex cursor-pointer items-start justify-between gap-6 py-2">
          <span><span className="block font-semibold">화면 캡처·화면 공유 방지</span>
            <span className="block text-[13.5px] text-muted">켜면 Zoom·Teams 화면 공유, 화면 녹화, 캡처를 하는 순간 이 앱 창이 사라져 보이거나(투명하게 빠짐) 검게 나옵니다. 캡처가 끝나면 다시 보입니다. 이 앱 화면을 캡처하거나 수업 중에 보여 주셔야 할 때만 잠시 끄고, 끝나면 다시 켜 주세요. (Windows·Mac)</span></span>
          <input type="checkbox" className="mt-1.5 h-5 w-5 shrink-0 accent-[var(--purple)]" checked={status.screen_protection} onChange={(e) => act(() => call("set_screen_protection", { enabled: e.target.checked }))} />
        </label>
      </section>

      <section className="card">
        <h2 className="h2 mb-3">교수님 보안 수칙</h2>
        <ol className="grid gap-3 sm:grid-cols-2">
          {PROFESSOR_RULES.map((r, i) => (
            <li key={r.title} className="flex gap-3 rounded-xl border border-border p-3.5">
              <span className="flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-purple-soft text-[13px] font-bold text-purple">{i + 1}</span>
              <div><div className="font-semibold">{r.title}</div><div className="text-[13px] text-muted">{r.body}</div></div>
            </li>
          ))}
        </ol>
      </section>

      <section className="card border-danger/30">
        <h2 className="h2 mb-1">전체 초기화</h2>
        <p className="sub text-[13.5px]">
          암호화 열쇠를 파기하고 저장된 대화·상담 기록·설정을 모두 지웁니다. 열쇠가 사라지므로 컴퓨터 백업에 남아 있던 이전 사본도 더 이상 읽을 수 없습니다.
          컴퓨터를 반납·폐기하거나 다른 사람에게 넘길 때 사용하세요. 내려받은 AI 모델은 개인정보가 아니므로 남습니다.
        </p>
        <button className="btn-danger mt-3" onClick={() => { setResetText(""); setResetOpen(true); }}><Icon name="trash" className="h-4 w-4" /> 전체 초기화</button>
      </section>

      <Confirm open={resetOpen} danger title="정말 전체 초기화할까요?" confirmLabel="초기화"
        body={<>
          모든 대화·상담 기록·설정이 지워지고 <b>되돌릴 수 없습니다</b>. 계속하려면 아래에 ‘초기화’를 입력해 주세요.
          <input className="input mt-3" value={resetText} onChange={(e) => setResetText(e.target.value)} placeholder="초기화" aria-label="초기화 확인 문구" />
        </>}
        onCancel={() => setResetOpen(false)}
        onConfirm={() => { if (resetText.trim() !== "초기화") return; setResetOpen(false); void act(() => call("factory_reset", { confirm: resetText }), "전체 초기화를 마쳤습니다."); }} />
    </>
  );
}
