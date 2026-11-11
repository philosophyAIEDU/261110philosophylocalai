import { Assessment, CatalogModel, HardwareReport, Recommendation, fitLabel, fmtGB, speedLabel } from "./api";
import { Icon } from "./ui";

const tierStyle: Record<string, string> = { light: "bg-ok-soft text-ok", balanced: "bg-gold-soft text-gold", high_quality: "bg-purple-soft text-purple" };

function Dots({ n }: { n: number }) {
  return (
    <span className="inline-flex gap-0.5" aria-label={`5점 중 ${n}점`}>
      {[1, 2, 3, 4, 5].map((i) => <span key={i} className={`h-1.5 w-3.5 rounded-full ${i <= n ? "bg-gold" : "bg-surface-2"}`} />)}
    </span>
  );
}

export function ModelFacts({ m, a }: { m: CatalogModel; a?: Assessment }) {
  return (
    <dl className="grid grid-cols-2 gap-x-4 gap-y-1.5 text-[13px]">
      <dt className="text-muted">내려받는 용량</dt><dd className="tabular-nums">약 {m.download_gb.toFixed(1)}GB</dd>
      <dt className="text-muted">필요한 메모리</dt><dd className="tabular-nums">약 {Math.ceil(m.est_ram_gb)}GB</dd>
      {a && <><dt className="text-muted">이 컴퓨터에서</dt><dd>{fitLabel[a.fit]} · 속도 {speedLabel[a.speed]}</dd></>}
      <dt className="text-muted">한국어</dt><dd><Dots n={m.korean} /></dd>
      <dt className="text-muted">문서 요약</dt><dd><Dots n={m.summarization} /></dd>
      <dt className="text-muted">분석·추론</dt><dd><Dots n={m.reasoning} /></dd>
      <dt className="text-muted">만든 곳</dt><dd>{m.publisher}</dd>
      <dt className="text-muted">라이선스</dt><dd>{m.license}</dd>
    </dl>
  );
}

export function RecommendationCard({ rec, selected, onSelect, installed }: { rec: Recommendation; selected: boolean; onSelect: () => void; installed?: boolean }) {
  const m = rec.assessment.model;
  return (
    <button
      onClick={onSelect}
      aria-pressed={selected}
      className={`card flex h-full flex-col text-left transition-all ${selected ? "border-purple ring-2 ring-purple/30" : "hover:border-purple/40"}`}
    >
      <div className="mb-3 flex items-center justify-between">
        <span className={`chip ${tierStyle[rec.tier]}`}>{rec.tier === "balanced" && <Icon name="spark" className="h-3.5 w-3.5" />}{rec.tier_label}</span>
        {installed && <span className="chip bg-ok-soft text-ok"><Icon name="check" className="h-3.5 w-3.5" />설치됨</span>}
      </div>
      <div className="text-[17px] font-bold">{m.display_name}</div>
      <div className="mb-2 text-[12.5px] text-muted">{m.id}</div>
      <p className="mb-4 text-[13.5px] text-muted">{m.description}</p>
      <div className="mt-auto"><ModelFacts m={m} a={rec.assessment} /></div>
    </button>
  );
}

export function ReasonList({ rec }: { rec: Recommendation }) {
  return (
    <div className="grid gap-4 md:grid-cols-2">
      <div>
        <div className="mb-1.5 text-[13px] font-bold text-muted">추천 이유</div>
        <ul className="space-y-1 text-[14px]">
          {rec.reasons.map((r, i) => <li key={i} className="flex gap-2"><Icon name="check" className="mt-1 h-4 w-4 shrink-0 text-ok" />{r}</li>)}
        </ul>
      </div>
      <div>
        <div className="mb-1.5 text-[13px] font-bold text-muted">알아 두실 점</div>
        <ul className="space-y-1 text-[14px]">
          {rec.cautions.map((r, i) => <li key={i} className="flex gap-2"><Icon name="info" className="mt-1 h-4 w-4 shrink-0 text-warn" />{r}</li>)}
        </ul>
      </div>
    </div>
  );
}

export function vramText(v: HardwareReport["profile"]["gpus"][number]["vram"]): string {
  switch (v.kind) {
    case "exact": return `전용 메모리 ${fmtGB(v.bytes, 0)}`;
    case "at_least": return `전용 메모리 ${fmtGB(v.bytes, 0)} 이상 (정확한 크기 확인 불가)`;
    case "shared": return "시스템 메모리와 함께 사용";
    default: return "메모리 크기 확인 불가";
  }
}

export function HardwareSummary({ hw, compact }: { hw: HardwareReport; compact?: boolean }) {
  const p = hw.profile;
  const accel = { apple_metal: "Apple 칩 그래픽 가속", nvidia_cuda: "NVIDIA 그래픽 가속", amd_gpu: "AMD 그래픽 (환경에 따라 가속)", cpu_only: "그래픽 가속 없음 (CPU로 실행)" }[p.acceleration];
  const rows: [string, string][] = [
    ["운영체제", `${p.os_name} ${p.os_version} (${p.arch})`],
    ["프로세서(CPU)", `${p.cpu_model} · ${p.cpu_physical_cores || p.cpu_logical_cores}코어`],
    ["메모리(RAM)", `전체 ${fmtGB(p.total_ram, 0)} · 지금 사용 가능 ${fmtGB(p.available_ram, 0)}`],
    ["그래픽", p.gpus.length ? p.gpus.map((g) => `${g.name} — ${vramText(g.vram)}`).join(" / ") : "별도 그래픽 장치를 찾지 못했습니다"],
    ["AI 실행 방식", accel],
    ["저장 공간", p.disk_total ? `${fmtGB(p.disk_free, 0)} 남음 (전체 ${fmtGB(p.disk_total, 0)})` : "확인 불가"],
  ];
  return (
    <div>
      {!compact && (
        <ul className="mb-4 space-y-1.5">
          {hw.summary.map((s, i) => <li key={i} className="flex gap-2 text-[15px]"><Icon name="check" className="mt-1 h-4 w-4 shrink-0 text-ok" />{s}</li>)}
        </ul>
      )}
      <dl className="grid grid-cols-[130px_1fr] gap-x-4 gap-y-2 text-[14px]">
        {rows.map(([k, v]) => (<div key={k} className="contents"><dt className="text-muted">{k}</dt><dd className="min-w-0">{v}</dd></div>))}
      </dl>
      {p.notes.length > 0 && <ul className="mt-3 space-y-1 text-[13px] text-warn">{p.notes.map((n, i) => <li key={i}>※ {n}</li>)}</ul>}
    </div>
  );
}
