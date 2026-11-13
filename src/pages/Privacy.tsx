import { useCallback, useEffect, useState } from "react";
import { PrivacyInfo, call, errMsg, fmtBytes } from "../lib/api";
import { Confirm, ErrorNotice, Icon, Notice, PageHeader } from "../lib/ui";
import { useApp } from "../App";
import SecurityPanel from "./SecurityPanel";

function Toggle({ checked, onChange, label, desc }: { checked: boolean; onChange: (v: boolean) => void; label: string; desc: string }) {
  return (
    <label className="flex cursor-pointer items-start justify-between gap-6 py-3">
      <span><span className="block font-semibold">{label}</span><span className="block text-[13.5px] text-muted">{desc}</span></span>
      <span className="relative mt-1 inline-flex shrink-0">
        <input type="checkbox" className="peer sr-only" checked={checked} onChange={(e) => onChange(e.target.checked)} />
        <span className="h-6 w-11 rounded-full bg-border transition-colors peer-checked:bg-purple peer-focus-visible:ring-2 peer-focus-visible:ring-purple/40" />
        <span className="absolute left-0.5 top-0.5 h-5 w-5 rounded-full bg-surface shadow transition-transform peer-checked:translate-x-5" />
      </span>
    </label>
  );
}

export default function Privacy() {
  const { local, refresh, status } = useApp();
  const [info, setInfo] = useState<PrivacyInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [confirm, setConfirm] = useState<"conversations" | "all" | null>(null);
  const [msg, setMsg] = useState<string | null>(null);

  const load = useCallback(() => { call<PrivacyInfo>("privacy_info").then(setInfo).catch((e) => setError(errMsg(e))); }, []);
  useEffect(() => { load(); void refresh(); }, [load, refresh]);

  const act = async (fn: () => Promise<unknown>, done?: string) => {
    setError(null); setMsg(null);
    try { await fn(); await refresh(); load(); if (done) setMsg(done); } catch (e) { setError(errMsg(e)); }
  };
  const syncRisk = info && /(onedrive|dropbox|icloud|google ?drive|mobile documents|naver ?mybox)/i.test(info.data_dir);

  return (
    <div className="space-y-6">
      <PageHeader title="개인정보 보호 설정" sub="교수님의 자료가 어디에 저장되고 어떻게 보호되는지 확인하고 관리합니다." />
      <SecurityPanel />
      <ErrorNotice error={error} onDismiss={() => setError(null)} />
      {msg && <Notice tone="ok">{msg}</Notice>}

      <section className="card">
        <div className="mb-3 flex items-center justify-between gap-3">
          <h2 className="h2">로컬 전용 실행 상태</h2>
          <button className="btn-ghost btn-sm" onClick={() => act(async () => {})}>다시 확인</button>
        </div>
        <div className={`mb-3 flex items-center gap-3 rounded-xl px-4 py-3 ${local?.verified ? "bg-ok-soft" : "bg-warn-soft"}`}>
          <Icon name={local?.verified ? "lock" : "alert"} className={`h-6 w-6 ${local?.verified ? "text-ok" : "text-warn"}`} />
          <div>
            <div className="font-bold">{local?.verified ? "확인됨: AI가 이 컴퓨터 안에서만 실행되고 있습니다" : "아직 ‘로컬 전용’ 상태를 확인하지 못했습니다"}</div>
            <div className="text-[13px] text-muted">설정값이 아니라 실행 중인 AI 엔진을 직접 검사한 결과입니다.</div>
          </div>
        </div>
        <ul className="space-y-1.5 text-[14px]">
          {(local?.details ?? ["AI 엔진이 실행 중이 아닙니다. AI 모델을 준비하면 확인할 수 있습니다."]).map((d, i) => (
            <li key={i} className="flex gap-2"><Icon name={d.startsWith("확인됨") ? "check" : "info"} className={`mt-1 h-4 w-4 shrink-0 ${d.startsWith("확인됨") ? "text-ok" : "text-warn"}`} />{d}</li>
          ))}
        </ul>
      </section>

      <section className="card">
        <h2 className="h2 mb-1">설정</h2>
        <div className="divide-y divide-border">
          <Toggle checked={!!info?.save_history} onChange={(v) => act(() => call("set_save_history", { enabled: v }))} label="대화 기록 저장" desc="켜면 대화가 암호화되어 이 컴퓨터에 저장됩니다. 끄면 창을 닫을 때 대화가 사라집니다. (기본: 꺼짐)" />
          <Toggle checked={!!info?.download_consent} onChange={(v) => act(() => call("set_download_consent", { consent: v }))} label="AI 내려받기를 위한 인터넷 사용 허용" desc="AI 실행 엔진과 AI 모델 파일을 내려받을 때만 인터넷을 사용합니다. 끄면 앱은 인터넷을 전혀 사용하지 않습니다." />
        </div>
      </section>

      <section className="card">
        <h2 className="h2 mb-3">이 앱이 하지 않는 일</h2>
        <ul className="grid gap-2 text-[14px] sm:grid-cols-2">
          {["외부 AI 서비스에 자료를 보내지 않습니다", "사용 기록이나 통계를 수집하지 않습니다", "오류 내용을 외부로 보내지 않습니다", "자료를 클라우드에 백업하지 않습니다", "AI 엔진의 클라우드 기능을 꺼 둡니다", "문서 원문을 로그 파일에 남기지 않습니다"].map((t) => (
            <li key={t} className="flex gap-2"><Icon name="shield" className="mt-0.5 h-[18px] w-[18px] shrink-0 text-ok" />{t}</li>
          ))}
        </ul>
      </section>

      <section className="card">
        <h2 className="h2 mb-3">저장 위치와 보관 중인 자료</h2>
        {info && (
          <>
            <dl className="grid grid-cols-[150px_1fr] gap-x-4 gap-y-2 text-[14px]">
              <dt className="text-muted">앱 자료 폴더</dt><dd className="break-all font-mono text-[13px]">{info.data_dir}</dd>
              <dt className="text-muted">AI 모델 폴더</dt><dd className="break-all font-mono text-[13px]">{info.models_dir} <span className="font-sans text-muted">({fmtBytes(info.models_bytes)})</span></dd>
              <dt className="text-muted">암호화 키 보관</dt><dd>{info.key_store === "os_keychain" ? "운영체제 보안 저장소 (키체인 / 자격 증명 관리자)" : "앱 자료 폴더의 키 파일 (개발용 환경 — 보안 저장소보다 약함)"}{status.has_passcode ? " + 앱 잠금 암호로 한 번 더 보호" : ""}</dd>
              <dt className="text-muted">저장된 대화</dt><dd className="tabular-nums">{info.stats.conversations}개 (메시지 {info.stats.messages}개)</dd>
              <dt className="text-muted">상담 기록</dt><dd className="tabular-nums">{info.stats.student_notes}개</dd>
              <dt className="text-muted">지금 불러온 파일</dt><dd className="tabular-nums">{info.loaded_documents}개 <span className="text-muted">(메모리에만 있으며 화면을 떠나거나 앱을 닫으면 사라집니다)</span></dd>
            </dl>
            <div className="mt-4 space-y-2">
              {syncRisk && <Notice tone="warn" title="동기화 폴더 주의">앱 자료 폴더가 클라우드 동기화 폴더 안에 있는 것으로 보입니다. 암호화된 자료 파일이 클라우드로 복제될 수 있습니다.</Notice>}
              <Notice tone="info" title="백업·동기화 안내">
                컴퓨터 전체 백업(Time Machine, Windows 백업 등)이나 클라우드 동기화를 사용하면 위 폴더의 암호화된 자료가 함께 복제될 수 있습니다. 또한 교수님이 저장한 Word·Excel 결과 파일은 암호화되지 않으므로 저장 위치에 유의해 주세요.
              </Notice>
            </div>
          </>
        )}
      </section>

      <section className="card">
        <h2 className="h2 mb-3">자료 삭제</h2>
        <div className="flex flex-wrap gap-2">
          <button className="btn-danger" onClick={() => setConfirm("conversations")}><Icon name="trash" className="h-4 w-4" /> 저장된 대화 모두 삭제</button>
          <button className="btn-danger" onClick={() => setConfirm("all")}><Icon name="trash" className="h-4 w-4" /> 대화와 상담 기록 모두 삭제</button>
        </div>
        <p className="mt-2 text-[13px] text-muted">삭제한 자료는 저장 파일에서도 지워지며 되돌릴 수 없습니다. AI 모델은 ‘내 AI 모델 관리’에서 지울 수 있습니다.</p>
      </section>

      <section className="card">
        <h2 className="h2 mb-1">인터넷 사용 기록</h2>
        <p className="sub mb-3 text-[13.5px]">앱을 켠 뒤 인터넷 사용을 승인하거나 차단한 내역입니다. 내려받은 항목의 이름만 기록되며 문서·대화 내용은 기록되지 않습니다.</p>
        {info && info.audit_log.length === 0 ? <p className="text-[14px] text-muted">이번 실행에서는 인터넷을 사용하지 않았습니다.</p> : (
          <div className="overflow-x-auto rounded-xl border border-border">
            <table className="w-full text-[13px]">
              <thead><tr className="bg-surface-2 text-left"><th className="px-3 py-2">시각</th><th className="px-3 py-2">내용</th><th className="px-3 py-2">대상</th><th className="px-3 py-2">결과</th></tr></thead>
              <tbody>{info?.audit_log.slice().reverse().map((a, i) => (
                <tr key={i} className="border-t border-border"><td className="whitespace-nowrap px-3 py-1.5 tabular-nums">{a.at}</td><td className="px-3 py-1.5">{a.detail || "-"}</td><td className="px-3 py-1.5 font-mono text-[12px]">{a.host}</td><td className={`px-3 py-1.5 font-semibold ${a.allowed ? "text-ok" : "text-danger"}`}>{a.allowed ? "허용" : "차단"}</td></tr>
              ))}</tbody>
            </table>
          </div>
        )}
      </section>

      <Confirm open={confirm !== null} danger confirmLabel="모두 삭제"
        title={confirm === "all" ? "대화와 상담 기록을 모두 삭제할까요?" : "저장된 대화를 모두 삭제할까요?"}
        body="이 컴퓨터에서 완전히 삭제되며 되돌릴 수 없습니다."
        onCancel={() => setConfirm(null)}
        onConfirm={() => { const c = confirm; setConfirm(null); void act(() => call(c === "all" ? "delete_all_data" : "delete_all_conversations"), "삭제했습니다."); }} />
    </div>
  );
}
