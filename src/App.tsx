import { createContext, useCallback, useContext, useEffect, useState } from "react";
import { AppStatus, InstalledModel, LocalOnlyReport, call, errMsg, isTauri, on } from "./lib/api";
import { Icon } from "./lib/ui";
import Onboarding from "./pages/Onboarding";
import Home from "./pages/Home";
import Chat from "./pages/Chat";
import TaskPage from "./pages/TaskPage";
import Students from "./pages/Students";
import DataPage from "./pages/DataPage";
import Models from "./pages/Models";
import Privacy from "./pages/Privacy";
import LockScreen from "./pages/Lock";
import UpdatePage from "./pages/Update";

export type Page = "home" | "chat" | "lecture" | "student" | "research" | "admin" | "data" | "models" | "privacy" | "update";

interface Ctx {
  status: AppStatus;
  local: LocalOnlyReport | null;
  engineError: string | null;
  refresh: () => Promise<void>;
  go: (p: Page) => void;
}
const AppCtx = createContext<Ctx | null>(null);
export const useApp = () => useContext(AppCtx)!;

const MENU: { id: Page; label: string; icon: string }[] = [
  { id: "home", label: "홈", icon: "home" },
  { id: "chat", label: "AI와 대화하기", icon: "chat" },
  { id: "lecture", label: "강의자료 분석", icon: "book" },
  { id: "student", label: "학생 지도", icon: "users" },
  { id: "research", label: "연구자료 분석", icon: "flask" },
  { id: "admin", label: "행정업무 도우미", icon: "briefcase" },
  { id: "data", label: "Excel 데이터 분석", icon: "table" },
  { id: "models", label: "내 AI 모델 관리", icon: "cpu" },
  { id: "privacy", label: "개인정보 보호 설정", icon: "shield" },
  { id: "update", label: "앱 업데이트", icon: "download" },
];

function useTheme() {
  const [theme, setTheme] = useState<"light" | "dark">(() => {
    try {
      const saved = localStorage.getItem("theme");
      if (saved === "light" || saved === "dark") return saved;
    } catch { /* 저장소 사용 불가 */ }
    return window.matchMedia?.("(prefers-color-scheme: dark)").matches ? "dark" : "light";
  });
  useEffect(() => {
    document.documentElement.dataset.theme = theme;
    try { localStorage.setItem("theme", theme); } catch { /* 무시 */ }
  }, [theme]);
  return { theme, toggle: () => setTheme((t) => (t === "dark" ? "light" : "dark")) };
}

/** '로컬 전용' 표시 — 실행 중인 엔진을 실제로 검사해 확인된 경우에만 초록색으로 표시한다. */
export function LocalBadge({ local, onClick }: { local: LocalOnlyReport | null; onClick?: () => void }) {
  const ok = !!local?.verified;
  return (
    <button onClick={onClick} className={`chip ${ok ? "bg-ok-soft text-ok" : "bg-surface-2 text-muted"}`} title={ok ? "확인됨: 이 컴퓨터 안에서만 실행 중" : "아직 확인되지 않았습니다. 눌러서 자세히 보기"}>
      <Icon name={ok ? "lock" : "info"} className="h-3.5 w-3.5" />
      {ok ? "로컬 전용 확인됨" : "로컬 전용 미확인"}
    </button>
  );
}

