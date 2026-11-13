import { useCallback, useEffect, useState } from "react";
import { Alias, ChatOutcome, StudentNote, TaskDef, call, errMsg } from "../lib/api";
import { Confirm, Empty, ErrorNotice, Icon, Notice, PageHeader, ResultCard, SecurityTip, useAiJob } from "../lib/ui";
import { NeedModel, useApp } from "../App";

const today = () => new Date().toLocaleDateString("sv-SE");

export default function Students() {
  const { status, refresh } = useApp();
  const [aliases, setAliases] = useState<Alias[]>([]);
  const pseudo = status.pseudonymize_students;
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
    setActive(t); setAliases([]);
    // 기록 내용은 화면이 아니라 앱(Rust)이 저장소에서 직접 읽어 가명 처리한 뒤 AI에 전달한다.
    const res = await job.run((jobId) => call<{ outcome: ChatOutcome; aliases: Alias[] }>("run_student_task", { jobId, taskId: t.id, noteIds: picked.map((n) => n.id), pseudonymize: pseudo }));
    if (res) setAliases(res.aliases);
  };

  return (
    <div className="space-y-6">
      <PageHeader title="학생 지도" sub="상담 기록을 이 컴퓨터에 안전하게 보관하고, 요약과 지도 계획 초안을 만듭니다." />
      <Notice tone="warn" title="학생 개인정보를 다룰 때 꼭 지켜 주세요">
        <ul className="mt-1 list-disc space-y-0.5 pl-5">
          <li>학생 개인정보의 수집·이용·보관은 <b>소속 대학의 개인정보 보호 규정</b>을 따라야 합니다.</li>
          <li>상담 목적에 <b>꼭 필요한 내용만</b> 기록하고, 주민등록번호·건강 정보 같은 민감정보는 적지 마세요.</li>
          <li>실명 대신 <b>별칭(예: 학생 A)</b>을 쓰면 더 안전합니다.</li>
          <li>{status.has_passcode ? "앱 잠금이 켜져 있습니다. 자리를 비울 때는 왼쪽 아래 '지금 잠그기'를 눌러 주세요." : <><b>앱 잠금 암호를 꼭 설정해 주세요.</b> 잠금이 없으면 이 컴퓨터를 쓰는 누구나 기록을 볼 수 있습니다. ('개인정보 보호 설정' 메뉴)</>}</li>
          <li>상담 기록이 필요 없어지면 바로 삭제해 주세요. 기록은 암호화되어 이 컴퓨터에만 저장됩니다.</li>
        </ul>
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
          <label className="mb-3 flex cursor-pointer items-start gap-2.5 text-[14px]">
            <input type="checkbox" className="mt-1 h-4 w-4 accent-[var(--purple)]" checked={pseudo}
              onChange={async (e) => { try { await call("set_pseudonymize", { enabled: e.target.checked }); await refresh(); } catch (err) { setError(errMsg(err)); } }} />
            <span><b>AI에 보내기 전에 가명 처리하기 (권장)</b><span className="block text-[13px] text-muted">학생 이름을 ‘학생1, 학생2…’로 바꾸고 전화번호·이메일·학번·주민번호를 가립니다. 결과물을 저장·공유해도 실명이 남지 않습니다.</span></span>
          </label>
          <div className="label">선택한 기록 {selected.size}개로 할 일</div>
          <div className="flex flex-wrap gap-2">
            {tasks.map((t) => <button key={t.id} className="btn-ghost" disabled={!ready || selected.size === 0 || job.running} onClick={() => run(t)} title={t.description}>{t.label}</button>)}
          </div>
        </div>
      </section>

      <ResultCard title={active?.label ?? "결과"} job={job} footer={aliases.length > 0 ? (
        <div className="mt-3 rounded-lg bg-surface-2 px-3 py-2 text-[13px]">
          <b>가명 대응표</b> <span className="text-muted">(이 화면에만 표시되며 AI에는 전달되지 않았습니다)</span>
          <div className="mt-1 flex flex-wrap gap-x-4 gap-y-1">{aliases.map((a) => <span key={a.alias}>{a.alias} = {a.original}</span>)}</div>
        </div>
      ) : undefined} />
      <SecurityTip>상담 기록 화면을 다른 사람에게 보여 주거나 화상회의에서 화면을 공유하지 마세요. ‘화면 캡처 방지’가 켜져 있으면 이 앱 창은 화면 공유·캡처에서 사라져 보이거나(투명하게 빠짐) 검게 나옵니다.</SecurityTip>

      <Confirm open={!!toDelete} danger title="상담 기록을 삭제할까요?" confirmLabel="삭제" body="이 기록이 이 컴퓨터에서 완전히 삭제됩니다. 되돌릴 수 없습니다."
        onCancel={() => setToDelete(null)}
        onConfirm={async () => { if (toDelete) { try { await call("delete_student_note", { id: toDelete.id }); setSelected((s) => { const n = new Set(s); n.delete(toDelete.id); return n; }); load(); } catch (e) { setError(errMsg(e)); } } setToDelete(null); }} />
    </div>
  );
}
