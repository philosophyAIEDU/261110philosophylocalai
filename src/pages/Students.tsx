import { useCallback, useEffect, useState } from "react";
import { ChatOutcome, StudentNote, TaskDef, call, errMsg } from "../lib/api";
import { Confirm, Empty, ErrorNotice, Icon, Notice, PageHeader, ResultCard, useAiJob } from "../lib/ui";
import { NeedModel, useApp } from "../App";

const today = () => new Date().toLocaleDateString("sv-SE");

export default function Students() {
  const { status } = useApp();
  const [notes, setNotes] = useState<StudentNote[]>([]);
  const [query, setQuery] = useState("");
  const [student, setStudent] = useState("");
  const [date, setDate] = useState(today());
  const [content, setContent] = useState("");
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [tasks, setTasks] = useState<TaskDef[]>([]);
  const [active, setActive] = useState<TaskDef | null>(null);
  const [toDelete, setToDelete] = useState<StudentNote | null>(null);
  const [error, setError] = useState<string | null>(null);
  const job = useAiJob();
  const ready = !!status.selected_model && status.engine.running;

  const load = useCallback(() => {
    call<StudentNote[]>("list_student_notes", { query: query.trim() || null }).then(setNotes).catch((e) => setError(errMsg(e)));
  }, [query]);
  useEffect(() => { const t = setTimeout(load, 200); return () => clearTimeout(t); }, [load]);
  useEffect(() => { call<TaskDef[]>("list_tasks").then((t) => setTasks(t.filter((x) => x.category === "student"))).catch(() => {}); }, []);

  const add = async () => {
    setError(null);
    try { await call("add_student_note", { student, date, content }); setContent(""); load(); }
    catch (e) { setError(errMsg(e)); }
  };
  const toggle = (id: string) => setSelected((s) => { const n = new Set(s); if (n.has(id)) n.delete(id); else n.add(id); return n; });
  const run = async (t: TaskDef) => {
    const picked = notes.filter((n) => selected.has(n.id));
    if (!picked.length || job.running) return;
    setActive(t);
    const pasted = picked.map((n) => `[${n.date}] ${n.student}\n${n.content}`).join("\n\n---\n\n");
    await job.run((jobId) => call<ChatOutcome>("run_task", { jobId, taskId: t.id, docIds: [], pasted, extra: "" }));
  };

  return (
    <div className="space-y-6">
      <PageHeader title="학생 지도" sub="상담 기록을 이 컴퓨터에 안전하게 보관하고, 요약과 지도 계획 초안을 만듭니다." />
      <Notice tone="warn" title="학생 개인정보를 다룰 때">
        학생 개인정보의 수집·이용·보관은 소속 대학의 개인정보 보호 규정을 따라야 합니다. 꼭 필요한 내용만 기록하시고, 실명 대신 별칭을 쓰는 것도 좋은 방법입니다. 기록은 암호화되어 이 컴퓨터에만 저장됩니다.
      </Notice>
      <NeedModel />
      <ErrorNotice error={error} onDismiss={() => setError(null)} />

      <section className="card">
        <h2 className="h2 mb-3">상담 기록 추가</h2>
        <div className="grid gap-3 sm:grid-cols-[1fr_180px]">
          <div><label className="label" htmlFor="st-name">학생 이름 또는 별칭</label><input id="st-name" className="input" value={student} onChange={(e) => setStudent(e.target.value)} placeholder="예: 학생 A" /></div>
          <div><label className="label" htmlFor="st-date">상담 날짜</label><input id="st-date" type="date" className="input" value={date} onChange={(e) => setDate(e.target.value)} /></div>
        </div>
        <div className="mt-3"><label className="label" htmlFor="st-content">상담 내용</label><textarea id="st-content" className="input min-h-[110px]" value={content} onChange={(e) => setContent(e.target.value)} placeholder="상담에서 나눈 이야기를 적어 주세요" /></div>
        <button className="btn-primary mt-3" disabled={!student.trim() || !content.trim()} onClick={add}><Icon name="plus" className="h-4 w-4" /> 기록 저장</button>
      </section>

      <section className="card">
        <div className="mb-3 flex flex-wrap items-center justify-between gap-3">
          <h2 className="h2">상담 기록 ({notes.length})</h2>
          <div className="relative w-[260px]">
            <Icon name="search" className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted" />
            <input className="input pl-9" placeholder="이름이나 내용으로 찾기" value={query} onChange={(e) => setQuery(e.target.value)} aria-label="상담 기록 검색" />
          </div>
        </div>
        {notes.length === 0 ? (
          <Empty icon="users" title={query ? "찾는 기록이 없습니다" : "아직 상담 기록이 없습니다"}>{query ? "다른 검색어로 찾아보세요." : "위에서 첫 상담 기록을 추가해 보세요."}</Empty>
        ) : (
          <ul className="space-y-2">
            {notes.map((n) => (
              <li key={n.id} className={`flex items-start gap-3 rounded-xl border px-3.5 py-3 ${selected.has(n.id) ? "border-purple bg-purple-soft" : "border-border"}`}>
                <input type="checkbox" className="mt-1.5 h-4 w-4 accent-[var(--purple)]" checked={selected.has(n.id)} onChange={() => toggle(n.id)} aria-label={`${n.student} ${n.date} 기록 선택`} />
                <div className="min-w-0 flex-1">
                  <div className="flex items-baseline gap-2"><span className="font-bold">{n.student}</span><span className="text-[12.5px] text-muted tabular-nums">{n.date}</span></div>
                  <p className="whitespace-pre-wrap text-[14px]">{n.content}</p>
                </div>
                <button className="p-1 text-muted hover:text-danger" aria-label="기록 삭제" onClick={() => setToDelete(n)}><Icon name="trash" className="h-4 w-4" /></button>
              </li>
            ))}
          </ul>
        )}
        <div className="mt-4 border-t border-border pt-4">
          <div className="label">선택한 기록 {selected.size}개로 할 일</div>
          <div className="flex flex-wrap gap-2">
            {tasks.map((t) => <button key={t.id} className="btn-ghost" disabled={!ready || selected.size === 0 || job.running} onClick={() => run(t)} title={t.description}>{t.label}</button>)}
          </div>
        </div>
      </section>

      <ResultCard title={active?.label ?? "결과"} job={job} />

      <Confirm open={!!toDelete} danger title="상담 기록을 삭제할까요?" confirmLabel="삭제" body="이 기록이 이 컴퓨터에서 완전히 삭제됩니다. 되돌릴 수 없습니다."
        onCancel={() => setToDelete(null)}
        onConfirm={async () => { if (toDelete) { try { await call("delete_student_note", { id: toDelete.id }); setSelected((s) => { const n = new Set(s); n.delete(toDelete.id); return n; }); load(); } catch (e) { setError(errMsg(e)); } } setToDelete(null); }} />
    </div>
  );
}
