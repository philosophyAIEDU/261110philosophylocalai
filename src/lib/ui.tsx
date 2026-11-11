// 공통 화면 요소
import { ReactNode, useCallback, useEffect, useRef, useState } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { call, errCode, errMsg, newJobId, on, pickSave } from "./api";

export function Icon({ name, className = "h-5 w-5" }: { name: string; className?: string }) {
  const paths: Record<string, string> = {
    home: "M3 11.5 12 4l9 7.5M5.5 10v9.5h13V10",
    chat: "M4 5.5h16v10.5H9.5L5 20v-4H4z",
    book: "M5 4.5h10.5a3 3 0 0 1 3 3V20H8a3 3 0 0 1-3-3zM5 17a3 3 0 0 1 3-3h10.5",
    users: "M9 11a3.5 3.5 0 1 0 0-7 3.5 3.5 0 0 0 0 7zm-6 9a6 6 0 0 1 12 0M16.5 4.3a3.5 3.5 0 0 1 0 6.4M18 14.5a6 6 0 0 1 3 5.5",
    flask: "M9.5 3.5h5M10.5 3.5v6L5 19a1.3 1.3 0 0 0 1.2 2h11.6A1.3 1.3 0 0 0 19 19l-5.5-9.5v-6M8 15h8",
    briefcase: "M4 8h16v11.5H4zM9 8V5.5h6V8M4 13h16",
    table: "M4 5h16v14H4zM4 10h16M4 14.5h16M10 5v14",
    cpu: "M7 7h10v10H7zM10 3.5V7m4-3.5V7m-4 10v3.5m4-3.5v3.5M3.5 10H7m-3.5 4H7m10-4h3.5M17 14h3.5",
    shield: "M12 3.5 5 6v5.5c0 4.2 2.9 7.6 7 9 4.1-1.4 7-4.8 7-9V6zM9 12l2.2 2.2L15 10.2",
    lock: "M6.5 10.5h11V20h-11zM9 10.5V8a3 3 0 0 1 6 0v2.5",
    download: "M12 4v11m0 0-4-4m4 4 4-4M5 19.5h14",
    check: "M5 12.5 10 17.5 19 7.5",
    x: "M6 6l12 12M18 6 6 18",
    stop: "M7 7h10v10H7z",
    copy: "M9 9h10v11H9zM5 15V4.5h10",
    trash: "M5 7h14M9.5 7V4.5h5V7M7 7l.8 12.5h8.4L17 7",
    plus: "M12 5v14M5 12h14",
    file: "M7 3.5h7l4 4V20.5H7zM14 3.5v4h4",
    send: "M4 12 20 4.5 16 20l-4-6zM12 14l8-9.5",
    sun: "M12 8a4 4 0 1 0 0 8 4 4 0 0 0 0-8zM12 2.5v2.5m0 14v2.5M2.5 12H5m14 0h2.5M5.3 5.3 7 7m10 10 1.7 1.7M5.3 18.7 7 17M17 7l1.7-1.7",
    moon: "M19.5 14.5A8 8 0 0 1 9.5 4.5a8 8 0 1 0 10 10z",
    alert: "M12 4 3 19.5h18zM12 10v4.5m0 2.5v.2",
    search: "M10.5 17a6.5 6.5 0 1 0 0-13 6.5 6.5 0 0 0 0 13zM15.5 15.5 20 20",
    spark: "M12 3.5l1.8 5.2 5.2 1.8-5.2 1.8L12 17.5l-1.8-5.2L5 10.5l5.2-1.8zM18.5 15.5l.8 2.2 2.2.8-2.2.8-.8 2.2-.8-2.2-2.2-.8 2.2-.8z",
    info: "M12 20.5a8.5 8.5 0 1 0 0-17 8.5 8.5 0 0 0 0 17zM12 11v5.5M12 7.8v.2",
  };
  return (
    <svg viewBox="0 0 24 24" className={className} fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d={paths[name] ?? ""} />
    </svg>
  );
}

/** AI 답변 표시. 원격 이미지와 링크 이동을 막는다 (문서 내용이 외부 접속을 유도하지 못하게). */
export function Md({ children }: { children: string }) {
  return (
    <div className="md">
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        skipHtml
        components={{
          a: ({ children: c, href }) => (
            <span title="보안을 위해 링크는 열리지 않습니다">
              {c}
              {href && String(c) !== href ? <span className="text-muted"> ({href})</span> : null}
            </span>
          ),
          img: ({ alt }) => <span className="text-muted">[이미지{alt ? `: ${alt}` : ""} — 표시하지 않음]</span>,
        }}
      >
        {children}
      </ReactMarkdown>
    </div>
  );
}

