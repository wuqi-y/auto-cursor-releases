import ReactDOM from "react-dom/client";
import App from "./App";
import { WebLogService } from "./services/weblogService";

const markTauriEnv = () => {
  try {
    const w = window as any;
    if (w.__TAURI__ || w.__TAURI_INTERNALS__) {
      document.documentElement.classList.add("tauri");
    }
  } catch {
    // ignore
  }
};

const applyInitialTheme = () => {
  try {
    const raw = localStorage.getItem("config-store");
    if (!raw) return;

    const parsed = JSON.parse(raw);
    const themeMode = parsed?.state?.configData?.themeMode?.value;
    if (themeMode !== "dark" && themeMode !== "light") return;

    document.documentElement.classList.toggle("dark", themeMode === "dark");
    document.documentElement.style.colorScheme = themeMode;
  } catch (error) {
    console.error("初始化主题失败:", error);
  }
};

// 初始化全局错误日志收集
WebLogService.init();
markTauriEnv();
applyInitialTheme();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  // <React.StrictMode>
  <App />
  // </React.StrictMode>,
);
