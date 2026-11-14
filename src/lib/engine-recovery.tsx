import { useState } from "react";
import { useApp } from "../App";
import { call, errMsg, newJobId } from "./api";
import { ErrorNotice, Notice } from "./ui";

/** 모델 다운로드와 분리한 엔진 복구. 설치 기록이 없어도 진입할 수 있다. */
export function EngineRecovery({ onRecovered }: { onRecovered?: () => void }) {
  const { status, engineError, refresh } = useApp();
  const [consent, setConsent] = useState(status.download_consent);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [stage, setStage] = useState("");
  if (status.engine.running) return null;
  const needsInstall = !status.engine.installed;
  const recover = async () => {
    setBusy(true); setError(null);
    try {
      if (needsInstall) {
        await call("set_download_consent", { consent: true });
        setStage("AI 실행 엔진만 설치하는 중… 모델 파일은 다시 받지 않습니다.");
        await call("install_engine", { jobId: newJobId() });
      }
      setStage("AI 실행 엔진을 시작하는 중…");
      await call("start_engine");
      await refresh();
      onRecovered?.();
    } catch (e) { setError(errMsg(e)); }
    finally { setBusy(false); setStage(""); }
  };
  return <Notice tone="warn" title={needsInstall ? "이 앱의 AI 실행 엔진을 준비해야 합니다" : "AI 실행 엔진이 멈춰 있습니다"}>
    <p>기존 AI 모델은 그대로 둡니다. {needsInstall ? "필요한 실행 엔진만 설치하고 시작합니다." : "아래 버튼으로 엔진을 다시 시작할 수 있습니다."}</p>
    {needsInstall && <label className="my-3 flex items-start gap-2"><input type="checkbox" className="mt-1" checked={consent} disabled={busy} onChange={(e) => setConsent(e.target.checked)} />실행 엔진을 내려받기 위한 인터넷 사용에 동의합니다.</label>}
    <ErrorNotice error={error ?? engineError} />
    {needsInstall && !status.engine.installable && <p>이 빌드에서는 엔진을 설치할 수 없습니다. ‘앱 업데이트’에서 새 버전을 확인해 주세요.</p>}
    <button className="btn-primary mt-3" disabled={busy || (needsInstall && (!consent || !status.engine.installable))} onClick={recover}>{busy ? stage : needsInstall ? "실행 엔진만 설치하고 시작하기" : "AI 엔진 다시 시작하기"}</button>
  </Notice>;
}