export function Notice({ tone = "info", children, title }: { tone?: "info" | "ok" | "warn" | "danger"; title?: string; children: ReactNode }) {
  const cls = {
    info: "bg-purple-soft text-text border-purple/20",
    ok: "bg-ok-soft text-text border-ok/25",
    warn: "bg-warn-soft text-text border-warn/25",
    danger: "bg-danger-soft text-text border-danger/25",
  }[tone];
  const icon = { info: "info", ok: "check", warn: "alert", danger: "alert" }[tone];
  const color = { info: "text-purple", ok: "text-ok", warn: "text-warn", danger: "text-danger" }[tone];
  return (
    <div className={`flex gap-3 rounded-xl border px-4 py-3 text-[14px] ${cls}`} role={tone === "danger" ? "alert" : "status"}>
      <Icon name={icon} className={`mt-0.5 h-[18px] w-[18px] shrink-0 ${color}`} />
      <div className="min-w-0">
        {title && <div className="font-bold">{title}</div>}
        <div>{children}</div>
      </div>
    </div>
  );
}

export function ErrorNotice({ error, onDismiss }: { error: string | null; onDismiss?: () => void }) {
  if (!error) return null;
  return (
    <Notice tone="danger">
      <div className="flex items-start justify-between gap-3">
        <span>{error}</span>
        {onDismiss && (
          <button className="text-muted hover:text-text" onClick={onDismiss} aria-label="닫기">
            <Icon name="x" className="h-4 w-4" />
          </button>
        )}
      </div>
    </Notice>
  );
}

export function ProgressBar({ value, total }: { value: number; total: number }) {
  const pct = total > 0 ? Math.min(100, (value / total) * 100) : 0;
  return (
    <div className="h-2.5 w-full overflow-hidden rounded-full bg-surface-2" role="progressbar" aria-valuenow={Math.round(pct)} aria-valuemin={0} aria-valuemax={100}>
      <div className="h-full rounded-full bg-gold transition-[width] duration-300" style={{ width: `${pct}%` }} />
    </div>
  );
}

export function PageHeader({ title, sub, right }: { title: string; sub?: string; right?: ReactNode }) {
  return (
    <div className="mb-6 flex items-start justify-between gap-6">
      <div>
        <h1 className="h1">{title}</h1>
        {sub && <p className="sub mt-1.5 max-w-[60ch]">{sub}</p>}
      </div>
      {right}
    </div>
  );
}

export function Empty({ icon, title, children }: { icon: string; title: string; children?: ReactNode }) {
  return (
    <div className="flex flex-col items-center justify-center rounded-2xl border border-dashed border-border px-6 py-10 text-center">
      <div className="mb-3 rounded-full bg-surface-2 p-3 text-muted"><Icon name={icon} className="h-6 w-6" /></div>
      <div className="font-semibold">{title}</div>
      {children && <div className="sub mt-1 max-w-[46ch]">{children}</div>}
    </div>
  );
}

/** AI 작업(스트리밍) 실행 훅. 진행 단계·실시간 글자·중지를 다룬다. */
export function useAiJob() {
  const [text, setText] = useState("");
  const [stage, setStage] = useState("");
  const [running, setRunning] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const jobRef = useRef<string | null>(null);

  useEffect(() => {
    const subs = [
      on("ai-token", (p) => { if (p.job_id === jobRef.current) setText((t) => t + p.token); }),
      on("ai-stage", (p) => { if (p.job_id === jobRef.current) setStage(p.stage); }),
    ];
    return () => { subs.forEach((s) => s.then((un) => un())); };
  }, []);

  const run = useCallback(async <T,>(fn: (jobId: string) => Promise<T>): Promise<T | null> => {
    const id = newJobId();
    jobRef.current = id;
    setText(""); setStage("AI가 준비하는 중"); setError(null); setRunning(true);
    try {
      return await fn(id);
    } catch (e) {
      if (errCode(e) !== "cancelled") setError(errMsg(e));
      return null;
    } finally {
      setRunning(false); setStage("");
    }
  }, []);

  const stop = useCallback(() => { if (jobRef.current) void call("cancel_job", { jobId: jobRef.current }); }, []);
  const reset = useCallback(() => { setText(""); setError(null); }, []);
  return { text, setText, stage, running, error, setError, run, stop, reset };
}

