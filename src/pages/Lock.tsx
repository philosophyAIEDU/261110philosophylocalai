import { FormEvent, useEffect, useRef, useState } from "react";
import { call, errCode, errMsg } from "../lib/api";
import { Icon } from "../lib/ui";

/** 잠금 화면. 잠금 암호를 입력해야 기록·문서에 접근할 수 있다. */
export default function LockScreen({ onUnlocked }: { onUnlocked: () => void }) {
  const [code, setCode] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [showHelp, setShowHelp] = useState(false);
  const [confirm, setConfirm] = useState("");
  const ref = useRef<HTMLInputElement>(null);
  useEffect(() => { ref.current?.focus(); }, []);

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    if (!code || busy) return;
    setBusy(true); setError(null);
    try {
      await call("unlock_app", { passcode: code });
      setCode("");
      onUnlocked();
    } catch (err) {
      setError(errMsg(err));
      if (errCode(err) === "wrong_passcode") setCode("");
    }
    setBusy(false);
    ref.current?.focus();
  };

  return (
    <div className="flex h-full items-center justify-center bg-bg p-6">
      <form onSubmit={submit} className="card w-full max-w-[420px] px-8 py-9 text-center">
        <div className="mx-auto mb-4 flex h-14 w-14 items-center justify-center rounded-2xl bg-navy text-on-primary"><Icon name="lock" className="h-7 w-7" /></div>
        <h1 className="h2 mb-1 text-[20px]">필로소피 로컬 AI가 잠겨 있습니다</h1>
        <p className="sub mb-6 text-[14px]">학생 기록과 대화를 보호하기 위해 잠갔습니다.<br />잠금 암호를 입력해 주세요.</p>
        <label className="sr-only" htmlFor="passcode">잠금 암호</label>
        <input id="passcode" ref={ref} type="password" autoComplete="current-password" className="input mb-3 text-center text-[16px] tracking-wider" placeholder="잠금 암호" value={code} onChange={(e) => setCode(e.target.value)} disabled={busy} />
        {error && <p className="mb-3 text-[13.5px] text-danger" role="alert">{error}</p>}
        <button className="btn-primary w-full py-3" disabled={!code || busy}>{busy ? "확인하는 중…" : "잠금 풀기"}</button>
        <button type="button" className="mt-4 text-[13px] text-muted underline underline-offset-4" onClick={() => setShowHelp((v) => !v)}>암호를 잊으셨나요?</button>
        {showHelp && (
          <div className="mt-3 rounded-xl bg-surface-2 px-4 py-3 text-left text-[13px] text-muted">
            잠금 암호는 교수님의 컴퓨터에만 있고 어디에도 보관되지 않아 <b className="text-text">되찾을 수 없습니다</b>. 이것이 자료를 지키는 방법입니다.
            암호를 잊으셨다면 <b className="text-text">전체 초기화</b>만 가능합니다. 저장된 대화·상담 기록·설정이 모두 지워지고 되돌릴 수 없습니다. (내려받은 AI 모델은 남습니다)
            <div className="mt-3 flex gap-2">
              <input className="input py-1.5 text-[13px]" placeholder="‘초기화’라고 입력" value={confirm} onChange={(e) => setConfirm(e.target.value)} aria-label="초기화 확인 문구" />
              <button type="button" className="btn-danger btn-sm shrink-0" disabled={confirm.trim() !== "초기화"}
                onClick={async () => { try { await call("factory_reset", { confirm }); onUnlocked(); } catch (err) { setError(errMsg(err)); } }}>전체 초기화</button>
            </div>
          </div>
        )}
      </form>
    </div>
  );
}