export default function App() {
  const [status, setStatus] = useState<AppStatus | null>(null);
  const [local, setLocal] = useState<LocalOnlyReport | null>(null);
  const [engineError, setEngineError] = useState<string | null>(null);
  const [page, setPage] = useState<Page>("home");
  const [fatal, setFatal] = useState<string | null>(null);
  const { theme, toggle } = useTheme();

  const refresh = useCallback(async () => {
    try {
      let s = await call<AppStatus>("app_status");
      setEngineError(null);
      // 설치된 엔진은 앱을 열 때 자동으로 시작한다.
      if (s.engine.installed && !s.engine.running) {
        try { await call("start_engine"); s = await call<AppStatus>("app_status"); }
        catch (e) { setEngineError(errMsg(e)); }
      }
      // 이미 받은 AI가 있는데 선택된 모델이 없으면 자동으로 선택한다 (다시 내려받으라고 묻지 않도록).
      if (s.engine.running && !s.selected_model && !s.locked) {
        try {
          const local = (await call<InstalledModel[]>("list_models")).filter((m) => !m.is_remote);
          if (local.length > 0) { await call("select_model", { name: local[0].name }); s = await call<AppStatus>("app_status"); }
        } catch { /* 무시 */ }
      }
      setStatus(s);
      setLocal(s.engine.running ? await call<LocalOnlyReport>("verify_local") : null);
    } catch (e) {
      setFatal(errMsg(e));
    }
  }, []);

  useEffect(() => { void refresh(); }, [refresh]);

  // 자동 잠금: 앱이 잠겼다는 알림을 받으면 잠금 화면으로 전환
  useEffect(() => {
    const sub = on("app-locked", () => { setStatus((s) => (s ? { ...s, locked: true } : s)); });
    return () => { sub.then((un) => un()); };
  }, []);

  // 키보드·마우스 사용을 앱에 알려 자동 잠금 시간을 늦춘다 (30초에 한 번만)
  useEffect(() => {
    let last = 0;
    const ping = () => { const now = Date.now(); if (now - last > 30_000) { last = now; void call("user_activity").catch(() => {}); } };
    const evs = ["mousemove", "keydown", "mousedown", "wheel", "touchstart"] as const;
    evs.forEach((e) => window.addEventListener(e, ping, { passive: true }));
    return () => evs.forEach((e) => window.removeEventListener(e, ping));
  }, []);

  if (fatal) {
    return (
      <div className="flex h-full items-center justify-center p-10">
        <div className="card max-w-lg">
          <h1 className="h2 mb-2">프로그램을 시작하지 못했습니다</h1>
          <p className="sub">{fatal}</p>
          <button className="btn-primary mt-4" onClick={() => { setFatal(null); void refresh(); }}>다시 시도</button>
        </div>
      </div>
    );
  }
  if (!status) return <div className="flex h-full items-center justify-center text-muted">불러오는 중…</div>;

  if (status.locked) return <LockScreen onUnlocked={() => { setPage("home"); void refresh(); }} />;

  const ctx: Ctx = { status, local, engineError, refresh, go: setPage };

  if (!status.onboarding_done) {
    return (
      <AppCtx.Provider value={ctx}>
        <Onboarding />
      </AppCtx.Provider>
    );
  }

  return (
    <AppCtx.Provider value={ctx}>
      <div className="flex h-full">
        <nav className="flex w-[248px] shrink-0 flex-col border-r border-border bg-surface px-3 py-5" aria-label="주 메뉴">
          <div className="mb-6 flex items-center gap-3 px-3">
            <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-navy text-[20px] font-bold text-on-primary">Φ</div>
            <div className="leading-tight">
              <div className="text-[15px] font-bold">필로소피 로컬 AI</div>
              <div className="text-[11.5px] text-muted">Philosophy Local AI</div>
            </div>
          </div>
          <ul className="flex-1 space-y-0.5">
            {MENU.map((m, i) => (
              <li key={m.id} className={i === 7 ? "mt-3 border-t border-border pt-3" : ""}>
                <button
                  onClick={() => setPage(m.id)}
                  aria-current={page === m.id ? "page" : undefined}
                  className={`flex w-full items-center gap-3 rounded-xl px-3 py-2.5 text-left text-[14.5px] transition-colors ${page === m.id ? "bg-purple-soft font-bold text-purple" : "text-text hover:bg-surface-2"}`}
                >
                  <Icon name={m.icon} className="h-[19px] w-[19px] shrink-0" />
                  {m.label}
                </button>
              </li>
            ))}
          </ul>
          <div className="space-y-2 px-2">
            {status.has_passcode ? (
              <button className="btn-ghost btn-sm w-full" onClick={async () => { try { await call("lock_app"); } catch { /* 무시 */ } }} title="자리를 비울 때 눌러 주세요">
                <Icon name="lock" className="h-4 w-4" /> 지금 잠그기
              </button>
            ) : (
              <button className="chip bg-warn-soft text-warn" onClick={() => setPage("privacy")} title="학생 정보를 다룬다면 잠금 암호를 설정해 주세요">
                <Icon name="alert" className="h-3.5 w-3.5" /> 앱 잠금 꺼짐
              </button>
            )}
            <LocalBadge local={local} onClick={() => setPage("privacy")} />
            <div className="flex items-center justify-between">
              <span className="text-[11.5px] text-muted">버전 {status.app_version}</span>
              <button className="rounded-lg p-1.5 text-muted hover:bg-surface-2" onClick={toggle} aria-label={theme === "dark" ? "밝은 화면으로" : "어두운 화면으로"}>
                <Icon name={theme === "dark" ? "sun" : "moon"} className="h-[18px] w-[18px]" />
              </button>
            </div>
          </div>
        </nav>
        <main className="min-w-0 flex-1 overflow-y-auto">
          {!isTauri && <div className="bg-warn-soft px-6 py-1.5 text-center text-[12.5px] text-warn">화면 미리보기 모드 — 실제 AI가 아닌 예시 데이터입니다</div>}
          {page === "chat" ? (
            <Chat />
          ) : (
            <div className="mx-auto max-w-[1040px] px-9 py-8">
              {page === "home" && <Home />}
              {page === "lecture" && <TaskPage key="lecture" category="lecture" />}
              {page === "student" && <Students />}
              {page === "research" && <TaskPage key="research" category="research" />}
              {page === "admin" && <TaskPage key="admin" category="admin" />}
              {page === "data" && <DataPage />}
              {page === "models" && <Models />}
              {page === "privacy" && <Privacy />}
              {page === "update" && <UpdatePage />}
            </div>
          )}
        </main>
      </div>
    </AppCtx.Provider>
  );
}

