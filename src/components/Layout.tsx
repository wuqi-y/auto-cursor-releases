import React, { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { broadcastThemeMode } from "../utils/themeBroadcast";
import { Link, Outlet, useLocation, useNavigate } from "react-router-dom";
import logoSvg from "../assets/logo.svg";
import { getCurrentVersion } from "../services/updateService";
import { CursorService } from "../services/cursorService";
import { useConfigStore } from "../stores/configStore";
import { TitleBar } from "./TitleBar";

export const Layout: React.FC = () => {
  const location = useLocation();
  const navigate = useNavigate();
  const [version, setVersion] = useState<string>("");
  const [cursorVersion, setCursorVersion] = useState<string>("");
  const [cursorVersionError, setCursorVersionError] = useState<boolean>(false);
  const [isCollapsed, setIsCollapsed] = useState(true);
  const [isMobile, setIsMobile] = useState(false);
  const [hoveredItem, setHoveredItem] = useState<string | null>(null);
  const [tooltipPosition, setTooltipPosition] = useState({ top: 0, left: 0 });
  const themeMode = useConfigStore((state) => state.getThemeMode() ?? "light");
  const setThemeMode = useConfigStore((state) => state.setThemeMode);

  useEffect(() => {
    getCurrentVersion().then(setVersion);

    CursorService.getCursorVersion()
      .then((version) => {
        setCursorVersion(version);
        setCursorVersionError(false);
      })
      .catch((error) => {
        console.error("获取Cursor版本失败:", error);
        setCursorVersion("未检测到");
        setCursorVersionError(true);
      });

    const checkScreenSize = () => {
      setIsMobile(window.innerWidth < 1024);
    };

    checkScreenSize();
    window.addEventListener("resize", checkScreenSize);
    return () => window.removeEventListener("resize", checkScreenSize);
  }, []);

  const handleOpenLogDirectory = async () => {
    try {
      await CursorService.openLogDirectory();
    } catch (error) {
      console.error("打开日志目录失败:", error);
    }
  };

  const toggleSidebar = () => {
    setIsCollapsed(!isCollapsed);
  };

  const toggleTheme = () => {
    const next = themeMode === "dark" ? "light" : "dark";
    setThemeMode(next);
    void broadcastThemeMode(next);
  };

  const handleMouseEnter = (
    label: string,
    event: React.MouseEvent<Element>,
  ) => {
    if (isCollapsed) {
      const rect = event.currentTarget.getBoundingClientRect();
      setTooltipPosition({
        top: rect.top + rect.height / 2,
        left: rect.right + 12,
      });
      setHoveredItem(label);
    }
  };

  const handleMouseLeave = () => {
    setHoveredItem(null);
  };

  const handleNavigateToMachineIdConfig = () => {
    navigate("/machine-id", {
      state: {
        highlightCustomPath: true,
        scrollToCustomPath: true,
      },
    });
  };

  const navItems = [
    {
      path: "/",
      label: "首页",
      icon: (
        <svg
          className="w-5 h-5"
          fill="none"
          stroke="currentColor"
          viewBox="0 0 24 24"
        >
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth={2}
            d="M3 12l2-2m0 0l7-7 7 7M5 10v10a1 1 0 001 1h3m10-11l2 2m-2-2v10a1 1 0 01-1 1h-3m-6 0a1 1 0 001-1v-4a1 1 0 011-1h2a1 1 0 011 1v4a1 1 0 001 1m-6 0h6"
          />
        </svg>
      ),
    },
    {
      path: "/machine-id",
      label: "Machine ID 管理",
      icon: (
        <svg
          className="w-5 h-5"
          fill="none"
          stroke="currentColor"
          viewBox="0 0 24 24"
        >
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth={2}
            d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z"
          />
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth={2}
            d="M15 12a3 3 0 11-6 0 3 3 0 016 0z"
          />
        </svg>
      ),
    },
    {
      path: "/auth-check",
      label: "授权检查",
      icon: (
        <svg
          className="w-5 h-5"
          fill="none"
          stroke="currentColor"
          viewBox="0 0 24 24"
        >
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth={2}
            d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z"
          />
        </svg>
      ),
    },
    {
      path: "/token-manage",
      label: "Token 管理",
      icon: (
        <svg
          className="w-5 h-5"
          fill="none"
          stroke="currentColor"
          viewBox="0 0 24 24"
        >
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth={2}
            d="M15 7a2 2 0 012 2m4 0a6 6 0 01-7.743 5.743L11 17H9v2H7v2H4a1 1 0 01-1-1v-2.586a1 1 0 01.293-.707l5.964-5.964A6 6 0 1121 9z"
          />
        </svg>
      ),
    },
    {
      path: "/auto-register",
      label: "自动注册",
      icon: (
        <svg
          className="w-5 h-5"
          fill="none"
          stroke="currentColor"
          viewBox="0 0 24 24"
        >
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth={2}
            d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z"
          />
        </svg>
      ),
    },
    {
      path: "/cursor-backup",
      label: "Cursor 备份",
      icon: (
        <svg
          className="w-5 h-5"
          fill="none"
          stroke="currentColor"
          viewBox="0 0 24 24"
        >
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth={2}
            d="M7 16a4 4 0 01-.88-7.903A5 5 0 1115.9 6L16 6a5 5 0 011 9.9M9 19l3 3m0 0l3-3m-3 3V10"
          />
        </svg>
      ),
    },
    {
      path: "/haozhu-api",
      label: "豪猪 API",
      openInNewWindow: true,
      icon: (
        <svg
          className="w-5 h-5"
          fill="none"
          stroke="currentColor"
          viewBox="0 0 24 24"
        >
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth={2}
            d="M12 18h.01M8 21h8a2 2 0 002-2V5a2 2 0 00-2-2H8a2 2 0 00-2 2v14a2 2 0 002 2z"
          />
        </svg>
      ),
    },
  ];

  return (
    <div className="flex flex-col h-screen min-h-0 app-shell window-frame">
      <div className="flex flex-col flex-1 min-h-0 window-surface">
        <TitleBar />
        <div className="flex flex-1 min-h-0">
          <aside
            className={`sticky top-0 z-30 h-full self-start overflow-hidden border-r border-slate-200/70 bg-white/78 pt-2 shadow-2xl shadow-slate-900/5 backdrop-blur-2xl transition-all duration-300 dark:border-slate-800/80 dark:bg-slate-950/78 dark:shadow-black/30 ${
              isMobile && isCollapsed ? "w-0" : isCollapsed ? "w-16" : "w-56"
            } ${isMobile && isCollapsed ? "-translate-x-full" : "translate-x-0"}`}
          >
            <div className="relative flex items-center px-2 border-b h-14 border-slate-200/80 dark:border-slate-800/80">
              {isCollapsed ? (
                <div className="flex flex-col items-center justify-center w-full gap-1">
                  <Link to="/" className="flex items-center">
                    <img
                      src={logoSvg}
                      alt="Cursor Manager Logo"
                      className="w-6 h-6"
                    />
                  </Link>
                  <button
                    onClick={toggleSidebar}
                    className="p-1 rounded-lg text-slate-500 hover:bg-slate-100 hover:text-slate-900 dark:text-slate-400 dark:hover:bg-slate-800 dark:hover:text-slate-100"
                    title="展开侧边栏"
                  >
                    <svg
                      className="h-3.5 w-3.5 rotate-180 transition-transform duration-300"
                      fill="none"
                      stroke="currentColor"
                      viewBox="0 0 24 24"
                    >
                      <path
                        strokeLinecap="round"
                        strokeLinejoin="round"
                        strokeWidth={2}
                        d="M11 19l-7-7 7-7m8 14l-7-7 7-7"
                      />
                    </svg>
                  </button>
                </div>
              ) : (
                <div className="flex items-center justify-between w-full gap-2 px-1">
                  <Link
                    to="/"
                    className="flex items-center flex-1 min-w-0 gap-3"
                  >
                    <div className="flex items-center justify-center h-9 w-9 rounded-2xl bg-blue-600/10 ring-1 ring-blue-500/20 dark:bg-blue-500/10 dark:ring-blue-400/20">
                      <img
                        src={logoSvg}
                        alt="Cursor Manager Logo"
                        className="w-6 h-6"
                      />
                    </div>
                    <div className="min-w-0">
                      <h1 className="text-sm font-bold truncate text-slate-900 dark:text-slate-100">
                        Cursor Manager
                      </h1>
                      <p className="truncate text-[11px] text-slate-500 dark:text-slate-400">
                        AI 工具台
                      </p>
                    </div>
                  </Link>
                  <button
                    onClick={toggleSidebar}
                    className="p-2 rounded-xl text-slate-500 hover:bg-slate-100 hover:text-slate-900 dark:text-slate-400 dark:hover:bg-slate-800 dark:hover:text-slate-100"
                    title="折叠侧边栏"
                  >
                    <svg
                      className="w-4 h-4"
                      fill="none"
                      stroke="currentColor"
                      viewBox="0 0 24 24"
                    >
                      <path
                        strokeLinecap="round"
                        strokeLinejoin="round"
                        strokeWidth={2}
                        d="M11 19l-7-7 7-7m8 14l-7-7 7-7"
                      />
                    </svg>
                  </button>
                </div>
              )}
            </div>

            <nav className="flex-1 px-2 py-3 space-y-1 overflow-y-auto scrollbar-none">
              {navItems.map((item) => {
                const isActive =
                  !item.openInNewWindow && location.pathname === item.path;
                const baseClass = `group relative flex items-center rounded-2xl text-sm font-medium transition-all duration-200 ${
                  isCollapsed ? "justify-center px-2 py-3" : "px-3 py-3"
                } ${
                  isActive
                    ? "bg-blue-600 text-white shadow-lg shadow-blue-500/25"
                    : "text-slate-600 hover:bg-slate-100 hover:text-slate-900 dark:text-slate-300 dark:hover:bg-slate-800 dark:hover:text-slate-100"
                }`;
                if (item.openInNewWindow) {
                  return (
                    <button
                      key={item.label}
                      type="button"
                      title="在新窗口打开"
                      onClick={() =>
                        void (async () => {
                          await invoke("open_haozhu_api_window");
                          const mode =
                            useConfigStore.getState().getThemeMode() ?? "light";
                          await broadcastThemeMode(mode);
                        })()
                      }
                      onMouseEnter={(e) => handleMouseEnter(item.label, e)}
                      onMouseLeave={handleMouseLeave}
                      className={`${baseClass} w-full border-0 bg-transparent text-left`}
                    >
                      <div
                        className={`${isCollapsed ? "" : "ml-0.5"} group-hover:scale-105 transition-transform`}
                      >
                        {item.icon}
                      </div>
                      <span
                        className={`ml-3 whitespace-nowrap transition-all duration-300 ${
                          isCollapsed
                            ? "w-0 overflow-hidden opacity-0"
                            : "opacity-100"
                        }`}
                      >
                        {item.label}
                      </span>
                    </button>
                  );
                }
                return (
                  <Link
                    key={item.path}
                    to={item.path}
                    onMouseEnter={(e) => handleMouseEnter(item.label, e)}
                    onMouseLeave={handleMouseLeave}
                    className={baseClass}
                  >
                    <div
                      className={`${isCollapsed ? "" : "ml-0.5"} ${isActive ? "" : "group-hover:scale-105"} transition-transform`}
                    >
                      {item.icon}
                    </div>
                    <span
                      className={`ml-3 whitespace-nowrap transition-all duration-300 ${
                        isCollapsed
                          ? "w-0 overflow-hidden opacity-0"
                          : "opacity-100"
                      }`}
                    >
                      {item.label}
                    </span>
                  </Link>
                );
              })}
            </nav>

            <div className="border-t border-slate-200/80 bg-white/65 p-2.5 dark:border-slate-800/80 dark:bg-slate-950/55">
              {!isCollapsed ? (
                <div className="space-y-2.5">
                  <button
                    onClick={toggleTheme}
                    className="flex w-full items-center justify-between rounded-2xl border border-slate-200/80 bg-slate-50/90 px-3 py-2.5 text-sm text-slate-700 hover:border-slate-300 hover:bg-white dark:border-slate-700 dark:bg-slate-900 dark:text-slate-200 dark:hover:border-slate-600 dark:hover:bg-slate-800"
                  >
                    <span className="flex items-center gap-2">
                      <span>{themeMode === "dark" ? "🌙" : "☀️"}</span>
                      <span>
                        {themeMode === "dark" ? "暗色模式" : "浅色模式"}
                      </span>
                    </span>
                    <span className="text-xs text-slate-400 dark:text-slate-500">
                      切换
                    </span>
                  </button>

                  <div className="space-y-2">
                    {cursorVersion && (
                      <div
                        className={`flex items-center justify-between rounded-2xl border px-3 py-2 text-xs ${
                          cursorVersionError
                            ? "border-amber-200 bg-amber-50 text-amber-700 dark:border-amber-500/30 dark:bg-amber-500/10 dark:text-amber-300"
                            : "border-blue-200 bg-blue-50 text-blue-700 dark:border-blue-500/30 dark:bg-blue-500/10 dark:text-blue-300"
                        }`}
                      >
                        <span className="font-semibold">Cursor</span>
                        <span>{cursorVersion}</span>
                      </div>
                    )}

                    {cursorVersionError && (
                      <button
                        onClick={handleNavigateToMachineIdConfig}
                        className="w-full px-3 py-2 text-xs font-medium border rounded-2xl border-amber-200 bg-amber-50 text-amber-700 hover:bg-amber-100 dark:border-amber-500/30 dark:bg-amber-500/10 dark:text-amber-300 dark:hover:bg-amber-500/15"
                        title="可能需要配置自定义Cursor路径"
                      >
                        配置路径
                      </button>
                    )}

                    {version && (
                      <div className="text-center text-[11px] text-slate-400 dark:text-slate-500">
                        工具 v{version}
                      </div>
                    )}
                  </div>

                  <div className="pt-2 border-t border-slate-200/80 dark:border-slate-800/80">
                    <div className="mb-2 text-xs font-semibold text-slate-600 dark:text-slate-300">
                      关于
                    </div>
                    <a
                      href="https://github.com/wuqi-y/auto-cursor-releases"
                      target="_blank"
                      rel="noopener noreferrer"
                      className="flex items-center px-3 py-2 text-xs rounded-2xl text-slate-600 hover:bg-slate-100 hover:text-blue-600 dark:text-slate-300 dark:hover:bg-slate-800 dark:hover:text-blue-300"
                      title="访问 GitHub"
                    >
                      <svg
                        className="mr-2 h-3.5 w-3.5 flex-shrink-0"
                        fill="currentColor"
                        viewBox="0 0 24 24"
                      >
                        <path
                          fillRule="evenodd"
                          d="M12 2C6.477 2 2 6.484 2 12.017c0 4.425 2.865 8.18 6.839 9.504.5.092.682-.217.682-.483 0-.237-.008-.868-.013-1.703-2.782.605-3.369-1.343-3.369-1.343-.454-1.158-1.11-1.466-1.11-1.466-.908-.62.069-.608.069-.608 1.003.07 1.531 1.032 1.531 1.032.892 1.53 2.341 1.088 2.91.832.092-.647.35-1.088.636-1.338-2.22-.253-4.555-1.113-4.555-4.951 0-1.093.39-1.988 1.029-2.688-.103-.253-.446-1.272.098-2.65 0 0 .84-.27 2.75 1.026A9.564 9.564 0 0112 6.844c.85.004 1.705.115 2.504.337 1.909-1.296 2.747-1.027 2.747-1.027.546 1.379.202 2.398.1 2.651.64.7 1.028 1.595 1.028 2.688 0 3.848-2.339 4.695-4.566 4.943.359.309.678.92.678 1.855 0 1.338-.012 2.419-.012 2.747 0 .268.18.58.688.482A10.019 10.019 0 0022 12.017C22 6.484 17.522 2 12 2z"
                          clipRule="evenodd"
                        />
                      </svg>
                      <span className="truncate">GitHub</span>
                    </a>
                  </div>
                </div>
              ) : (
                <div className="space-y-2">
                  <button
                    onClick={toggleTheme}
                    className="flex items-center justify-center w-full p-2 border rounded-xl border-slate-200/80 bg-slate-50/90 text-slate-600 hover:bg-white dark:border-slate-700 dark:bg-slate-900 dark:text-slate-300 dark:hover:bg-slate-800"
                    title="切换主题"
                  >
                    <span>{themeMode === "dark" ? "🌙" : "☀️"}</span>
                  </button>
                  {cursorVersion && (
                    <div
                      className="flex items-center justify-center text-xs font-bold text-blue-600 dark:text-blue-400"
                      title={`Cursor ${cursorVersion}`}
                    >
                      C
                    </div>
                  )}
                  <div className="flex flex-col items-center gap-2 pt-2 border-t border-slate-200/80 dark:border-slate-800/80">
                    <a
                      href="https://github.com/wuqi-y/auto-cursor-releases"
                      target="_blank"
                      rel="noopener noreferrer"
                      className="rounded-xl p-1.5 text-slate-400 hover:bg-slate-100 hover:text-blue-600 dark:text-slate-500 dark:hover:bg-slate-800 dark:hover:text-blue-300"
                      title="GitHub"
                    >
                      <svg
                        className="w-4 h-4"
                        fill="currentColor"
                        viewBox="0 0 24 24"
                      >
                        <path
                          fillRule="evenodd"
                          d="M12 2C6.477 2 2 6.484 2 12.017c0 4.425 2.865 8.18 6.839 9.504.5.092.682-.217.682-.483 0-.237-.008-.868-.013-1.703-2.782.605-3.369-1.343-3.369-1.343-.454-1.158-1.11-1.466-1.11-1.466-.908-.62.069-.608.069-.608 1.003.07 1.531 1.032 1.531 1.032.892 1.53 2.341 1.088 2.91.832.092-.647.35-1.088.636-1.338-2.22-.253-4.555-1.113-4.555-4.951 0-1.093.39-1.988 1.029-2.688-.103-.253-.446-1.272.098-2.65 0 0 .84-.27 2.75 1.026A9.564 9.564 0 0112 6.844c.85.004 1.705.115 2.504.337 1.909-1.296 2.747-1.027 2.747-1.027.546 1.379.202 2.398.1 2.651.64.7 1.028 1.595 1.028 2.688 0 3.848-2.339 4.695-4.566 4.943.359.309.678.92.678 1.855 0 1.338-.012 2.419-.012 2.747 0 .268.18.58.688.482A10.019 10.019 0 0022 12.017C22 6.484 17.522 2 12 2z"
                          clipRule="evenodd"
                        />
                      </svg>
                    </a>
                  </div>
                </div>
              )}
            </div>
          </aside>

          {isMobile && !isCollapsed && (
            <div
              className="fixed inset-0 z-40 bg-slate-950/55 backdrop-blur-sm"
              onClick={() => setIsCollapsed(true)}
            />
          )}

          {isCollapsed && hoveredItem && (
            <div
              className="animate-in pointer-events-none fixed z-[100] -translate-y-1/2 rounded-xl border border-slate-800 bg-slate-950 px-3 py-1.5 text-xs font-medium text-white shadow-2xl shadow-black/40"
              style={
                {
                  "--tooltip-top": `${tooltipPosition.top}px`,
                  "--tooltip-left": `${tooltipPosition.left}px`,
                  top: "var(--tooltip-top)",
                  left: "var(--tooltip-left)",
                } as React.CSSProperties
              }
            >
              {hoveredItem}
              <div className="absolute w-0 h-0 -translate-y-1/2 border-4 border-transparent -left-2 top-1/2 border-r-slate-950"></div>
            </div>
          )}

          <main className="flex flex-col flex-1 transition-all duration-300">
            {isMobile && (
              <div className="px-4 py-3 border-b border-slate-200/80 bg-white/80 backdrop-blur-xl dark:border-slate-800/80 dark:bg-slate-950/80">
                <div className="flex items-center justify-between">
                  <button
                    onClick={toggleSidebar}
                    className="p-2 rounded-xl text-slate-600 hover:bg-slate-100 dark:text-slate-300 dark:hover:bg-slate-800"
                    title="打开导航菜单"
                  >
                    <svg
                      className="w-6 h-6"
                      fill="none"
                      stroke="currentColor"
                      viewBox="0 0 24 24"
                    >
                      <path
                        strokeLinecap="round"
                        strokeLinejoin="round"
                        strokeWidth={2}
                        d="M4 6h16M4 12h16M4 18h16"
                      />
                    </svg>
                  </button>
                  <button
                    onClick={toggleTheme}
                    className="px-3 py-2 text-sm border rounded-xl border-slate-200/80 bg-slate-50/90 text-slate-700 dark:border-slate-700 dark:bg-slate-900 dark:text-slate-200"
                  >
                    {themeMode === "dark" ? "🌙 暗色" : "☀️ 浅色"}
                  </button>
                </div>
              </div>
            )}

            <div className="flex-1 p-4 overflow-auto scrollbar-none sm:p-6">
              <div className="max-w-6xl mx-auto">
                <Outlet />
              </div>
            </div>

            <footer className="border-t border-slate-200/80 bg-white/50 backdrop-blur-xl dark:border-slate-800/80 dark:bg-slate-950/35">
              <div className="max-w-6xl px-4 py-4 mx-auto sm:px-6">
                <div className="space-y-3 text-center">
                  <div className="text-xs text-slate-500 dark:text-slate-400">
                    <p>
                      项目地址：
                      <a
                        href="https://github.com/wuqi-y/auto-cursor-releases"
                        target="_blank"
                        rel="noopener noreferrer"
                        className="ml-1 text-blue-600 hover:text-blue-700 dark:text-blue-400 dark:hover:text-blue-300"
                      >
                        GitHub
                      </a>
                      <span className="mx-2">|</span>
                      <button
                        onClick={handleOpenLogDirectory}
                        className="text-blue-600 hover:underline dark:text-blue-400"
                      >
                        📂 打开日志目录
                      </button>
                    </p>
                    <p className="mt-1">
                      © 2025 Cursor Manager. 仅供学习研究使用.
                    </p>
                  </div>

                  <div className="text-[11px] leading-snug text-slate-500 dark:text-slate-400">
                    免责声明：本工具仅供学习和研究目的使用。使用本工具产生的任何后果由用户自行承担，开发者不承担任何法律责任。请遵守相关服务条款和法律法规。
                  </div>
                </div>
              </div>
            </footer>
          </main>
        </div>
      </div>
    </div>
  );
};
