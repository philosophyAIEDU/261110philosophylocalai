// 강의자료 분석 / 연구자료 분석 / 행정업무 도우미 — 같은 화면 구성을 쓴다.
import { useEffect, useState } from "react";
import { ChatMessage, ChatOutcome, Chunk, DocInfo, TaskDef, call, errMsg } from "../lib/api";
import { Empty, ErrorNotice, Icon, Md, Notice, PageHeader, ResultCard, SecurityTip, useAiJob } from "../lib/ui";
import { NeedModel, useApp } from "../App";

const META: Record<string, { title: string; sub: string; pasteLabel: string; pastePlaceholder: string; qa: boolean; note?: string; tip: string }> = {
  lecture: { title: "강의자료 분석", sub: "강의자료를 불러와 요약, 핵심 개념, 수업 목표, 토론 질문, 퀴즈 초안을 만듭니다.", pasteLabel: "또는 내용을 직접 붙여 넣기", pastePlaceholder: "강의 노트나 자료 내용을 붙여 넣으세요", qa: true, tip: "불러온 파일은 이 컴퓨터의 메모리에서만 읽고 복사본을 만들지 않으며, 다른 메뉴로 이동하거나 앱이 잠기면 지워집니다. 학생 이름이 들어간 자료는 꼭 필요한 부분만 불러와 주세요." },
  research: { title: "연구자료 분석", sub: "논문과 연구자료를 요약·비교하고, 주제별로 정리합니다.", pasteLabel: "또는 내용을 직접 붙여 넣기", pastePlaceholder: "초록이나 메모를 붙여 넣으세요", qa: true, note: "AI는 자료에 없는 참고문헌이나 연구 결과를 만들어 내지 않도록 설정되어 있지만, 인용 전에는 반드시 원문을 확인해 주세요.", tip: "미발표 연구자료도 이 컴퓨터 밖으로 나가지 않습니다. 다만 결과를 Word로 저장하거나 복사하면 그 파일은 암호화되지 않으니, 공유 폴더·메신저로 옮길 때 주의해 주세요." },
  admin: { title: "행정업무 도우미", sub: "회의 메모나 자료를 바탕으로 회의록, 보고서, 공문 초안을 작성합니다.", pasteLabel: "메모·내용 입력", pastePlaceholder: "회의 메모, 행사 내용, 전달할 사항 등을 적어 주세요", qa: false, tip: "공문·보고서 초안에 들어간 개인정보(연락처, 학번 등)는 실제 문서로 보내기 전에 꼭 필요한지 다시 확인해 주세요. 저장한 파일은 암호화되지 않습니다." },
};

