import React, { useState, useEffect } from "react";
import { Link } from "react-router-dom";
import { CursorService } from "../services/cursorService";

import { LoadingSpinner } from "../components/LoadingSpinner";
import { Button } from "../components/Button";
import logoSvg from "../assets/logo.svg";

export const HomePage: React.FC = () => {
  const [cursorInstalled, setCursorInstalled] = useState<boolean | null>(null);
  const [cursorPaths, setCursorPaths] = useState<[string, string] | null>(null);
  const [loading, setLoading] = useState(true);
  const [debugInfo, setDebugInfo] = useState<string[]>([]);
  const [showDebug, setShowDebug] = useState(false);

  useEffect(() => {
    checkCursorInstallation();
  }, []);

  const checkCursorInstallation = async () => {
    try {
      setLoading(true);
      const installed = await CursorService.checkCursorInstallation();
      setCursorInstalled(installed);

      if (installed) {
        const paths = await CursorService.getCursorPaths();
        setCursorPaths(paths);
      } else {
        const debug = await CursorService.debugCursorPaths();
        setDebugInfo(debug);
      }
    } catch (error) {
      console.error("检查 Cursor 安装失败:", error);
      setCursorInstalled(false);
    } finally {
      setLoading(false);
    }
  };

  if (loading) {
    return <LoadingSpinner message="正在检查 Cursor 安装状态..." />;
  }

  return (
    <div className="space-y-6">
      <section className="surface-primary rounded-[28px] px-6 py-7 text-center sm:px-8">
        <div className="mb-4 flex items-center justify-center gap-4">
          <div className="flex h-16 w-16 items-center justify-center rounded-[22px] bg-blue-600/10 ring-1 ring-blue-500/20 dark:bg-blue-500/10 dark:ring-blue-400/20">
            <img src={logoSvg} alt="Cursor Manager Logo" className="h-10 w-10" />
          </div>
          <div className="text-left">
            <h1 className="text-3xl font-bold text-slate-900 dark:text-slate-100">
              Cursor Manager
            </h1>
            <p className="mt-1 text-sm text-slate-600 dark:text-slate-300">
              AI 代码助手管理工具
            </p>
          </div>
        </div>
        <p className="mx-auto max-w-2xl text-sm leading-6 text-slate-500 dark:text-slate-400">
          管理和恢复 Cursor 的 Machine ID、查看使用统计、进行账户与授权相关操作。
        </p>
      </section>

      <section className="surface-primary rounded-[28px] p-6 sm:p-7">
        <div className="mb-4 flex items-center justify-between gap-3">
          <div>
            <h2 className="text-lg font-semibold text-slate-900 dark:text-slate-100">
              Cursor 安装状态
            </h2>
            <p className="mt-1 text-sm text-slate-500 dark:text-slate-400">
              自动检测当前环境中的 Cursor 安装与配置路径。
            </p>
          </div>
          <div className="rounded-full border border-slate-200 bg-slate-50 px-3 py-1 text-xs font-medium text-slate-600 dark:border-slate-700 dark:bg-slate-900 dark:text-slate-300">
            实时检测
          </div>
        </div>

        {cursorInstalled === true ? (
          <div className="space-y-4">
            <div className="status-success flex items-center gap-2 rounded-2xl px-4 py-3 text-sm font-medium">
              <span className="text-base">✅</span>
              <span>Cursor 已安装，环境状态正常。</span>
            </div>

            {cursorPaths && (
              <div className="surface-secondary rounded-2xl p-4">
                <h3 className="mb-3 text-sm font-semibold text-slate-900 dark:text-slate-100">
                  安装路径
                </h3>
                <div className="space-y-2 text-sm text-slate-600 dark:text-slate-300">
                  <div className="rounded-xl bg-white/80 px-3 py-2 dark:bg-slate-900/80">
                    <strong className="mr-2 text-slate-800 dark:text-slate-100">应用路径:</strong>
                    <span className="break-all">{cursorPaths[0]}</span>
                  </div>
                  <div className="rounded-xl bg-white/80 px-3 py-2 dark:bg-slate-900/80">
                    <strong className="mr-2 text-slate-800 dark:text-slate-100">配置路径:</strong>
                    <span className="break-all">{cursorPaths[1]}</span>
                  </div>
                </div>
              </div>
            )}
          </div>
        ) : (
          <div className="space-y-4">
            <div className="status-error flex items-center gap-2 rounded-2xl px-4 py-3 text-sm font-medium">
              <span className="text-base">❌</span>
              <span>未检测到 Cursor 安装。</span>
            </div>

            <div className="rounded-2xl border border-red-200 bg-red-50/90 p-4 dark:border-red-500/25 dark:bg-red-500/10">
              <p className="mb-3 text-sm text-red-700 dark:text-red-300">
                请确保 Cursor 已正确安装并至少运行过一次。
              </p>

              <Button
                variant="secondary"
                size="sm"
                onClick={() => setShowDebug(!showDebug)}
              >
                {showDebug ? "隐藏" : "显示"}调试信息
              </Button>

              {showDebug && debugInfo.length > 0 && (
                <div className="mt-3 space-y-2">
                  {debugInfo.map((info, index) => (
                    <p
                      key={index}
                      className="rounded-xl border border-red-200 bg-white/80 p-3 text-xs text-red-700 dark:border-red-500/20 dark:bg-slate-900/70 dark:text-red-300"
                    >
                      {info}
                    </p>
                  ))}
                </div>
              )}
            </div>
          </div>
        )}
      </section>

      {cursorInstalled && (
        <section className="grid grid-cols-1 gap-4 md:grid-cols-2">
          <div className="surface-primary card-hover rounded-[26px] p-6">
            <div className="mb-4 flex items-center gap-3">
              <div className="flex h-11 w-11 items-center justify-center rounded-2xl bg-blue-600/10 text-xl dark:bg-blue-500/10">
                🔧
              </div>
              <div>
                <h3 className="text-base font-semibold text-slate-900 dark:text-slate-100">
                  Machine ID 管理
                </h3>
                <p className="text-sm text-slate-500 dark:text-slate-400">
                  备份、恢复或重置设备标识。
                </p>
              </div>
            </div>
            <Link to="/machine-id">
              <Button variant="primary" className="w-full">
                进入管理
              </Button>
            </Link>
          </div>

          <div className="surface-primary card-hover rounded-[26px] p-6">
            <div className="mb-4 flex items-center gap-3">
              <div className="flex h-11 w-11 items-center justify-center rounded-2xl bg-emerald-600/10 text-xl dark:bg-emerald-500/10">
                🔐
              </div>
              <div>
                <h3 className="text-base font-semibold text-slate-900 dark:text-slate-100">
                  授权检查
                </h3>
                <p className="text-sm text-slate-500 dark:text-slate-400">
                  查看账户授权状态与订阅信息。
                </p>
              </div>
            </div>
            <Link to="/auth-check">
              <Button variant="primary" className="w-full">
                开始检查
              </Button>
            </Link>
          </div>
        </section>
      )}

      <div className="flex justify-center">
        <Button variant="secondary" onClick={checkCursorInstallation} loading={loading}>
          重新检查
        </Button>
      </div>
    </div>
  );
};
