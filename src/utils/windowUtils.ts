import { invoke } from "@tauri-apps/api/core";

/**
 * 窗口闪烁提示用户
 * 首先尝试调用Tauri的窗口闪烁功能，如果不可用则使用浏览器标题闪烁
 */
export const flashWindow = async () => {
  try {
    // 尝试调用Tauri的窗口闪烁功能
    await invoke("flash_window");
  } catch (error) {
    console.log("Flash window not available:", error);
    // 如果Tauri闪烁不可用，使用浏览器标题闪烁
    const originalTitle = document.title;
    let flashCount = 0;
    const flashInterval = setInterval(() => {
      document.title =
        flashCount % 2 === 0 ? "🔔 新消息 - " + originalTitle : originalTitle;
      flashCount++;
      if (flashCount >= 6) {
        // 闪烁3次
        clearInterval(flashInterval);
        document.title = originalTitle;
      }
    }, 500);
  }
};

/**
 * 显示自动登录窗口
 * @returns Promise<void>
 */
export const showAutoLoginWindow = async () => {
  try {
    await invoke("show_auto_login_window");
  } catch (error) {
    console.error("Failed to show auto login window:", error);
    throw new Error("显示窗口失败，可能窗口已关闭");
  }
};
