import { useEffect, useState } from "react";
import { PrivacyInfo, call, fmtBytes, fmtGB } from "../lib/api";
import { Icon } from "../lib/ui";
import { LocalBadge, NeedModel, Page, useApp } from "../App";

const FEATURES: { page: Page; icon: string; title: string; desc: string }[] = [
  { page: "chat", icon: "chat", title: "AI와 대화하기", desc: "궁금한 점을 묻고 글 초안을 함께 다듬습니다." },
  { page: "lecture", icon: "book", title: "강의자료 분석", desc: "요약, 핵심 개념, 토론 질문, 퀴즈 초안을 만듭니다." },
  { page: "student", icon: "users", title: "학생 지도", desc: "상담 기록을 정리하고 지도 계획 초안을 만듭니다." },
  { page: "research", icon: "flask", title: "연구자료 분석", desc: "논문을 요약·비교하고 아이디어를 정리합니다." },
  { page: "admin", icon: "briefcase", title: "행정업무 도우미", desc: "회의록, 보고서, 공문 초안을 작성합니다." },
  { page: "data", icon: "table", title: "Excel 데이터 분석", desc: "통계를 정확히 계산하고 보고서로 정리합니다." },
];

export default function Home() {
  const { status, local, go } = useApp();
  const [info, setInfo] = useState<PrivacyInfo | null>(null);
  useEffect(() => { call<PrivacyInfo>("privacy_info").then(setInfo).catch(() => {}); }, []);

  return (
    <div>
      <section className="mb-5 overflow-hidden rounded-3xl bg-navy px-9 py-9 text-on-primary">
        <div className="mb-4 inline-flex items-center gap-2 rounded-full border border-current/25 bg-white/10 px-3.5 py-1.5 text-[13px] font-bold tracking-wide">
          <Icon name="lock" className="h-4 w-4" /> LOCAL AI · 내 컴퓨터 안에서만 실행되는 AI
        </div>
        <h1 className="mb-3 text-[30px] font-bold leading-[1.3] tracking-[-0.015em]">교수님의 연구와 수업을 돕는 나만의 AI</h1>
        <p className="mb-5 max-w-[58ch] text-[15.5px] opacity-85">중요한 자료는 내 컴퓨터에서 안전하게 관리하고, AI의 도움으로 업무를 더 효율적으로 처리하세요.</p>
        <ul className="flex flex-wrap gap-x-6 gap-y-2 text-[14px] font-semibold">
          {["인터넷 없이도 작동합니다", "자료가 외부 서버로 가지 않습니다", "클라우드 AI를 사용하지 않습니다"].map((t) => (
            <li key={t} className="flex items-center gap-1.5"><Icon name="check" className="h-4 w-4" />{t}</li>
          ))}
        </ul>
        <p className="mt-6 text-[12.5px] font-semibold tracking-wide opacity-60">AI로 성장하고, 그 성장을 나눈다</p>
      </section>

      <section className="card mb-8" aria-label="로컬 실행 확인">
        <div className="mb-3 flex flex-wrap items-center justify-between gap-3">
          <div>
            <h2 className="h2">지금 이 AI는 교수님 컴퓨터 안에서만 일하고 있나요?</h2>
            <p className="sub text-[13.5px]">설정값이 아니라, 실행 중인 AI를 직접 검사한 결과입니다.</p>
          </div>
          <LocalBadge local={local} onClick={() => go("privacy")} />
        </div>
        <ul className="grid gap-3 sm:grid-cols-3">
          {[
            { ok: !!local?.cloud_disabled, title: "클라우드 기능 꺼짐", desc: "인터넷 너머의 AI 서버를 쓰지 않습니다" },
            { ok: !!local?.loopback_only, title: "외부 접속 차단", desc: "다른 컴퓨터에서 이 AI에 접속할 수 없습니다" },
            { ok: !!local?.model_local, title: "AI가 이 컴퓨터에 있음", desc: "선택한 AI 모델이 이 컴퓨터에 저장되어 있습니다" },
          ].map((c) => (
            <li key={c.title} className={`rounded-xl border px-4 py-3 ${c.ok ? "border-ok/30 bg-ok-soft" : "border-border bg-surface-2"}`}>
              <div className="flex items-center gap-2 font-bold">
                <Icon name={c.ok ? "check" : "info"} className={`h-[18px] w-[18px] ${c.ok ? "text-ok" : "text-muted"}`} />
                {c.title}
              </div>
              <div className="mt-0.5 text-[13px] text-muted">{c.desc}</div>
              <div className={`mt-1 text-[12.5px] font-semibold ${c.ok ? "text-ok" : "text-muted"}`}>{c.ok ? "확인됨" : "아직 확인 전"}</div>
            </li>
          ))}
        </ul>
      </section>

      <NeedModel />

      <div className="mb-8 grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
        {FEATURES.map((f) => (
          <button key={f.page} onClick={() => go(f.page)} className="card group text-left transition-colors hover:border-purple/50">
            <div className="mb-3 inline-flex rounded-xl bg-gold-soft p-2.5 text-gold"><Icon name={f.icon} /></div>
            <div className="mb-1 text-[16px] font-bold group-hover:text-purple">{f.title}</div>
            <div className="sub text-[13.5px]">{f.desc}</div>
          </button>
        ))}
      </div>

      <section className="card grid gap-5 sm:grid-cols-3" aria-label="현재 상태">
        <button className="text-left" onClick={() => go("models")}>
          <div className="label">현재 AI 모델</div>
          <div className="font-bold">{status.selected_model ?? "아직 없음"}</div>
          <div className="text-[12.5px] text-muted">눌러서 바꾸기</div>
        </button>
        <div>
          <div className="label">실행 상태</div>
          <LocalBadge local={local} onClick={() => go("privacy")} />
          <div className="mt-1 text-[12.5px] text-muted">{status.engine.running ? "AI 엔진 실행 중" : status.engine.installed ? "AI 엔진 중지됨" : "AI 엔진 미설치"}</div>
        </div>
        <div>
          <div className="label">저장 공간</div>
          <div className="font-bold tabular-nums">{info && info.disk_free > 0 ? `${fmtGB(info.disk_free, 0)} 남음` : "확인 중"}</div>
          <div className="text-[12.5px] text-muted tabular-nums">AI 모델이 사용 중: {info ? fmtBytes(info.models_bytes) : "-"}</div>
        </div>
      </section>
    </div>
  );
}
