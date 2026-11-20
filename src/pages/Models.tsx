import { useCallback, useEffect, useState } from "react";
import { Assessment, CatalogModel, HardwareReport, InstalledModel, LibraryView, RecommendationResult, call, errMsg, fitLabel, fmtBytes, speedLabel } from "../lib/api";
import { Confirm, Empty, ErrorNotice, Icon, Notice, PageHeader } from "../lib/ui";
import { HardwareSummary, ModelFacts, RecommendationCard, ReasonList } from "../lib/modelcard";
import { DownloadFlow, customModelInfo } from "../lib/download";
import { PURPOSES } from "./Onboarding";
import { useApp } from "../App";
import { EngineRecovery } from "../lib/engine-recovery";

export default function Models() {
  const { status, refresh } = useApp();
  const [installed, setInstalled] = useState<InstalledModel[]>([]);
  const [hw, setHw] = useState<HardwareReport | null>(null);
  const [recs, setRecs] = useState<RecommendationResult | null>(null);
  const [purpose, setPurpose] = useState(status.purpose);
  const [target, setTarget] = useState<CatalogModel | null>(null);
  const [toDelete, setToDelete] = useState<InstalledModel | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [showAll, setShowAll] = useState(false);
  const [customs, setCustoms] = useState<string[]>([]);
  const [customInput, setCustomInput] = useState("");
  const [customTarget, setCustomTarget] = useState<string | null>(null);
  const [customError, setCustomError] = useState<string | null>(null);
  const [library, setLibrary] = useState<LibraryView | null>(null);
  const [showLibrary, setShowLibrary] = useState(false);
  const [libTag, setLibTag] = useState("전체");
  const [hideTooLarge, setHideTooLarge] = useState(true);

  const loadInstalled = useCallback(() => { call<InstalledModel[]>("list_models").then(setInstalled).catch((e) => setError(errMsg(e))); }, []);
  const loadCustoms = useCallback(() => { call<string[]>("list_custom_models").then(setCustoms).catch(() => {}); }, []);
  useEffect(() => { loadInstalled(); loadCustoms(); call<HardwareReport>("detect_hardware").then(setHw).catch(() => {}); }, [loadInstalled, loadCustoms]);
  useEffect(() => { if (showLibrary && hw && !library) call<LibraryView>("browse_library").then(setLibrary).catch((e) => setCustomError(errMsg(e))); }, [showLibrary, hw, library]);
  const addCustom = async (picked?: string) => {
    setCustomError(null);
    try {
      const name = await call<string>("add_custom_model", { name: picked ?? customInput });
      if (!picked) setCustomInput("");
      loadCustoms(); setTarget(null); setCustomTarget(name);
      if (picked) document.getElementById("custom-download")?.scrollIntoView?.({ behavior: "smooth", block: "center" });
    } catch (e) { setCustomError(errMsg(e)); }
  };
  const removeCustom = async (name: string) => {
    try { await call("remove_custom_model", { name }); if (customTarget === name) setCustomTarget(null); loadCustoms(); } catch (e) { setCustomError(errMsg(e)); }
  };
  const ramGB = hw ? Math.round(hw.profile.total_ram / 1024 ** 3) : null;
  useEffect(() => { if (hw) call<RecommendationResult>("recommend_models", { purpose }).then(setRecs).catch((e) => setError(errMsg(e))); }, [hw, purpose]);

  const isInstalled = (id: string) => installed.some((m) => m.name === id);
  const select = async (name: string) => { try { await call("select_model", { name }); await refresh(); } catch (e) { setError(errMsg(e)); } };
  const picked = recs?.recommendations.find((r) => r.assessment.model.id === target?.id);

  return (
    <div className="space-y-6">
      <PageHeader title="내 AI 모델 관리" sub="이 컴퓨터에 설치된 AI를 확인하고, 새 AI를 내려받거나 지울 수 있습니다." />
      <ErrorNotice error={error} onDismiss={() => setError(null)} />
      <EngineRecovery onRecovered={loadInstalled} />

      <section className="card">
        <h2 className="h2 mb-3">설치된 AI</h2>
        {installed.length === 0 ? (
          <Empty icon="cpu" title="아직 설치된 AI가 없습니다">아래 추천 목록에서 AI를 골라 내려받아 주세요.</Empty>
        ) : (
          <ul className="space-y-2">
            {installed.map((m) => {
              const current = status.selected_model === m.name;
              return (
                <li key={m.name} className={`flex flex-wrap items-center gap-3 rounded-xl border px-4 py-3 ${current ? "border-purple bg-purple-soft" : "border-border"}`}>
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-2 font-bold">{m.name}{current && <span className="chip bg-surface text-purple"><Icon name="check" className="h-3.5 w-3.5" />사용 중</span>}{m.is_remote && <span className="chip bg-danger-soft text-danger">원격 모델 — 사용 불가</span>}</div>
                    <div className="text-[12.5px] text-muted tabular-nums">{fmtBytes(m.size_bytes)}{m.parameter_size ? ` · 규모 ${m.parameter_size}` : ""}</div>
                  </div>
                  {!current && !m.is_remote && status.engine.installed && <button className="btn-ghost btn-sm" onClick={() => select(m.name)}>이 AI 사용하기</button>}
                  {!status.engine.installed && recs?.all.find((a) => a.model.id === m.name) && <button className="btn-primary btn-sm" onClick={() => setTarget(recs!.all.find((a) => a.model.id === m.name)!.model)}>실행 엔진만 설치하기</button>}
                  <button className="btn-danger btn-sm" onClick={() => setToDelete(m)}><Icon name="trash" className="h-4 w-4" /> 삭제</button>
                </li>
              );
            })}
          </ul>
        )}
      </section>

      <section>
        <div className="mb-3 flex flex-wrap items-center justify-between gap-3">
          <h2 className="h2">교수님 컴퓨터에 맞는 추천 AI</h2>
          <select className="input w-auto py-1.5" value={purpose} onChange={(e) => setPurpose(e.target.value as typeof purpose)} aria-label="주요 사용 목적">{PURPOSES.map((p) => <option key={p.id} value={p.id}>{p.label}</option>)}</select>
        </div>
        {!recs ? <div className="card text-muted">컴퓨터 사양을 확인하는 중…</div> : recs.recommendations.length === 0 ? <Notice tone="warn">{recs.explanation}</Notice> : (
          <>
            <div className={`grid gap-4 ${recs.recommendations.length === 3 ? "md:grid-cols-3" : "md:grid-cols-2"}`}>
              {recs.recommendations.map((r) => <RecommendationCard key={r.assessment.model.id} rec={r} installed={isInstalled(r.assessment.model.id)} selected={target?.id === r.assessment.model.id} onSelect={() => { setCustomTarget(null); setTarget(r.assessment.model); }} />)}
            </div>
            <p className="mt-3 text-[13px] text-muted">{recs.explanation} 품질 표시는 개발팀의 예상치이며 측정으로 검증한 값이 아닙니다.</p>
          </>
        )}
      </section>

      {target && (
        <section className="card">
          <div className="mb-4 flex items-start justify-between gap-3">
            <div><h2 className="h2">{target.display_name}</h2><div className="text-[12.5px] text-muted">{target.id}</div></div>
            <button className="p-1 text-muted hover:text-text" aria-label="닫기" onClick={() => setTarget(null)}><Icon name="x" /></button>
          </div>
          {picked && <div className="mb-5"><ReasonList rec={picked} /></div>}
          {isInstalled(target.id) && status.engine.installed ? <Notice tone="ok">이미 설치된 AI입니다. 위 목록에서 ‘이 AI 사용하기’를 눌러 바꿀 수 있습니다.</Notice> : <DownloadFlow key={target.id} model={target} doneLabel="완료" onDone={() => { setTarget(null); loadInstalled(); }} />}
        </section>
      )}

      <section className="card">
        <h2 className="h2 mb-1">ollama.com의 다른 AI 직접 추가하기</h2>
        <p className="mb-3 text-[13.5px] text-muted">추천 목록에 없는 AI도 <b className="text-text">ollama.com 공식 모델 목록</b>(ollama.com/library)에 있으면 이름으로 추가해 이 컴퓨터에서 쓸 수 있습니다. 예: <code>qwen3:8b</code>, <code>exaone3.5:7.8b</code>, <code>llama3.2:3b</code></p>
        <div className="flex flex-wrap gap-2">
          <input className="input min-w-[220px] flex-1" value={customInput} onChange={(e) => setCustomInput(e.target.value)} onKeyDown={(e) => { if (e.key === "Enter" && customInput.trim()) void addCustom(); }}
            placeholder="모델 이름:크기 (예: qwen3:8b)" aria-label="추가할 모델 이름" spellCheck={false} autoCapitalize="off" />
          <button className="btn-primary" disabled={!customInput.trim()} onClick={() => addCustom()}><Icon name="plus" className="h-4 w-4" /> 추가</button>
        </div>
        <ErrorNotice error={customError} onDismiss={() => setCustomError(null)} />

        <div className="mt-3 rounded-xl border border-border">
          <button className="flex w-full items-center justify-between px-4 py-3 text-left" onClick={() => setShowLibrary((v) => !v)} aria-expanded={showLibrary}>
            <span className="font-bold">이름을 몰라도 괜찮아요 — 목록에서 고르기</span><span className="text-[13px] text-muted">{showLibrary ? "접기" : "펼치기"}</span>
          </button>
          {showLibrary && (
            <div className="border-t border-border px-4 py-3">
              {!library ? <div className="text-muted">목록을 준비하는 중…</div> : (() => {
                const tags = ["전체", ...Array.from(new Set(library.models.flatMap((e) => e.model.tags)))];
                const rows = library.models
                  .filter((e) => libTag === "전체" || e.model.tags.includes(libTag))
                  .filter((e) => !hideTooLarge || e.fit !== "too_large")
                  .sort((a, b) => a.model.download_gb - b.model.download_gb);
                return (
                  <>
                    <div className="mb-3 flex flex-wrap items-center gap-2">
                      {tags.map((t) => <button key={t} className={`chip cursor-pointer ${libTag === t ? "bg-purple text-on-primary" : "bg-surface-2 text-text"}`} onClick={() => setLibTag(t)} aria-pressed={libTag === t}>{t}</button>)}
                      <label className="ml-auto flex cursor-pointer items-center gap-1.5 text-[13px] text-muted"><input type="checkbox" className="h-4 w-4 accent-[var(--purple)]" checked={hideTooLarge} onChange={(e) => setHideTooLarge(e.target.checked)} /> 이 컴퓨터에 너무 큰 AI 숨기기</label>
                    </div>
                    {rows.length === 0 ? <div className="text-muted">조건에 맞는 AI가 없습니다.</div> : (
                      <ul className="space-y-2">
                        {rows.map(({ model: m, fit }) => {
                          const added = customs.includes(m.id);
                          return (
                            <li key={m.id} className={`flex flex-wrap items-start gap-3 rounded-xl border border-border px-3.5 py-3 ${fit === "too_large" ? "opacity-60" : ""}`}>
                              <div className="min-w-0 flex-1">
                                <div className="flex flex-wrap items-baseline gap-x-2"><span className="font-bold">{m.display_name}</span><code className="text-[12px] text-muted">{m.id}</code></div>
                                <p className="text-[13.5px]">{m.description}</p>
                                <div className="mt-1 flex flex-wrap gap-x-3 gap-y-1 text-[12.5px] text-muted tabular-nums">
                                  <span>약 {m.download_gb < 1 ? `${Math.round(m.download_gb * 1000)}MB` : `${m.download_gb}GB`}</span><span>{m.publisher}</span>
                                  <span className={/비상업/.test(m.license) ? "text-warn" : ""}>{m.license}</span>
                                  {m.tags.map((t) => <span key={t}>#{t}</span>)}
                                </div>
                              </div>
                              <div className="flex flex-col items-end gap-1.5">
                                {fit && <span className={`chip ${fit === "comfortable" ? "bg-ok-soft text-ok" : fit === "tight" ? "bg-warn-soft text-warn" : "bg-surface-2 text-muted"}`}>{fitLabel[fit]}</span>}
                                {isInstalled(m.id) ? <span className="chip bg-ok-soft text-ok">설치됨</span>
                                  : fit === "too_large" ? null
                                  : <button className="btn-ghost btn-sm" onClick={() => addCustom(m.id)}><Icon name={added ? "download" : "plus"} className="h-4 w-4" /> {added ? "내려받기" : "추가하고 내려받기"}</button>}
                              </div>
                            </li>
                          );
                        })}
                      </ul>
                    )}
                    <p className="mt-3 text-[12.5px] text-muted">{library.note} (목록 기준일 {library.list_version}) 이 목록은 앱에 들어 있어 인터넷을 쓰지 않으며, 앱을 업데이트하면 새 모델이 추가됩니다. 목록에 없는 최신 모델은 ollama.com/library 에서 이름을 확인해 위 칸에 넣어 주세요.</p>
                  </>
                );
              })()}
            </div>
          )}
        </div>

        <Notice tone="warn" title="직접 추가하기 전에 확인해 주세요">
          <ul className="mt-1 list-disc space-y-0.5 pl-5">
            <li><b>컴퓨터 성능:</b> 앱이 미리 확인하지 못합니다. {ramGB ? <>이 컴퓨터의 메모리는 약 <b>{ramGB}GB</b>입니다. </> : null}ollama.com에 표시된 모델 파일 크기가 메모리의 절반을 넘으면 매우 느리거나 실행되지 않을 수 있습니다.</li>
            <li><b>라이선스:</b> 모델마다 사용 조건이 다릅니다. ‘비상업’ 표시 모델은 사용 범위가 제한되니 ollama.com 모델 페이지에서 확인해 주세요.</li>
            <li><b>보안:</b> ollama.com 공식 목록의 모델만 추가할 수 있습니다. 다른 사이트의 모델이나 클라우드(원격) 모델은 막혀 있어, 추가한 모델도 이 컴퓨터 안에서만 실행됩니다.</li>
            <li>추천 목록의 AI와 달리 한국어 실력·답변 품질은 개발팀이 확인하지 않았습니다.</li>
          </ul>
        </Notice>
        {customs.length > 0 && (
          <ul className="mt-3 space-y-2">
            {customs.map((n) => (
              <li key={n} className={`flex flex-wrap items-center gap-3 rounded-xl border px-4 py-2.5 ${customTarget === n ? "border-purple bg-purple-soft" : "border-border"}`}>
                <span className="min-w-0 flex-1 font-semibold">{n} <span className="text-[12px] font-normal text-muted">직접 추가</span></span>
                {isInstalled(n) ? <span className="chip bg-ok-soft text-ok">설치됨</span> : <button className="btn-ghost btn-sm" onClick={() => { setTarget(null); setCustomTarget(n); }}><Icon name="download" className="h-4 w-4" /> 내려받기</button>}
                <button className="btn-ghost btn-sm" onClick={() => removeCustom(n)} title="목록에서만 빼며, 이미 받은 모델 파일은 위 ‘설치된 AI’에서 삭제합니다">목록에서 빼기</button>
              </li>
            ))}
          </ul>
        )}
        {customTarget && (
          <div id="custom-download" className="mt-4 rounded-xl border border-border p-4">
            <div className="mb-3 flex items-start justify-between gap-3">
              <div><div className="font-bold">{customTarget}</div><div className="text-[12.5px] text-muted">ollama.com/library/{customTarget.split(":")[0]}</div></div>
              <button className="p-1 text-muted hover:text-text" aria-label="닫기" onClick={() => setCustomTarget(null)}><Icon name="x" /></button>
            </div>
            <DownloadFlow key={customTarget} custom model={customModelInfo(customTarget)} doneLabel="완료" onDone={() => { setCustomTarget(null); loadInstalled(); }} />
          </div>
        )}
      </section>

      <section className="card">
        <button className="flex w-full items-center justify-between text-left" onClick={() => setShowAll((v) => !v)} aria-expanded={showAll}>
          <h2 className="h2">전체 AI 목록과 컴퓨터 정보</h2><span className="text-[13px] text-muted">{showAll ? "접기" : "펼치기"}</span>
        </button>
        {showAll && (
          <div className="mt-4 space-y-5">
            {hw && <div className="rounded-xl bg-surface-2 p-4"><HardwareSummary hw={hw} compact /></div>}
            <div className="grid gap-3 md:grid-cols-2">
              {(recs?.all ?? []).map((a: Assessment) => (
                <div key={a.model.id} className={`rounded-xl border border-border p-4 ${a.fit === "too_large" ? "opacity-60" : ""}`}>
                  <div className="mb-2 flex items-center justify-between gap-2">
                    <div><div className="font-bold">{a.model.display_name}</div><div className="text-[12px] text-muted">{a.model.id} · {a.model.quantization}</div></div>
                    {isInstalled(a.model.id) ? <span className="chip bg-ok-soft text-ok">설치됨</span> : a.fit === "too_large" ? <span className="chip bg-surface-2 text-muted">{fitLabel[a.fit]}</span> : <button className="btn-ghost btn-sm" onClick={() => { setTarget(a.model); window.scrollTo?.(0, 0); }}>내려받기</button>}
                  </div>
                  <ModelFacts m={a.model} a={a} />
                  {a.fit !== "too_large" && a.speed === "slow" && <p className="mt-2 text-[12.5px] text-warn">이 컴퓨터에서는 응답이 {speedLabel[a.speed]} 수 있습니다.</p>}
                </div>
              ))}
            </div>
            <p className="text-[12.5px] text-muted">AI 실행 엔진: {status.engine_name} (MIT 라이선스) · 모델 정보 기준일: {recs ? "앱 내장 목록" : "-"}</p>
          </div>
        )}
      </section>

      <Confirm open={!!toDelete} danger title="AI 모델을 삭제할까요?" confirmLabel="삭제"
        body={<>“{toDelete?.name}” 모델 파일({toDelete ? fmtBytes(toDelete.size_bytes) : ""})을 이 컴퓨터에서 지웁니다. 필요하면 다시 내려받을 수 있습니다.</>}
        onCancel={() => setToDelete(null)}
        onConfirm={async () => { if (toDelete) { try { await call("delete_model", { name: toDelete.name }); await refresh(); loadInstalled(); } catch (e) { setError(errMsg(e)); } } setToDelete(null); }} />
    </div>
  );
}
