import { useCallback, useEffect, useRef, useState } from "react";
import { ChatMessage, ChatOutcome, ConversationMeta, InstalledModel, call, errMsg } from "../lib/api";
import { Confirm, CopyButton, ErrorNotice, Icon, Md, SecurityTip, useAiJob } from "../lib/ui";
import { LocalBadge, NeedModel, useApp } from "../App";

const STARTERS = ["다음 주 수업 도입부에 쓸 질문 세 가지를 제안해 주세요.", "학생에게 보낼 과제 안내문 초안을 정중한 문체로 써 주세요.", "연구계획서의 '연구 필요성' 문단을 쓰는 요령을 알려 주세요."];

export default function Chat() {
  const { status, local, refresh, go } = useApp();
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [convId, setConvId] = useState<string | null>(null);
  const [convs, setConvs] = useState<ConversationMeta[]>([]);
  const [models, setModels] = useState<InstalledModel[]>([]);
  const [input, setInput] = useState("");
  const [toDelete, setToDelete] = useState<ConversationMeta | null>(null);
  const [error, setError] = useState<string | null>(null);
  const job = useAiJob();
  const endRef = useRef<HTMLDivElement>(null);
  const ready = !!status.selected_model && status.engine.running;

  const loadConvs = useCallback(() => { call<ConversationMeta[]>("list_conversations").then(setConvs).catch(() => {}); }, []);
  useEffect(() => { loadConvs(); call<InstalledModel[]>("list_models").then((m) => setModels(m.filter((x) => !x.is_remote))).catch(() => {}); }, [loadConvs]);
  useEffect(() => { endRef.current?.scrollIntoView({ block: "end" }); }, [messages, job.text]);

  const send = async (text: string) => {
    const content = text.trim();
    if (!content || job.running || !ready) return;
    const next: ChatMessage[] = [...messages, { role: "user", content }];
    setMessages(next); setInput("");
    const res = await job.run((jobId) => call<{ outcome: ChatOutcome; conversation_id: string | null }>("chat_send", { jobId, conversationId: convId, messages: next }));
    if (res) {
      if (res.outcome.content) setMessages([...next, { role: "assistant", content: res.outcome.content + (res.outcome.cancelled ? "\n\n*(중지됨)*" : "") }]);
      if (res.conversation_id) { setConvId(res.conversation_id); loadConvs(); }
    }
    job.setText("");
  };

  const openConv = async (c: ConversationMeta) => {
    if (job.running) return;
    try {
      const msgs = await call<{ role: string; content: string }[]>("get_conversation", { id: c.id });
      setMessages(msgs.map((m) => ({ role: m.role === "assistant" ? "assistant" : "user", content: m.content })));
      setConvId(c.id);
    } catch (e) { setError(errMsg(e)); }
  };
  const newChat = () => { if (!job.running) { setMessages([]); setConvId(null); job.reset(); } };
  const changeModel = async (name: string) => {
    try { await call("select_model", { name }); await refresh(); } catch (e) { setError(errMsg(e)); }
  };

  return (
    <div className="flex h-full">
      <aside className="flex w-[230px] shrink-0 flex-col border-r border-border bg-surface/60 p-3">
        <button className="btn-primary mb-3 w-full" onClick={newChat}><Icon name="plus" className="h-4 w-4" /> 새 대화</button>
        <div className="label px-1">저장된 대화</div>
        <div className="flex-1 space-y-0.5 overflow-y-auto">
          {!status.save_history && <p className="px-1 text-[12.5px] text-muted">대화 기록 저장이 꺼져 있습니다. 대화는 창을 닫으면 사라집니다. <button className="underline" onClick={() => go("privacy")}>설정 바꾸기</button></p>}
          {status.save_history && convs.length === 0 && <p className="px-1 text-[12.5px] text-muted">아직 저장된 대화가 없습니다.</p>}
          {convs.map((c) => (
            <div key={c.id} className={`group flex items-center rounded-lg ${convId === c.id ? "bg-purple-soft" : "hover:bg-surface-2"}`}>
              <button className="min-w-0 flex-1 truncate px-2.5 py-2 text-left text-[13.5px]" onClick={() => openConv(c)} title={c.title}>{c.title}</button>
              <button className="p-1.5 text-muted opacity-0 hover:text-danger group-hover:opacity-100 focus:opacity-100" aria-label="대화 삭제" onClick={() => setToDelete(c)}><Icon name="trash" className="h-4 w-4" /></button>
            </div>
          ))}
        </div>
      </aside>

      <section className="flex min-w-0 flex-1 flex-col">
        <header className="flex items-center justify-between gap-3 border-b border-border px-6 py-3">
          <div className="flex items-center gap-2 text-[14px]">
            <span className="text-muted">AI 모델</span>
            <select className="input w-auto py-1.5" value={status.selected_model ?? ""} onChange={(e) => changeModel(e.target.value)} disabled={job.running || models.length === 0} aria-label="AI 모델 선택">
              {!status.selected_model && <option value="">선택 안 됨</option>}
              {models.map((m) => <option key={m.name} value={m.name}>{m.name}</option>)}
            </select>
          </div>
          <LocalBadge local={local} onClick={() => go("privacy")} />
        </header>

        <div className="flex-1 overflow-y-auto px-6 py-6">
          <div className="mx-auto max-w-[780px] space-y-5">
            <NeedModel />
            <ErrorNotice error={error} onDismiss={() => setError(null)} />
            {messages.length === 0 && !job.running && (
              <div className="pt-10 text-center">
                <h1 className="h1 mb-2">무엇을 도와드릴까요?</h1>
                <p className="sub mb-4">이 대화는 교수님의 컴퓨터 안에서만 처리됩니다.</p>
                <div className="mx-auto mb-6 max-w-[560px] text-left"><SecurityTip>학생 이름·학번·연락처는 꼭 필요할 때만 입력해 주세요. {status.save_history ? (status.has_passcode ? "대화는 암호화되어 저장되고, 앱 잠금으로 보호됩니다." : "대화 기록 저장이 켜져 있지만 앱 잠금이 없습니다. ‘개인정보 보호 설정’에서 잠금 암호를 설정해 주세요.") : "대화 기록 저장이 꺼져 있어 창을 닫으면 대화가 사라집니다."}</SecurityTip></div>
                <div className="mx-auto grid max-w-[560px] gap-2">
                  {STARTERS.map((s) => <button key={s} className="card py-3 text-left text-[14px] hover:border-purple/50" disabled={!ready} onClick={() => send(s)}>{s}</button>)}
                </div>
              </div>
            )}
            {messages.map((m, i) => (
              <div key={i} className={m.role === "user" ? "flex justify-end" : ""}>
                {m.role === "user" ? (
                  <div className="max-w-[85%] whitespace-pre-wrap rounded-2xl rounded-br-md bg-navy px-4 py-2.5 text-on-primary">{m.content}</div>
                ) : (
                  <div className="card">
                    <Md>{m.content}</Md>
                    <div className="mt-3"><CopyButton text={m.content} /></div>
                  </div>
                )}
              </div>
            ))}
            {job.running && (
              <div className="card" aria-live="polite">
                {job.text ? <Md>{job.text}</Md> : <div className="flex items-center gap-3 text-muted"><span className="typing"><span /><span /><span /></span>AI가 생각하는 중</div>}
              </div>
            )}
            <ErrorNotice error={job.error} onDismiss={() => job.setError(null)} />
            <div ref={endRef} />
          </div>
        </div>

        <footer className="border-t border-border px-6 py-4">
          <form className="mx-auto flex max-w-[780px] items-end gap-2" onSubmit={(e) => { e.preventDefault(); void send(input); }}>
            <textarea
              className="input max-h-40 min-h-[48px] flex-1 resize-none"
              rows={Math.min(6, Math.max(1, input.split("\n").length))}
              placeholder={ready ? "질문이나 요청을 입력하세요 (Enter로 보내기, Shift+Enter로 줄바꿈)" : "AI 모델을 먼저 준비해 주세요"}
              value={input}
              disabled={!ready}
              onChange={(e) => setInput(e.target.value)}
              onKeyDown={(e) => { if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) { e.preventDefault(); void send(input); } }}
              aria-label="메시지 입력"
            />
            {job.running ? (
              <button type="button" className="btn-ghost h-[48px]" onClick={job.stop}><Icon name="stop" className="h-4 w-4" /> 응답 중지</button>
            ) : (
              <button type="submit" className="btn-primary h-[48px]" disabled={!ready || !input.trim()}><Icon name="send" className="h-4 w-4" /> 보내기</button>
            )}
          </form>
        </footer>
      </section>

      <Confirm
        open={!!toDelete} danger title="대화를 삭제할까요?" confirmLabel="삭제"
        body={<>“{toDelete?.title}” 대화가 이 컴퓨터에서 완전히 삭제됩니다. 되돌릴 수 없습니다.</>}
        onCancel={() => setToDelete(null)}
        onConfirm={async () => {
          if (!toDelete) return;
          try { await call("delete_conversation", { id: toDelete.id }); if (convId === toDelete.id) newChat(); loadConvs(); } catch (e) { setError(errMsg(e)); }
          setToDelete(null);
        }}
      />
    </div>
  );
}
