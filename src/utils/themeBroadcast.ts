import { emit } from "@tauri-apps/api/event";
import type { ThemeMode } from "../stores/configStore";

/** 广播主题变更到应用内所有 Webview（子窗口与主窗口各自监听并更新 zustand） */
export async function broadcastThemeMode(themeMode: ThemeMode): Promise<void> {
  try {
    await emit("app-theme-mode", { themeMode });
  } catch {
    // 非 Tauri 环境忽略
  }
}
