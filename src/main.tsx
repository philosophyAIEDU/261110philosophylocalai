import React from "react";
import ReactDOM from "react-dom/client";
import "./styles.css";
import { isTauri } from "./lib/api";

async function boot() {
  // 브라우저에서 화면만 미리 볼 때(개발 전용) 가짜 데이터를 쓴다. 배포용 빌드에는 포함되지 않는다.
  if (import.meta.env.DEV && !isTauri) {
    const { installPreviewBackend } = await import("./lib/preview");
    installPreviewBackend();
  }
  const { default: App } = await import("./App");
  ReactDOM.createRoot(document.getElementById("root")!).render(
    <React.StrictMode>
      <App />
    </React.StrictMode>,
  );
}
void boot();
