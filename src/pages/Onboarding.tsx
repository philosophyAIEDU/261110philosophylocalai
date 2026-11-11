import { useEffect, useState } from "react";
import { HardwareReport, Purpose, Recommendation, RecommendationResult, call, errMsg } from "../lib/api";
import { ErrorNotice, Icon, Notice } from "../lib/ui";
import { HardwareSummary, RecommendationCard, ReasonList } from "../lib/modelcard";
import { DownloadFlow } from "../lib/download";
import { useApp } from "../App";

export const PURPOSES: { id: Purpose; label: string; desc: string }[] = [
  { id: "general", label: "두루 사용", desc: "대화, 글 초안, 일반 업무" },
  { id: "documents", label: "문서 요약 중심", desc: "강의자료, 회의록 정리" },
  { id: "research", label: "연구자료 분석 중심", desc: "논문 요약·비교" },
  { id: "data", label: "데이터 해석 중심", desc: "Excel 분석 보고서" },
];

export default function Onboarding() {
  const { refresh } = useApp();
  const [step, setStep] = useState(0);
  const [hw, setHw] = useState<HardwareReport | null>(null);
  const [purpose, setPurpose] = useState<Purpose>("general");
  const [recs, setRecs] = useState<RecommendationResult | null>(null);
  const [picked, setPicked] = useState<Recommendation | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (step === 1 && !hw) call<HardwareReport>("detect_hardware").then(setHw).catch((e) => setError(errMsg(e)));
  }, [step, hw]);

  useEffect(() => {
    if (step !== 2) return;
    call<RecommendationResult>("recommend_models", { purpose })
      .then((r) => { setRecs(r); setPicked(r.recommendations.find((x) => x.tier === "balanced") ?? r.recommendations[0] ?? null); })
      .catch((e) => setError(errMsg(e)));
  }, [step, purpose]);

  const finish = async () => { await call("complete_onboarding"); await refresh(); };
  const titles = ["시작하기", "컴퓨터 확인", "AI 고르기", "AI 내려받기"];

  return (
    <div className="flex h-full flex-col overflow-y-auto">
      <header className="flex items-center justify-between px-10 py-6">
        <div className="flex items-center gap-3">
          <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-navy text-[20px] font-bold text-on-primary">Φ</div>
          <div className="text-[15px] font-bold">필로소피 로컬 AI</div>
        </div>
        <ol className="flex items-center gap-2 text-[13px]">
          {titles.map((t, i) => (
            <li key={t} className="flex items-center gap-2">
              <span className={`flex h-6 w-6 items-center justify-center rounded-full text-[12px] font-bold ${i < step ? "bg-ok text-on-primary" : i === step ? "bg-navy text-on-primary" : "bg-surface-2 text-muted"}`}>{i < step ? <Icon name="check" className="h-3.5 w-3.5" /> : i + 1}</span>
              <span className={i === step ? "font-bold" : "text-muted"}>{t}</span>
              {i < titles.length - 1 && <span className="mx-1 h-px w-5 bg-border" />}
            </li>
          ))}
        </ol>
      </header>

      <div className="mx-auto w-full max-w-[980px] flex-1 px-10 pb-12">
        <ErrorNotice error={error} onDismiss={() => setError(null)} />

        {step === 0 && (
          <div className="mx-auto max-w-[680px] pt-8">
            <p className="mb-3 text-[13px] font-bold tracking-wide text-gold">AI로 성장하고, 그 성장을 나눈다</p>
            <h1 className="mb-4 text-[34px] font-bold leading-[1.25] tracking-[-0.015em]">교수님의 연구와 수업을 돕는<br />나만의 AI</h1>
            <p className="sub mb-8 text-[16px]">중요한 자료는 내 컴퓨터에서 안전하게 관리하고, AI의 도움으로 업무를 더 효율적으로 처리하세요.</p>
            <div className="mb-8 space-y-3">
              {[
                ["lock", "내 컴퓨터 안에서만 실행됩니다", "학생 정보, 강의자료, 연구자료가 인터넷으로 나가지 않습니다."],
                ["cpu", "컴퓨터에 맞는 AI를 골라 드립니다", "사양을 자동으로 확인하고 알맞은 AI 2~3개를 추천합니다."],
                ["download", "버튼 한 번으로 준비됩니다", "별도 프로그램 설치나 명령어 입력이 필요 없습니다."],
              ].map(([icon, t, d]) => (
                <div key={t} className="card flex items-start gap-4 py-4">
                  <div className="rounded-xl bg-purple-soft p-2.5 text-purple"><Icon name={icon} /></div>
                  <div><div className="font-bold">{t}</div><div className="sub">{d}</div></div>
                </div>
              ))}
            </div>
            <button className="btn-primary px-7 py-3 text-[15.5px]" onClick={() => setStep(1)}>시작하기</button>
          </div>
        )}

        {step === 1 && (
          <div className="mx-auto max-w-[760px] pt-4">
            <h1 className="h1 mb-2">교수님의 컴퓨터를 확인했습니다</h1>
            <p className="sub mb-6">이 정보는 알맞은 AI를 추천하는 데만 쓰이며, 어디로도 전송되지 않습니다.</p>
            <div className="card mb-6">
              {hw ? <HardwareSummary hw={hw} /> : <div className="flex items-center gap-3 text-muted"><span className="typing"><span /><span /><span /></span>컴퓨터 사양을 확인하는 중</div>}
            </div>
            <div className="flex gap-2">
              <button className="btn-ghost" onClick={() => setStep(0)}>이전</button>
              <button className="btn-primary" disabled={!hw} onClick={() => setStep(2)}>알맞은 AI 추천받기</button>
            </div>
          </div>
        )}

        {step === 2 && (
          <div className="pt-4">
            <h1 className="h1 mb-2">교수님께 맞는 AI를 추천합니다</h1>
            <p className="sub mb-5">주로 어떤 일에 사용하실지 고르면 추천이 달라집니다. 나중에 다른 AI로 바꿀 수 있습니다.</p>
            <div className="mb-6 flex flex-wrap gap-2" role="radiogroup" aria-label="주요 사용 목적">
              {PURPOSES.map((p) => (
                <button key={p.id} role="radio" aria-checked={purpose === p.id} onClick={() => setPurpose(p.id)}
                  className={`rounded-xl border px-4 py-2 text-left text-[14px] ${purpose === p.id ? "border-purple bg-purple-soft font-bold text-purple" : "border-border bg-surface hover:bg-surface-2"}`}>
                  {p.label}<span className="ml-2 text-[12.5px] font-normal text-muted">{p.desc}</span>
                </button>
              ))}
            </div>
            {recs && recs.recommendations.length === 0 && <Notice tone="warn">{recs.explanation}</Notice>}
            {recs && recs.recommendations.length > 0 && (
              <>
                <div className={`mb-5 grid gap-4 ${recs.recommendations.length === 3 ? "md:grid-cols-3" : "md:grid-cols-2"}`}>
                  {recs.recommendations.map((r) => (
                    <RecommendationCard key={r.assessment.model.id} rec={r} selected={picked?.assessment.model.id === r.assessment.model.id} onSelect={() => setPicked(r)} />
                  ))}
                </div>
                {picked && <div className="card mb-5"><ReasonList rec={picked} /></div>}
                <p className="mb-5 text-[13px] text-muted">{recs.explanation} 품질 표시는 개발팀의 예상치이며 측정으로 검증한 값이 아닙니다.</p>
              </>
            )}
            <div className="flex items-center gap-2">
              <button className="btn-ghost" onClick={() => setStep(1)}>이전</button>
              <button className="btn-primary" disabled={!picked} onClick={() => setStep(3)}>이 AI로 계속하기</button>
              <button className="ml-auto text-[13.5px] text-muted underline underline-offset-4" onClick={finish}>지금은 건너뛰기</button>
            </div>
          </div>
        )}

        {step === 3 && picked && (
          <div className="mx-auto max-w-[640px] pt-4">
            <h1 className="h1 mb-2">{picked.assessment.model.display_name} 준비하기</h1>
            <p className="sub mb-6">아래 버튼을 누르면 필요한 것을 모두 자동으로 내려받고 설치합니다.</p>
            <div className="card"><DownloadFlow model={picked.assessment.model} onDone={finish} /></div>
            <div className="mt-4 flex items-center gap-2">
              <button className="btn-ghost" onClick={() => setStep(2)}>다른 AI 고르기</button>
              <button className="ml-auto text-[13.5px] text-muted underline underline-offset-4" onClick={finish}>나중에 내려받기</button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
