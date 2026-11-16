// '지난 기록' — 분석 결과를 암호화해 이 앱에만 저장하고, 다시 열어 보고 내보낼 수 있게 한다.
import { useCallback, useEffect, useState } from "react";
import { Alias, ConversationMeta, call, errMsg } from "./api";
import { Confirm, ErrorNotice, ExportButtons, Icon, Md } from "./ui";

export type RecordKind = "lecture" | "research" | "admin" | "student" | "data";

/** 결과를 기록으로 저장한다. 기록 저장이 꺼져 있으면 저장하지 않는다(null). 실패해도 화면 작업은 계속된다. */
export async function saveRecord(kind: RecordKind, title: string, request: string, result: string, aliases?: Alias[]): Promise<string | null> {
  try {
    return await call<string | null>("save_record", { kind, title, request, result, aliases: aliases ?? null });
  } catch {
    return null;
  }
}

export async function appendRecord(id: string, request: string, result: string) {
  try { await call("append_record", { id, request, result }); } catch { /* 무시 */ }
}

interface Msg { role: string; content: string; created_at: string }

/** 기록 저장이 끝나면 목록을 새로 고치도록 알린다 */
const listeners = new Set<() => void>();
export function notifyRecordsChanged() { listeners.forEach((f) => f()); }

export function RecordsPanel({ kind, title = "지난 기록" }: { kind: RecordKind; title?: string }) {
  const [list, setList] = useState<ConversationMeta[]>([]);
  const [open, setOpen] = useState<ConversationMeta | null>(null);
  const [msgs, setMsgs] = useState<Msg[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [toDelete, setToDelete] = useState<ConversationMeta | null>(null);
  const [expanded, setExpanded] = useState(false);

  const load = useCallback(() => { call<ConversationMeta[]>("list_records", { kind }).then(setList).catch((e) => setError(errMsg(e))); }, [kind]);
  useEffect(() => { load(); listeners.add(load); return () => { listeners.delete(load); }; }, [load]);

  const show = async (r: ConversationMeta) => {
    setError(null);
    try { setMsgs(await call<Msg[]>("get_conversation", { id: r.id })); setOpen(r); } catch (e) { setError(errMsg(e)); }
  };

  const body = msgs.filter((m) => m.role === "user" || m.role === "assistant");
  const aliases: Alias[] = (() => { try { const a = msgs.find((m) => m.role === "aliases"); return a ? JSON.parse(a.content) : []; } catch { return []; } })();
  // 내보내기용: 질문·답이 여러 개면 함께, 결과 하나면 결과만 (가명 대응표는 넣지 않음)
  const exportText = body.length <= 2 ? (body.find((m) => m.role === "assistant")?.content ?? "") : body.map((m) => (m.role === "user" ? `### 질문\n${m.content}` : m.content)).join("\n\n");
  const shown = expanded ? list : list.slice(0, 6);

  return (
    <section className="card">
      <div className="mb-1 flex items-center justify-between gap-3">
        <h2 className="h2">{title} {list.length > 0 && <span className="text-[14px] font-normal text-muted">({list.length})</span>}</h2>
      </div>
      <p className="sub mb-3 text-[13px]">결과는 암호화되어 이 앱에만 저장되며, 앱 잠금 암호가 있어야 열 수 있습니다. 필요 없어진 기록은 지워 주세요.</p>
      <ErrorNotice error={error} onDismiss={() => setError(null)} />
      {list.length === 0 ? (
        <p className="text-[14px] text-muted">아직 저장된 기록이 없습니다. 작업을 실행하면 결과가 여기에 자동으로 저장됩니다.</p>
      ) : (
        <ul className="divide-y divide-border">
          {shown.map((r) => (
            <li key={r.id} className={`group flex items-center gap-3 py-2 ${open?.id === r.id ? "text-purple" : ""}`}>
              <Icon name="file" className="h-4 w-4 shrink-0 text-gold" />
              <button className="min-w-0 flex-1 truncate text-left text-[14px] hover:underline" onClick={() => show(r)} title={r.title}>{r.title}</button>
              <span className="shrink-0 text-[12.5px] text-muted tabular-nums">{r.updated_at.slice(0, 16)}</span>
              <button className="p-1 text-muted hover:text-danger" aria-label="기록 삭제" onClick={() => setToDelete(r)}><Icon name="trash" className="h-4 w-4" /></button>
            </li>
          ))}
        </ul>
      )}
      {list.length > 6 && <button className="mt-2 text-[13px] text-muted underline" onClick={() => setExpanded((v) => !v)}>{expanded ? "접기" : `전체 ${list.length}개 보기`}</button>}

      {open && (
        <div className="mt-4 rounded-xl border border-border p-4">
          <div className="mb-3 flex items-start justify-between gap-3">
            <div><div className="font-bold">{open.title}</div><div className="text-[12.5px] text-muted">{open.updated_at}</div></div>
            <button className="p-1 text-muted hover:text-text" aria-label="닫기" onClick={() => setOpen(null)}><Icon name="x" /></button>
          </div>
          <div className="max-h-[520px] space-y-3 overflow-y-auto">
            {body.map((m, i) => m.role === "user"
              ? <div key={i} className="rounded-lg bg-surface-2 px-3 py-2 text-[13.5px]"><b>요청</b> <span className="whitespace-pre-wrap text-muted">{m.content}</span></div>
              : <div key={i}><Md>{m.content}</Md></div>)}
          </div>
          {aliases.length > 0 && (
            <div className="mt-3 rounded-lg bg-surface-2 px-3 py-2 text-[13px]">
              <b>가명 대응표</b> <span className="text-muted">(내보낸 파일에는 들어가지 않습니다)</span>
              <div className="mt-1 flex flex-wrap gap-x-4 gap-y-1">{aliases.map((a) => <span key={a.alias}>{a.alias} = {a.original}</span>)}</div>
            </div>
          )}
          <div className="mt-4 border-t border-border pt-3"><ExportButtons title={open.title} markdown={exportText} /></div>
        </div>
      )}

      <Confirm open={!!toDelete} danger title="기록을 삭제할까요?" confirmLabel="삭제" body={<>“{toDelete?.title}” 기록이 이 컴퓨터에서 완전히 삭제됩니다. 되돌릴 수 없습니다.</>}
        onCancel={() => setToDelete(null)}
        onConfirm={async () => { if (toDelete) { try { await call("delete_conversation", { id: toDelete.id }); if (open?.id === toDelete.id) setOpen(null); load(); } catch (e) { setError(errMsg(e)); } } setToDelete(null); }} />
    </section>
  );
}
