import { useEffect, useState } from "react";
import { HardwareReport, InstalledModel, Purpose, Recommendation, RecommendationResult, call, errMsg } from "../lib/api";
import { ErrorNotice, Icon, Notice, SecurityTip } from "../lib/ui";
import { HardwareSummary, RecommendationCard, ReasonList } from "../lib/modelcard";
import { DownloadFlow } from "../lib/download";
import { useApp } from "../App";
import { PasscodeForm, PROFESSOR_RULES } from "./SecurityPanel";

export const PURPOSES: { id: Purpose; label: string; desc: string }[] = [
  { id: "general", label: "두루 사용", desc: "대화, 글 초안, 일반 업무" },
  { id: "documents", label: "문서 요약 중심", desc: "강의자료, 회의록 정리" },
  { id: "research", label: "연구자료 분석 중심", desc: "논문 요약·비교" },
  { id: "data", label: "데이터 해석 중심", desc: "Excel 분석 보고서" },
];

export default function Onboarding() {
  const { refresh, status } = useApp();
  const [installed, setInstalled] = useState<InstalledModel[]>([]);
  const [step, setStep] = useState(0);
  const [hw, setHw] = useState<HardwareReport | null>(null);
  const [purpose, setPurpose] = useState<Purpose>("general");
  const [recs, setRecs] = useState<RecommendationResult | null>(null);
  const [picked, setPicked] = useState<Recommendation | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (step === 1 && !hw) call<HardwareReport>("detect_hardware").then(setHw).catch((e) => setError(errMsg(e)));
  }, [step, hw]);

  // 이미 받은 AI가 있으면 다시 내려받지 않고 바로 쓸 수 있게 한다 (재설치·초기화 후 등)
  useEffect(() => {
    if (step !== 2) return;
    let alive = true;
    call<InstalledModel[]>("list_models").then((l) => { if (alive) setInstalled(l.filter((m) => !m.is_remote)); }).catch((e) => { if (alive) setError(errMsg(e)); });
    return () => { alive = false; };
  }, [step, status.engine.installed]);

  useEffect(() => {
    if (step !== 2) return;
    call<RecommendationResult>("recommend_models", { purpose })
      .then((r) => { setRecs(r); setPicked(r.recommendations.find((x) => x.tier === "balanced") ?? r.recommendations[0] ?? null); })
      .catch((e) => setError(errMsg(e)));
  }, [step, purpose]);

  const [passSet, setPassSet] = useState(false);
  const finish = async () => { await call("complete_onboarding"); await refresh(); };
  const toSecurity = () => setStep(4);
  const titles = ["시작하기", "컴퓨터 확인", "AI 고르기", "AI 내려받기", "안전 설정"];

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
            {installed.length > 0 && (
              <div className="mb-5 flex flex-wrap items-center justify-between gap-3 rounded-xl border border-ok/25 bg-ok-soft px-4 py-3 text-[14px]">
                <span><b>이미 이 컴퓨터에 받은 AI가 있습니다:</b> {installed.map((m) => m.name).join(", ")}. 다시 내려받을 필요가 없습니다.</span>
                {status.engine.installed && installed.map((m) => <button key={m.name} className="btn-primary btn-sm shrink-0" onClick={async () => { try { await call("select_model", { name: m.name }); await refresh(); setStep(4); } catch (e) { setError(errMsg(e)); } }}>{m.name} 사용하기</button>)}
                {!status.engine.installed && <span>같은 AI를 선택하면 실행 엔진만 설치합니다.</span>}
              </div>
            )}
            {recs && recs.recommendations.length === 0 && <Notice tone="warn">{recs.explanation}</Notice>}
            {recs && recs.recommendations.length > 0 && (
              <>
                <div className={`mb-5 grid gap-4 ${recs.recommendations.length === 3 ? "md:grid-cols-3" : "md:grid-cols-2"}`}>
                  {recs.recommendations.map((r) => (
                    <RecommendationCard key={r.assessment.model.id} rec={r} installed={installed.some((m) => m.name === r.assessment.model.id)} selected={picked?.assessment.model.id === r.assessment.model.id} onSelect={() => setPicked(r)} />
                  ))}
                </div>
                {picked && <div className="card mb-5"><ReasonList rec={picked} /></div>}
                <p className="mb-5 text-[13px] text-muted">{recs.explanation} 품질 표시는 개발팀의 예상치이며 측정으로 검증한 값이 아닙니다.</p>
              </>
            )}
            <div className="flex items-center gap-2">
              <button className="btn-ghost" onClick={() => setStep(1)}>이전</button>
              <button className="btn-primary" disabled={!picked} onClick={async () => {
                if (!picked) return;
                const name = picked.assessment.model.id;
                if (status.engine.installed && installed.some((m) => m.name === name)) {
                  try { await call("select_model", { name }); await refresh(); setStep(4); } catch (e) { setError(errMsg(e)); }
                } else setStep(3);
              }}>이 AI로 계속하기</button>
              <button className="ml-auto text-[13.5px] text-muted underline underline-offset-4" onClick={toSecurity}>지금은 건너뛰기</button>
            </div>
          </div>
        )}

        {step === 3 && picked && (
          <div className="mx-auto max-w-[640px] pt-4">
            <h1 className="h1 mb-2">{picked.assessment.model.display_name} 준비하기</h1>
            <p className="sub mb-6">이미 받은 AI가 있는지 먼저 확인하고, 필요한 파일만 설치합니다.</p>
            <div className="card"><DownloadFlow model={picked.assessment.model} onDone={toSecurity} doneLabel="다음: 안전 설정" /></div>
            <div className="mt-4 flex items-center gap-2">
              <button className="btn-ghost" onClick={() => setStep(2)}>다른 AI 고르기</button>
              <button className="ml-auto text-[13.5px] text-muted underline underline-offset-4" onClick={toSecurity}>나중에 내려받기</button>
            </div>
          </div>
        )}

        {step === 4 && (
          <div className="mx-auto max-w-[680px] pt-4">
            <h1 className="h1 mb-2">마지막으로, 안전하게 쓰기 위한 설정</h1>
            <p className="sub mb-5">학생 상담 기록처럼 민감한 정보를 다루신다면 앱 잠금 암호를 꼭 설정해 주세요. 나중에 ‘개인정보 보호 설정’에서도 바꿀 수 있습니다.</p>
            <div className="card mb-4">
              <div className="mb-1 flex items-center gap-2 font-bold"><Icon name="lock" className="h-5 w-5 text-purple" />앱 잠금 암호 {passSet && <span className="chip bg-ok-soft text-ok">설정됨</span>}</div>
              <p className="text-[13.5px] text-muted">암호 없이는 저장된 대화·상담 기록을 열 수 없고, 10분 동안 사용하지 않으면 자동으로 잠깁니다.</p>
              {!passSet && <PasscodeForm hasPasscode={false} onDone={() => { setPassSet(true); void refresh(); }} />}
            </div>
            <div className="card mb-4">
              <div className="mb-2 flex items-center gap-2 font-bold"><Icon name="shield" className="h-5 w-5 text-purple" />이미 켜져 있는 보호 기능</div>
              <ul className="space-y-1 text-[14px]">
                <li className="flex gap-2"><Icon name="check" className="mt-1 h-4 w-4 shrink-0 text-ok" />AI가 이 컴퓨터 안에서만 실행되고, 자료가 인터넷으로 나가지 않습니다.</li>
                <li className="flex gap-2"><Icon name="check" className="mt-1 h-4 w-4 shrink-0 text-ok" />저장되는 기록은 모두 암호화됩니다. 대화 기록 저장은 기본으로 꺼져 있습니다.</li>
                <li className="flex gap-2"><Icon name="check" className="mt-1 h-4 w-4 shrink-0 text-ok" />학생 기록을 AI에 보낼 때 이름을 ‘학생1’처럼 가명으로 바꿉니다.</li>
              </ul>
            </div>
            <SecurityTip title="꼭 기억해 주세요">
              {PROFESSOR_RULES.slice(2, 5).map((r) => r.title).join(" · ")}. 자세한 수칙은 ‘개인정보 보호 설정’ 메뉴에 있습니다.
            </SecurityTip>
            <div className="mt-5 flex items-center gap-2">
              <button className="btn-primary px-6" onClick={finish}>{passSet ? "완료하고 시작하기" : "잠금 없이 시작하기"}</button>
              {!passSet && <span className="text-[13px] text-warn">학생 정보를 다룬다면 잠금 암호를 설정하신 뒤 시작해 주세요.</span>}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
