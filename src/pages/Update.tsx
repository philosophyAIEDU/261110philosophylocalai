import { useEffect, useRef, useState } from "react";
import { UpdateInfo, call, errCode, errMsg, fmtBytes, newJobId, on } from "../lib/api";
import { ErrorNotice, Icon, Md, Notice, PageHeader, ProgressBar, SecurityTip } from "../lib/ui";
import { useApp } from "../App";

type Phase = "idle" | "checking" | "checked" | "downloading" | "downloaded";

export default function UpdatePage() {
  const { status, refresh } = useApp();
  const [phase, setPhase] = useState<Phase>("idle");
  const [info, setInfo] = useState<UpdateInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [progress, setProgress] = useState<{ stage: string; total_bytes: number; completed_bytes: number } | null>(null);
  const [fileName, setFileName] = useState<string | null>(null);
  const [consentDraft, setConsentDraft] = useState(status.update_consent);
  const jobRef = useRef<string | null>(null);

  useEffect(() => {
    const sub = on("update-progress", (p) => { if (p.job_id === jobRef.current) setProgress(p.progress); });
    return () => { sub.then((un) => un()); };
  }, []);

  const check = async () => {
    setError(null); setInfo(null); setFileName(null); setPhase("checking");
    try {
      if (!status.update_consent) { await call("set_update_consent", { consent: true }); await refresh(); }
      setInfo(await call<UpdateInfo>("check_update"));
      setPhase("checked");
    } catch (e) { setError(errMsg(e)); setPhase("idle"); }
  };

  const download = async () => {
    setError(null); setProgress(null); setPhase("downloading");
    jobRef.current = newJobId();
    try {
      setFileName(await call<string>("download_update", { jobId: jobRef.current }));
      setPhase("downloaded");
    } catch (e) {
      if (errCode(e) !== "cancelled") setError(errMsg(e));
      setPhase("checked");
    }
  };

  const isWindows = navigator.userAgent.includes("Windows");

  return (
    <div className="space-y-6">
      <PageHeader title="앱 업데이트" sub="새 버전이 나왔는지 확인하고, 새 기능이 담긴 설치 파일을 안전하게 내려받습니다." />

      <section className="card">
        <div className="flex flex-wrap items-center justify-between gap-4">
          <div>
            <div className="label">지금 사용 중인 버전</div>
            <div className="text-[22px] font-bold tabular-nums">v{status.app_version}</div>
          </div>
          <button className="btn-primary px-6" onClick={check} disabled={phase === "checking" || phase === "downloading" || !consentDraft}>
            <Icon name="search" className="h-4 w-4" /> {phase === "checking" ? "확인하는 중…" : "업데이트 확인하기"}
          </button>
        </div>
        {!status.update_consent && (
          <label className="mt-4 flex cursor-pointer items-start gap-3 rounded-xl bg-surface-2 px-4 py-3 text-[14px]">
            <input type="checkbox" className="mt-1 h-4 w-4 accent-[var(--purple)]" checked={consentDraft} onChange={(e) => setConsentDraft(e.target.checked)} />
            <span>업데이트를 확인할 때 이 앱의 공식 저장소(GitHub)에 접속하는 것에 동의합니다.
              <span className="block text-[13px] text-muted">GitHub에는 교수님의 문서·대화·기록이 전혀 전송되지 않습니다. GitHub가 알 수 있는 것은 접속한 인터넷 주소(IP)와 시각뿐입니다. 버튼을 누를 때만 접속하며, 자동으로 확인하지 않습니다.</span></span>
          </label>
        )}
        <label className="mt-4 flex cursor-pointer items-center gap-2.5 text-[14px]">
          <input type="checkbox" className="h-4 w-4 accent-[var(--purple)]" checked={status.include_prerelease}
            onChange={async (e) => { await call("set_include_prerelease", { enabled: e.target.checked }); await refresh(); }} />
          <span>시험판(베타) 버전도 받기 <span className="text-muted">— 새 기능을 먼저 쓸 수 있지만 충분히 검증되지 않았을 수 있습니다</span></span>
        </label>
      </section>

      <ErrorNotice error={error} onDismiss={() => setError(null)} />

      {info && !info.update_available && (
        <Notice tone="ok" title="최신 버전을 사용하고 계십니다">
          {info.latest_version ? `가장 최근 버전은 v${info.latest_version}입니다.` : "아직 공개된 버전이 없습니다."}
        </Notice>
      )}

      {info && info.update_available && (
        <section className="card">
          <div className="mb-3 flex flex-wrap items-center gap-2">
            <span className="chip bg-gold-soft text-gold"><Icon name="spark" className="h-3.5 w-3.5" />새 버전</span>
            {info.is_prerelease && <span className="chip bg-warn-soft text-warn">시험판</span>}
            {info.published_at && <span className="text-[13px] text-muted">{info.published_at.slice(0, 10)} 공개</span>}
          </div>
          <h2 className="h2 mb-1 text-[19px]">{info.title ?? `v${info.latest_version}`}</h2>
          <p className="sub mb-4 text-[14px]">v{info.current_version} → <b className="text-text">v{info.latest_version}</b></p>
          {info.notes && (
            <details className="mb-4 rounded-xl border border-border px-4 py-2" open>
              <summary className="cursor-pointer py-1 font-semibold">새 버전에서 달라진 점</summary>
              <div className="mt-2 max-h-[360px] overflow-y-auto"><Md>{info.notes}</Md></div>
            </details>
          )}

          {info.asset ? (
            <>
              {phase === "checked" && (
                <button className="btn-gold w-full py-3 text-[15.5px]" onClick={download}>
                  <Icon name="download" /> 새 버전 내려받기 ({fmtBytes(info.asset.size)})
                </button>
              )}
              {phase === "downloading" && (
                <div className="rounded-xl border border-border p-4">
                  <div className="mb-2 flex items-center justify-between text-[14px]">
                    <span className="font-semibold">{progress?.stage ?? "준비하는 중"}</span>
                    <button className="btn-ghost btn-sm" onClick={() => jobRef.current && call("cancel_job", { jobId: jobRef.current })}>취소</button>
                  </div>
                  <ProgressBar value={progress?.completed_bytes ?? 0} total={progress?.total_bytes ?? info.asset.size} />
                  <div className="mt-2 text-[13px] text-muted tabular-nums">{fmtBytes(progress?.completed_bytes ?? 0)} / {fmtBytes(info.asset.size)}</div>
                </div>
              )}
              {phase === "downloaded" && (
                <div className="space-y-3">
                  <Notice tone="ok" title="내려받기와 무결성 확인을 마쳤습니다">
                    ‘{fileName}’이(가) 공식 릴리스에 기록된 값(SHA-256)과 정확히 일치합니다.
                  </Notice>
                  <div className="flex flex-wrap gap-2">
                    <button className="btn-primary" onClick={async () => { try { await call("open_installer"); } catch (e) { setError(errMsg(e)); } }}>
                      <Icon name="download" className="h-4 w-4" /> {isWindows ? "설치 시작 (앱이 닫힙니다)" : "설치 파일 열기"}
                    </button>
                    <button className="btn-ghost" onClick={async () => { try { await call("reveal_installer"); } catch (e) { setError(errMsg(e)); } }}>폴더에서 보기</button>
                  </div>
                  <p className="text-[13px] text-muted">
                    {isWindows
                      ? "설치 프로그램이 열리면 안내에 따라 진행해 주세요. 서명이 없는 시험판은 ‘Windows의 PC 보호’ 창에서 ‘추가 정보 → 실행’을 눌러야 합니다."
                      : "열린 창에서 앱을 ‘응용 프로그램’ 폴더로 끌어다 놓아 기존 앱을 바꿔 주세요. 그 전에 이 앱을 종료해 주세요."}
                    {" "}저장된 대화·상담 기록·설정과 AI 모델은 업데이트 후에도 그대로 남습니다.
                  </p>
                </div>
              )}
            </>
          ) : (
            <Notice tone="warn" title="자동으로 내려받을 수 없습니다">
              {info.asset_note} <button className="underline" onClick={() => call("open_release_page")}>릴리스 페이지 열기</button>
            </Notice>
          )}
        </section>
      )}

      <SecurityTip title="안전한 업데이트">
        새 버전은 이 앱의 공식 GitHub 저장소에서만 받습니다. 내려받은 파일은 GitHub에 기록된 SHA-256 값과 한 바이트라도 다르면 자동으로 삭제됩니다.
        이메일·메신저로 받은 설치 파일은 열지 마시고, 업데이트는 꼭 이 화면이나 공식 릴리스 페이지에서 받아 주세요.
      </SecurityTip>
      <p className="text-[13px] text-muted">
        모든 버전 보기: <button className="underline" onClick={() => call("open_release_page")}>공식 릴리스 페이지</button> (기본 브라우저로 열립니다)
      </p>
    </div>
  );
}