export function CopyButton({ text }: { text: string }) {
  const [done, setDone] = useState(false);
  return (
    <button
      className="btn-ghost btn-sm"
      onClick={async () => { try { await navigator.clipboard.writeText(text); setDone(true); setTimeout(() => setDone(false), 1500); } catch { /* 클립보드 사용 불가 */ } }}
    >
      <Icon name={done ? "check" : "copy"} className="h-4 w-4" /> {done ? "복사됨" : "복사"}
    </button>
  );
}

/** 결과 저장 버튼 (Word / 텍스트). 저장 위치는 교수님이 직접 고른다. */
export function ExportButtons({ title, markdown }: { title: string; markdown: string }) {
  const [msg, setMsg] = useState<string | null>(null);
  const saveAs = async (kind: "docx" | "md") => {
    setMsg(null);
    try {
      const safe = title.replace(/[\\/:*?"<>|]/g, " ").trim() || "결과";
      const path = await pickSave(`${safe}.${kind}`, kind === "docx" ? "Word 문서" : "텍스트 문서", [kind]);
      if (!path) return;
      if (kind === "docx") await call("export_docx", { title, markdown, path });
      else await call("export_text", { content: markdown, path });
      setMsg("저장했습니다.");
    } catch (e) { setMsg(errMsg(e)); }
  };
  return (
    <div className="flex flex-wrap items-center gap-2">
      <CopyButton text={markdown} />
      <button className="btn-ghost btn-sm" onClick={() => saveAs("docx")}><Icon name="file" className="h-4 w-4" /> Word로 저장</button>
      <button className="btn-ghost btn-sm" onClick={() => saveAs("md")}><Icon name="download" className="h-4 w-4" /> 텍스트로 저장</button>
      {msg && <span className="text-[13px] text-muted">{msg}</span>}
    </div>
  );
}

/** AI 결과 카드: 실시간 표시 + 중지 + 저장 */
export function ResultCard({ title, job, footer }: { title: string; job: ReturnType<typeof useAiJob>; footer?: ReactNode }) {
  if (!job.running && !job.text && !job.error) return null;
  return (
    <section className="card" aria-live="polite">
      <div className="mb-3 flex items-center justify-between gap-3">
        <h2 className="h2">{title}</h2>
        {job.running && (
          <button className="btn-ghost btn-sm" onClick={job.stop}><Icon name="stop" className="h-4 w-4" /> 중지</button>
        )}
      </div>
      {job.running && !job.text && (
        <div className="flex items-center gap-3 text-muted"><span className="typing"><span /><span /><span /></span>{job.stage || "AI가 작성하는 중"}</div>
      )}
      {job.running && job.text && job.stage && <div className="mb-2 text-[13px] text-muted">{job.stage}</div>}
      {job.text && <Md>{job.text}</Md>}
      <ErrorNotice error={job.error} />
      {!job.running && job.text && (
        <div className="mt-4 border-t border-border pt-4">
          <ExportButtons title={title} markdown={job.text} />
          <p className="mt-3 text-[12.5px] text-muted">AI가 만든 초안입니다. 사용하기 전에 내용을 확인해 주세요.</p>
          {footer}
        </div>
      )}
    </section>
  );
}

export function Confirm({ open, title, body, confirmLabel, danger, onConfirm, onCancel }: { open: boolean; title: string; body: ReactNode; confirmLabel: string; danger?: boolean; onConfirm: () => void; onCancel: () => void }) {
  useEffect(() => {
    if (!open) return;
    const h = (e: KeyboardEvent) => { if (e.key === "Escape") onCancel(); };
    window.addEventListener("keydown", h);
    return () => window.removeEventListener("keydown", h);
  }, [open, onCancel]);
  if (!open) return null;
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-6" role="dialog" aria-modal="true" aria-label={title}>
      <div className="card w-full max-w-md shadow-xl">
        <h2 className="h2 mb-2">{title}</h2>
        <div className="sub mb-5">{body}</div>
        <div className="flex justify-end gap-2">
          <button className="btn-ghost" onClick={onCancel}>취소</button>
          <button className={danger ? "btn-danger" : "btn-primary"} onClick={onConfirm} autoFocus>{confirmLabel}</button>
        </div>
      </div>
    </div>
  );
}