/** 모델이 준비되지 않았을 때 각 기능 화면에 보여 주는 안내 */
export function NeedModel() {
  const { status, go, refresh } = useApp();
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  if (status.selected_model && status.engine.running) return null;
  // 엔진은 설치되어 있는데 멈춰 있을 뿐이면, 다시 내려받을 필요 없이 다시 시작만 하면 된다.
  if (status.engine.installed && !status.engine.running) {
    return (
      <div className="mb-5 flex flex-wrap items-center justify-between gap-3 rounded-xl border border-warn/25 bg-warn-soft px-4 py-3 text-[14px]">
        <span>AI 실행 엔진이 멈춰 있습니다. 이미 받은 AI는 그대로 있으니 다시 시작만 하면 됩니다.{err && <span className="block text-danger">{err}</span>}</span>
        <button className="btn-primary btn-sm shrink-0" disabled={busy} onClick={async () => { setBusy(true); setErr(null); try { await call("start_engine"); await refresh(); } catch (e) { setErr(errMsg(e)); } setBusy(false); }}>{busy ? "시작하는 중…" : "AI 엔진 다시 시작"}</button>
      </div>
    );
  }
  return (
    <div className="mb-5 flex items-center justify-between gap-4 rounded-xl border border-warn/25 bg-warn-soft px-4 py-3 text-[14px]">
      <span>{!status.engine.installed ? "AI 실행 엔진이 아직 설치되지 않았습니다." : "사용할 AI 모델이 아직 없습니다."} 먼저 AI 모델을 준비해 주세요.</span>
      <button className="btn-primary btn-sm shrink-0" onClick={() => go("models")}>AI 모델 준비하기</button>
    </div>
  );
}