export default function TaskPage({ category }: { category: "lecture" | "research" | "admin" }) {
  const meta = META[category];
  const { status } = useApp();
  const [tasks, setTasks] = useState<TaskDef[]>([]);
  const [docs, setDocs] = useState<DocInfo[]>([]);
  const [pasted, setPasted] = useState("");
  const [extra, setExtra] = useState("");
  const [active, setActive] = useState<TaskDef | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const job = useAiJob();
  const qaJob = useAiJob();
  const [question, setQuestion] = useState("");
  const [qaLog, setQaLog] = useState<{ q: string; a: string; evidence: Chunk[] }[]>([]);
  const ready = !!status.selected_model && status.engine.running;
  const hasMaterial = docs.length > 0 || pasted.trim().length > 0;
  const busy = job.running || qaJob.running;

  useEffect(() => { call<TaskDef[]>("list_tasks").then((t) => setTasks(t.filter((x) => x.category === category))).catch(() => {}); }, [category]);
  // 화면을 떠나면 불러온 문서를 메모리에서 지운다.
  useEffect(() => () => { void call("clear_loaded_files"); }, []);

  const addFile = async () => {
    setError(null);
    setLoading(true);
    try { const d = await call<DocInfo | null>("pick_document"); if (d) setDocs((x) => [...x, d]); }
    catch (e) { setError(errMsg(e)); }
    setLoading(false);
  };
  const removeDoc = (id: string) => { void call("remove_document", { id }); setDocs((x) => x.filter((d) => d.id !== id)); };

  const runTask = async (t: TaskDef) => {
    if (busy) return;
    setActive(t);
    await job.run((jobId) => call<ChatOutcome>("run_task", { jobId, taskId: t.id, docIds: docs.map((d) => d.id), pasted, extra }));
  };

  const ask = async () => {
    const q = question.trim();
    if (!q || busy) return;
    setQuestion("");
    const history: ChatMessage[] = qaLog.slice(-2).flatMap((x) => [{ role: "user" as const, content: x.q }, { role: "assistant" as const, content: x.a }]);
    const res = await qaJob.run((jobId) => call<{ outcome: ChatOutcome; evidence: Chunk[] }>("doc_qa", { jobId, docIds: docs.map((d) => d.id), question: q, history }));
    if (res?.outcome.content) setQaLog((l) => [...l, { q, a: res.outcome.content, evidence: res.evidence }]);
    qaJob.setText("");
  };

  return (
    <div className="space-y-6">
      <PageHeader title={meta.title} sub={meta.sub} />
      <NeedModel />
      {meta.note && <Notice tone="info">{meta.note}</Notice>}
      <SecurityTip title="보안 유의사항">{meta.tip}</SecurityTip>

      <section className="card">
        <div className="mb-3 flex items-center justify-between">
          <h2 className="h2">1. 자료 준비</h2>
          <button className="btn-ghost btn-sm" onClick={addFile} disabled={loading || busy}><Icon name="plus" className="h-4 w-4" /> {loading ? "읽는 중…" : "파일 불러오기"}</button>
        </div>
        <ErrorNotice error={error} onDismiss={() => setError(null)} />
        {docs.length === 0 ? (
          <Empty icon="file" title="불러온 파일이 없습니다">PDF, Word(.docx), PowerPoint(.pptx), 텍스트 파일을 불러올 수 있습니다. 파일은 이 컴퓨터 안에서만 읽습니다.</Empty>
        ) : (
          <ul className="space-y-2">
            {docs.map((d) => (
              <li key={d.id} className="flex items-start gap-3 rounded-xl border border-border px-3.5 py-2.5">
                <Icon name="file" className="mt-0.5 h-5 w-5 shrink-0 text-gold" />
                <div className="min-w-0 flex-1">
                  <div className="truncate font-semibold">{d.file_name}</div>
                  <div className="text-[12.5px] text-muted tabular-nums">글자 {d.char_count.toLocaleString("ko-KR")}자{d.units ? ` · ${d.kind === "pptx" ? "슬라이드" : "쪽"} ${d.units}` : ""}</div>
                  {d.warnings.map((w, i) => <div key={i} className="text-[12.5px] text-warn">※ {w}</div>)}
                </div>
                <button className="p-1 text-muted hover:text-danger" aria-label={`${d.file_name} 제거`} onClick={() => removeDoc(d.id)} disabled={busy}><Icon name="x" className="h-4 w-4" /></button>
              </li>
            ))}
          </ul>
        )}
        <div className="mt-4">
          <label className="label" htmlFor="pasted">{meta.pasteLabel}</label>
          <textarea id="pasted" className="input min-h-[96px]" placeholder={meta.pastePlaceholder} value={pasted} onChange={(e) => setPasted(e.target.value)} />
        </div>
      </section>

      <section className="card">
        <h2 className="h2 mb-3">2. 할 일 고르기</h2>
        <div className="grid gap-2.5 sm:grid-cols-2 lg:grid-cols-3">
          {tasks.map((t) => (
            <button key={t.id} disabled={!ready || !hasMaterial || busy} onClick={() => runTask(t)}
              className={`rounded-xl border px-4 py-3 text-left transition-colors disabled:cursor-not-allowed disabled:opacity-50 ${active?.id === t.id && (job.running || job.text) ? "border-purple bg-purple-soft" : "border-border hover:border-purple/50 hover:bg-surface-2"}`}>
              <div className="font-bold">{t.label}</div>
              <div className="text-[13px] text-muted">{t.description}</div>
            </button>
          ))}
        </div>
        {tasks.some((t) => t.extra_hint) && (
          <div className="mt-4">
            <label className="label" htmlFor="extra">덧붙일 내용 (선택) — 예: {tasks.filter((t) => t.extra_hint).map((t) => t.extra_hint.replace(" (선택)", "")).slice(0, 2).join(", ")}</label>
            <input id="extra" className="input" value={extra} onChange={(e) => setExtra(e.target.value)} placeholder="필요한 경우에만 입력하세요" />
          </div>
        )}
        {!hasMaterial && <p className="mt-3 text-[13px] text-muted">자료를 불러오거나 내용을 입력하면 버튼이 켜집니다.</p>}
      </section>

      <ResultCard title={active?.label ?? "결과"} job={job} />

      {meta.qa && (
        <section className="card">
          <h2 className="h2 mb-1">자료에 질문하기</h2>
          <p className="sub mb-4 text-[13.5px]">불러온 파일의 내용에 근거해서만 답하고, 근거가 된 부분을 함께 보여 드립니다.</p>
          <div className="space-y-4">
            {qaLog.map((x, i) => (
              <div key={i} className="space-y-2">
                <div className="flex justify-end"><div className="max-w-[85%] rounded-2xl rounded-br-md bg-navy px-4 py-2 text-on-primary">{x.q}</div></div>
                <div className="rounded-xl border border-border p-4">
                  <Md>{x.a}</Md>
                  <details className="mt-3 text-[13px]">
                    <summary className="cursor-pointer font-semibold text-muted">답변에 사용한 자료 부분 {x.evidence.length}개 보기</summary>
                    <ul className="mt-2 space-y-2">
                      {x.evidence.map((c) => <li key={c.index} className="rounded-lg bg-surface-2 px-3 py-2"><b>[근거 {c.index}]</b> <span className="whitespace-pre-wrap text-muted">{c.text.slice(0, 500)}{c.text.length > 500 ? "…" : ""}</span></li>)}
                    </ul>
                  </details>
                </div>
              </div>
            ))}
            {qaJob.running && <div className="rounded-xl border border-border p-4">{qaJob.text ? <Md>{qaJob.text}</Md> : <span className="typing"><span /><span /><span /></span>}</div>}
            <ErrorNotice error={qaJob.error} onDismiss={() => qaJob.setError(null)} />
          </div>
          <form className="mt-4 flex gap-2" onSubmit={(e) => { e.preventDefault(); void ask(); }}>
            <input className="input flex-1" placeholder={docs.length ? "예: 이 자료에서 평가 방법은 어떻게 설명하고 있나요?" : "먼저 파일을 불러와 주세요"} value={question} disabled={!ready || docs.length === 0} onChange={(e) => setQuestion(e.target.value)} aria-label="자료에 대한 질문" />
            {qaJob.running ? <button type="button" className="btn-ghost" onClick={qaJob.stop}>중지</button> : <button className="btn-primary" disabled={!ready || docs.length === 0 || !question.trim() || busy}>질문하기</button>}
          </form>
        </section>
      )}
    </div>
  );
}
