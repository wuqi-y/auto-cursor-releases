import { useEffect, useState } from "react";
import { BrowserRouter as Router, Routes, Route } from "react-router-dom";
import { Layout } from "./components/Layout";
import { HomePage } from "./pages/HomePage";
import { MachineIdPage } from "./pages/MachineIdPage";
import { AuthCheckPage } from "./pages/AuthCheckPage";
import { TokenManagePage } from "./pages/TokenManagePage";
import { AutoRegisterPage } from "./pages/AutoRegisterPage";
import { CursorBackupPage } from "./pages/CursorBackupPage";
import { UsageProvider } from "./context/UsageContext";
import { UpdateModal } from "./components/UpdateModal";
import { ErrorBoundary } from "./components/ErrorBoundary";
import { checkForUpdates } from "./services/updateService";
import { UpdateInfo } from "./types/update";
import { useConfigStore } from "./stores/configStore";
import "./App.css";

function App() {
  const [updateInfo, setUpdateInfo] = useState<UpdateInfo | null>(null);
  const [showUpdateModal, setShowUpdateModal] = useState(false);
  const themeMode = useConfigStore((state) => state.getThemeMode() ?? "light");

  useEffect(() => {
    const root = document.documentElement;
    root.classList.toggle("dark", themeMode === "dark");
    root.style.colorScheme = themeMode;
  }, [themeMode]);

  useEffect(() => {
    // 应用启动时检查更新
    const checkUpdates = async () => {
      try {
        console.log("🔍 检查应用更新...");
        const update = await checkForUpdates();
        console.log("🔍 检查应用更新:", update);
        if (update.hasUpdate) {
          console.log("🔄 发现新版本:", update.version);
          setUpdateInfo(update);
          setShowUpdateModal(true);
        } else {
          console.log("✅ 应用已是最新版本");
        }
      } catch (error) {
        console.error("❌ 检查更新失败:", error);
        // 静默失败，不影响应用正常使用
      }
    };

    // 禁用右键菜单
    const handleContextMenu = (e: MouseEvent) => {
      e.preventDefault();
      return false;
    };

    // 禁用开发者工具相关快捷键
    const handleKeyDown = (e: KeyboardEvent) => {
      // 禁用F12
      if (e.key === "F12") {
        e.preventDefault();
        return false;
      }
      // 禁用Ctrl+Shift+I (开发者工具)
      if (e.ctrlKey && e.shiftKey && e.key === "I") {
        e.preventDefault();
        return false;
      }
      // 禁用Ctrl+Shift+J (控制台)
      if (e.ctrlKey && e.shiftKey && e.key === "J") {
        e.preventDefault();
        return false;
      }
      // 禁用Ctrl+U (查看源代码)
      if (e.ctrlKey && e.key === "u") {
        e.preventDefault();
        return false;
      }
      // 禁用Ctrl+Shift+C (元素选择器)
      if (e.ctrlKey && e.shiftKey && e.key === "C") {
        e.preventDefault();
        return false;
      }
    };

    // 添加事件监听器
    document.addEventListener("contextmenu", handleContextMenu);
    document.addEventListener("keydown", handleKeyDown);

    // 延迟3秒后检查更新，避免影响应用启动速度
    const timer = setTimeout(checkUpdates, 3000);

    return () => {
      clearTimeout(timer);
      document.removeEventListener("contextmenu", handleContextMenu);
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, []);

  const handleCloseUpdateModal = () => {
    // 只有非强制更新才能关闭弹窗
    if (updateInfo && !updateInfo.isForceUpdate) {
      setShowUpdateModal(false);
    }
  };

  return (
    <ErrorBoundary>
      <UsageProvider>
        <Router>
          <Layout>
            <Routes>
              <Route path="/" element={<HomePage />} />
              <Route path="/machine-id" element={<MachineIdPage />} />
              <Route path="/auth-check" element={<AuthCheckPage />} />
              <Route path="/token-manage" element={<TokenManagePage />} />
              <Route path="/auto-register" element={<AutoRegisterPage />} />
              <Route path="/cursor-backup" element={<CursorBackupPage />} />
            </Routes>
          </Layout>
        </Router>

        {/* 更新弹窗 */}
        {showUpdateModal && updateInfo && (
          <UpdateModal
            updateInfo={updateInfo}
            onClose={handleCloseUpdateModal}
          />
        )}
      </UsageProvider>
    </ErrorBoundary>
  );
}

export default App;
