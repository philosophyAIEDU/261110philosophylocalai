// AI 모델 원클릭 내려받기 흐름:
// 동의 확인 → (필요 시) 실행 엔진 설치 → 엔진 시작 → 모델 내려받기 → 무결성 확인 → 실행 테스트 → 선택
import { useEffect, useRef, useState } from "react";
import { CatalogModel, Progress, call, errCode, errMsg, fmtBytes, newJobId, on } from "./api";
import { ErrorNotice, Icon, Notice, ProgressBar } from "./ui";
import { useApp } from "../App";

type Phase = "idle" | "engine" | "starting" | "model" | "testing" | "done" | "error";

export function DownloadFlow({ model, onDone, doneLabel = "AI 사용 시작하기" }: { model: CatalogModel; onDone: () => void; doneLabel?: string }) {
  const { status, refresh } = useApp();
  const [consent, setConsent] = useState(status.download_consent);
  const [phase, setPhase] = useState<Phase>("idle");
  const [progress, setProgress] = useState<Progress | null>(null);
  const [speed, setSpeed] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [testReply, setTestReply] = useState<{ reply: string; tokens_per_sec: number | null } | null>(null);
  const jobRef = useRef<string | null>(null);
  const lastRef = useRef<{ t: number; b: number } | null>(null);

  useEffect(() => {
    const handle = (p: { job_id: string; progress: Progress }) => {
      if (p.job_id !== jobRef.current) return;
      setProgress(p.progress);
      const now = performance.now();
      const last = lastRef.current;
      if (!last || p.progress.completed_bytes < last.b) lastRef.current = { t: now, b: p.progress.completed_bytes };
      else if (now - last.t > 800) {
        setSpeed(((p.progress.completed_bytes - last.b) / (now - last.t)) * 1000);
        lastRef.current = { t: now, b: p.progress.completed_bytes };
      }
    };
    const subs = [on("pull-progress", handle), on("engine-install-progress", handle)];
    return () => { subs.forEach((s) => s.then((un) => un())); };
  }, []);

  const begin = async () => {
    setError(null); setTestReply(null); setProgress(null); setSpeed(0); lastRef.current = null;
    try {
      await call("set_download_consent", { consent: true });
      if (!status.engine.installed) {
        setPhase("engine");
        jobRef.current = newJobId();
        await call("install_engine", { jobId: jobRef.current });
      }
      setPhase("starting"); setProgress(null);
      await call("start_engine");
      setPhase("model"); lastRef.current = null;
      jobRef.current = newJobId();
      await call("pull_model", { jobId: jobRef.current, name: model.id });
      setPhase("testing");
      const t = await call<{ reply: string; tokens_per_sec: number | null }>("test_model", { name: model.id });
      setTestReply(t);
      await call("select_model", { name: model.id });
      await refresh();
      setPhase("done");
    } catch (e) {
      if (errCode(e) === "cancelled") { setPhase("idle"); setProgress(null); }
      else { setError(errMsg(e)); setPhase("error"); }
      await refresh();
    }
  };

  const cancel = () => { if (jobRef.current) void call("cancel_job", { jobId: jobRef.current }); };
  const busy = phase === "engine" || phase === "starting" || phase === "model" || phase === "testing";
  const steps: { key: Phase; label: string }[] = [
    { key: "engine", label: "AI 실행 엔진 설치" },
    { key: "model", label: "AI 모델 내려받기" },
    { key: "testing", label: "실행 테스트" },
  ];
  const order: Phase[] = ["engine", "starting", "model", "testing", "done"];
  const stepState = (k: Phase) => {
    if (phase === "done") return "done";
    const cur = order.indexOf(phase === "starting" ? "engine" : phase);
    const idx = order.indexOf(k);
    if (k === "engine" && status.engine.installed && phase !== "engine") return "done";
    return cur > idx ? "done" : cur === idx ? "active" : "todo";
  };

  return (
    <div className="space-y-4">
      {phase === "idle" || phase === "error" ? (
        <>
          <div className="rounded-xl bg-surface-2 px-4 py-3 text-[14px]">
            <div className="mb-1 font-bold">내려받기 안내</div>
            <ul className="list-disc space-y-0.5 pl-5 text-muted">
              <li>내려받는 용량: 약 <b className="text-text">{model.download_gb.toFixed(1)}GB</b>{!status.engine.installed && " + AI 실행 엔진(Ollama)"}</li>
              <li>인터넷은 <b className="text-text">AI 실행 엔진과 AI 모델 파일을 내려받을 때만</b> 사용합니다.</li>
              <li>교수님의 문서, 질문, 대화 내용은 <b className="text-text">어디로도 전송되지 않습니다.</b></li>
              <li>라이선스: {model.license} ({model.publisher})</li>
            </ul>
          </div>
          {!status.engine.installed && !status.engine.installable && (
            <Notice tone="warn" title="이 빌드에서는 AI 실행 엔진을 설치할 수 없습니다">
              공식 배포 파일의 무결성 확인값이 등록되지 않은 개발용 빌드입니다. 안전을 위해 확인값이 없으면 설치하지 않습니다.
            </Notice>
          )}
          <label className="flex cursor-pointer items-start gap-3 text-[14px]">
            <input type="checkbox" className="mt-1 h-4 w-4 accent-[var(--purple)]" checked={consent} onChange={(e) => setConsent(e.target.checked)} />
            <span>AI 실행 엔진과 AI 모델을 내려받기 위해 인터넷을 사용하는 것에 동의합니다.</span>
          </label>
          <ErrorNotice error={error} />
          <button className="btn-gold w-full py-3 text-[15.5px]" disabled={!consent || (!status.engine.installed && !status.engine.installable)} onClick={begin}>
            <Icon name="download" /> {phase === "error" ? "다시 시도하기" : "이 AI 다운로드하기"}
          </button>
        </>
      ) : (
        <>
          <ol className="flex items-center gap-2 text-[13px]">
            {steps.map((s, i) => {
              const st = stepState(s.key);
              return (
                <li key={s.key} className="flex items-center gap-2">
                  <span className={`flex h-6 w-6 items-center justify-center rounded-full text-[12px] font-bold ${st === "done" ? "bg-ok text-on-primary" : st === "active" ? "bg-gold text-on-primary" : "bg-surface-2 text-muted"}`}>
                    {st === "done" ? <Icon name="check" className="h-3.5 w-3.5" /> : i + 1}
                  </span>
                  <span className={st === "todo" ? "text-muted" : "font-semibold"}>{s.label}</span>
                  {i < steps.length - 1 && <span className="mx-1 h-px w-6 bg-border" />}
                </li>
              );
            })}
          </ol>
          {busy && (
            <div className="rounded-xl border border-border p-4">
              <div className="mb-2 flex items-center justify-between text-[14px]">
                <span className="font-semibold">
                  {phase === "starting" ? "AI 실행 엔진을 시작하는 중" : phase === "testing" ? "AI가 잘 작동하는지 확인하는 중" : progress?.stage ?? "준비하는 중"}
                </span>
                {(phase === "engine" || phase === "model") && !progress?.verifying && <button className="btn-ghost btn-sm" onClick={cancel}>취소</button>}
              </div>
              {(phase === "engine" || phase === "model") && (
                <>
                  <ProgressBar value={progress?.completed_bytes ?? 0} total={progress?.total_bytes ?? 0} />
                  <div className="mt-2 flex justify-between text-[13px] text-muted tabular-nums">
                    <span>{progress && progress.total_bytes > 0 ? `${fmtBytes(progress.completed_bytes)} / ${fmtBytes(progress.total_bytes)} (${Math.floor((progress.completed_bytes / progress.total_bytes) * 100)}%)` : "잠시만 기다려 주세요"}</span>
                    <span>{speed > 0 && !progress?.verifying ? `초당 ${fmtBytes(speed)}` : ""}</span>
                  </div>
                </>
              )}
              {(phase === "starting" || phase === "testing") && <span className="typing"><span /><span /><span /></span>}
            </div>
          )}
          {phase === "done" && (
            <>
              <Notice tone="ok" title="AI 준비가 끝났습니다">
                내려받은 파일을 확인했고, AI가 이 컴퓨터에서 정상적으로 답변했습니다.
                {testReply && <div className="mt-1.5 rounded-lg bg-surface px-3 py-2 text-muted">AI의 첫 인사: “{testReply.reply.slice(0, 120)}”</div>}
              </Notice>
              <button className="btn-primary w-full py-3 text-[15.5px]" onClick={onDone}>{doneLabel}</button>
            </>
          )}
        </>
      )}
    </div>
  );
}
