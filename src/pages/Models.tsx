import { useCallback, useEffect, useState } from "react";
import { Assessment, CatalogModel, HardwareReport, InstalledModel, RecommendationResult, call, errMsg, fitLabel, fmtBytes, speedLabel } from "../lib/api";
import { Confirm, Empty, ErrorNotice, Icon, Notice, PageHeader } from "../lib/ui";
import { HardwareSummary, ModelFacts, RecommendationCard, ReasonList } from "../lib/modelcard";
import { DownloadFlow } from "../lib/download";
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

  const loadInstalled = useCallback(() => { call<InstalledModel[]>("list_models").then(setInstalled).catch((e) => setError(errMsg(e))); }, []);
  useEffect(() => { loadInstalled(); call<HardwareReport>("detect_hardware").then(setHw).catch(() => {}); }, [loadInstalled]);
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
              {recs.recommendations.map((r) => <RecommendationCard key={r.assessment.model.id} rec={r} installed={isInstalled(r.assessment.model.id)} selected={target?.id === r.assessment.model.id} onSelect={() => setTarget(r.assessment.model)} />)}
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
