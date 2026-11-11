import { createContext, useCallback, useContext, useEffect, useState } from "react";
import { AppStatus, LocalOnlyReport, call, errMsg, isTauri } from "./lib/api";
import { Icon } from "./lib/ui";
import Onboarding from "./pages/Onboarding";
import Home from "./pages/Home";
import Chat from "./pages/Chat";
import TaskPage from "./pages/TaskPage";
import Students from "./pages/Students";
import DataPage from "./pages/DataPage";
import Models from "./pages/Models";
import Privacy from "./pages/Privacy";

export type Page = "home" | "chat" | "lecture" | "student" | "research" | "admin" | "data" | "models" | "privacy";

interface Ctx {
  status: AppStatus;
  local: LocalOnlyReport | null;
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
  const [page, setPage] = useState<Page>("home");
  const [fatal, setFatal] = useState<string | null>(null);
  const { theme, toggle } = useTheme();

  const refresh = useCallback(async () => {
    try {
      let s = await call<AppStatus>("app_status");
      // 설치된 엔진은 앱을 열 때 자동으로 시작한다.
      if (s.engine.installed && !s.engine.running) {
        try { await call("start_engine"); s = await call<AppStatus>("app_status"); } catch { /* 화면에서 안내 */ }
      }
      setStatus(s);
      setLocal(s.engine.running ? await call<LocalOnlyReport>("verify_local") : null);
    } catch (e) {
      setFatal(errMsg(e));
    }
  }, []);

  useEffect(() => { void refresh(); }, [refresh]);

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

  const ctx: Ctx = { status, local, refresh, go: setPage };

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
            </div>
          )}
        </main>
      </div>
    </AppCtx.Provider>
  );
}

/** 모델이 준비되지 않았을 때 각 기능 화면에 보여 주는 안내 */
export function NeedModel() {
  const { status, go } = useApp();
  if (status.selected_model && status.engine.running) return null;
  return (
    <div className="mb-5 flex items-center justify-between gap-4 rounded-xl border border-warn/25 bg-warn-soft px-4 py-3 text-[14px]">
      <span>{!status.engine.running ? "AI 실행 엔진이 준비되지 않았습니다." : "사용할 AI 모델이 아직 없습니다."} 먼저 AI 모델을 준비해 주세요.</span>
      <button className="btn-primary btn-sm shrink-0" onClick={() => go("models")}>AI 모델 준비하기</button>
    </div>
  );
}
