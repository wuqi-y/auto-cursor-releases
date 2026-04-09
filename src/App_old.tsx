import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AuthCheckResult } from "./types/auth";
import "./App.css";

interface BackupInfo {
  path: string;
  filename: string;
  timestamp: string;
  size: number;
  date_formatted: string;
}

interface MachineIds {
  "telemetry.devDeviceId": string;
  "telemetry.macMachineId": string;
  "telemetry.machineId": string;
  "telemetry.sqmId": string;
  "storage.serviceMachineId": string;
}

interface RestoreResult {
  success: boolean;
  message: string;
  details: string[];
}

interface ResetResult {
  success: boolean;
  message: string;
  details: string[];
  new_ids?: MachineIds;
}

interface TokenInfo {
  token?: string;
  source: string;
  found: boolean;
  message: string;
}

function App() {
  const [backups, setBackups] = useState<BackupInfo[]>([]);
  const [selectedBackup, setSelectedBackup] = useState<BackupInfo | null>(null);
  const [selectedIds, setSelectedIds] = useState<MachineIds | null>(null);
  const [loading, setLoading] = useState(false);
  const [cursorInstalled, setCursorInstalled] = useState<boolean | null>(null);
  const [cursorPaths, setCursorPaths] = useState<[string, string] | null>(null);
  const [restoreResult, setRestoreResult] = useState<RestoreResult | null>(
    null
  );
  const [resetResult, setResetResult] = useState<ResetResult | null>(null);
  const [currentMachineIds, setCurrentMachineIds] = useState<MachineIds | null>(
    null
  );
  const [machineIdFileContent, setMachineIdFileContent] = useState<
    string | null
  >(null);
  const [backupDebugInfo, setBackupDebugInfo] = useState<{
    directory: string;
    files: string[];
  } | null>(null);
  const [currentStep, setCurrentStep] = useState<
    | "check"
    | "menu"
    | "select"
    | "preview"
    | "confirm"
    | "result"
    | "reset"
    | "complete_reset"
    | "auth_check"
  >("check");
  const [authResult, setAuthResult] = useState<AuthCheckResult | null>(null);
  const [userToken, setUserToken] = useState<string>("");
  const [tokenInfo, setTokenInfo] = useState<TokenInfo | null>(null);
  const [autoTokenLoading, setAutoTokenLoading] = useState<boolean>(false);
  const [debugInfo, setDebugInfo] = useState<string[]>([]);
  const [showDebug, setShowDebug] = useState<boolean>(false);

  useEffect(() => {
    checkCursorInstallation();
  }, []);

  // Auto-load token when entering auth check page
  useEffect(() => {
    if (currentStep === "auth_check" && !tokenInfo) {
      getTokenAuto();
    }
  }, [currentStep]);

  const checkCursorInstallation = async () => {
    try {
      setLoading(true);
      const installed = await invoke<boolean>("check_cursor_installation");
      setCursorInstalled(installed);

      if (installed) {
        const paths = await invoke<[string, string]>("get_cursor_paths");
        setCursorPaths(paths);
        await loadBackups();
        await loadCurrentMachineIds();
        await loadBackupDebugInfo();
        setCurrentStep("menu");
      }
    } catch (error) {
      console.error("Error checking Cursor installation:", error);
      setCursorInstalled(false);
    } finally {
      setLoading(false);
    }
  };

  const loadBackups = async () => {
    try {
      const backupList = await invoke<BackupInfo[]>("get_available_backups");
      setBackups(backupList);
    } catch (error) {
      console.error("Error loading backups:", error);
    }
  };

  const loadCurrentMachineIds = async () => {
    try {
      const currentIds = await invoke<MachineIds | null>(
        "get_current_machine_ids"
      );
      setCurrentMachineIds(currentIds);

      const fileContent = await invoke<string | null>(
        "get_machine_id_file_content"
      );
      setMachineIdFileContent(fileContent);
    } catch (error) {
      console.error("Error loading current machine IDs:", error);
    }
  };

  const loadBackupDebugInfo = async () => {
    try {
      const [directory, files] = await invoke<[string, string[]]>(
        "get_backup_directory_info"
      );
      setBackupDebugInfo({ directory, files });
    } catch (error) {
      console.error("Error loading backup debug info:", error);
    }
  };

  const selectBackup = async (backup: BackupInfo) => {
    try {
      setLoading(true);
      setSelectedBackup(backup);
      const ids = await invoke<MachineIds>("extract_backup_ids", {
        backupPath: backup.path,
      });
      setSelectedIds(ids);
      setCurrentStep("preview");
    } catch (error) {
      console.error("Error extracting IDs from backup:", error);
      alert("无法从备份中提取机器ID信息");
    } finally {
      setLoading(false);
    }
  };

  const confirmRestore = () => {
    setCurrentStep("confirm");
  };

  const executeRestore = async () => {
    if (!selectedBackup) return;

    try {
      setLoading(true);
      const result = await invoke<RestoreResult>("restore_machine_ids", {
        backupPath: selectedBackup.path,
      });
      setRestoreResult(result);
      setCurrentStep("result");

      // Reload current machine IDs after successful restore
      if (result.success) {
        await loadCurrentMachineIds();
        await loadBackupDebugInfo();
      }
    } catch (error) {
      console.error("Error restoring machine IDs:", error);
      setRestoreResult({
        success: false,
        message: `恢复过程中发生错误: ${error}`,
        details: [],
      });
      setCurrentStep("result");
    } finally {
      setLoading(false);
    }
  };

  const executeReset = async () => {
    try {
      setLoading(true);
      const result = await invoke<ResetResult>("reset_machine_ids");
      setResetResult(result);
      setCurrentStep("result");

      // Reload current machine IDs after successful reset
      if (result.success) {
        await loadCurrentMachineIds();
        await loadBackupDebugInfo();
      }
    } catch (error) {
      console.error("Error resetting machine IDs:", error);
      setResetResult({
        success: false,
        message: `重置过程中发生错误: ${error}`,
        details: [],
      });
      setCurrentStep("result");
    } finally {
      setLoading(false);
    }
  };

  const executeCompleteReset = async () => {
    try {
      setLoading(true);
      const result = await invoke<ResetResult>("complete_cursor_reset");
      setResetResult(result);
      setCurrentStep("result");

      // Reload current machine IDs after successful complete reset
      if (result.success) {
        await loadCurrentMachineIds();
        await loadBackupDebugInfo();
      }
    } catch (error) {
      console.error("Error completing Cursor reset:", error);
      setResetResult({
        success: false,
        message: `完全重置过程中发生错误: ${error}`,
        details: [],
      });
      setCurrentStep("result");
    } finally {
      setLoading(false);
    }
  };

  const resetProcess = () => {
    setSelectedBackup(null);
    setSelectedIds(null);
    setRestoreResult(null);
    setResetResult(null);
    setAuthResult(null);
    setCurrentStep("menu");
    loadBackups();
    loadCurrentMachineIds(); // Reload current machine IDs after reset
    loadBackupDebugInfo(); // Reload backup debug info
  };

  const getTokenAuto = async () => {
    try {
      setAutoTokenLoading(true);
      const result = await invoke<TokenInfo>("get_token_auto");
      setTokenInfo(result);

      if (result.found && result.token) {
        setUserToken(result.token);
      }
    } catch (error) {
      console.error("Error getting token automatically:", error);
      setTokenInfo({
        token: undefined,
        source: "Error",
        found: false,
        message: `获取 Token 时发生错误: ${error}`,
      });
    } finally {
      setAutoTokenLoading(false);
    }
  };

  const getDebugInfo = async () => {
    try {
      const result = await invoke<string[]>("debug_cursor_paths");
      setDebugInfo(result);
      setShowDebug(true);
    } catch (error) {
      console.error("Error getting debug info:", error);
      setDebugInfo([`获取调试信息时发生错误: ${error}`]);
      setShowDebug(true);
    }
  };

  const checkUserAuthorization = async () => {
    if (!userToken.trim()) {
      alert("请输入有效的 Cursor Token");
      return;
    }

    try {
      setLoading(true);
      const result = await invoke<AuthCheckResult>("check_user_authorization", {
        token: userToken,
      });
      setAuthResult(result);
      setCurrentStep("result");
    } catch (error) {
      console.error("Error checking user authorization:", error);
      setAuthResult({
        success: false,
        message: `授权检查过程中发生错误: ${error}`,
        details: [],
      });
      setCurrentStep("result");
    } finally {
      setLoading(false);
    }
  };

  const formatFileSize = (bytes: number) => {
    return (bytes / 1024).toFixed(1) + " KB";
  };

  // Component to display current machine ID info
  const CurrentMachineInfo = () => (
    <div className="p-6 mb-6 bg-white rounded-lg shadow-md">
      <h2 className="flex items-center mb-4 text-xl font-semibold">
        🔍 当前机器ID信息
      </h2>

      {currentMachineIds ? (
        <div className="space-y-4">
          <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
            {Object.entries(currentMachineIds).map(([key, value]) => (
              <div
                key={key}
                className="py-2 pl-4 border-l-4 border-blue-500 rounded-r bg-blue-50"
              >
                <p className="mb-1 font-mono text-sm text-gray-600">{key}</p>
                <p className="font-mono text-sm text-blue-700 break-all">
                  {value || "(空)"}
                </p>
              </div>
            ))}
          </div>

          {machineIdFileContent && (
            <div className="p-4 mt-4 rounded-lg bg-gray-50">
              <h3 className="mb-2 font-medium text-gray-700">
                machineId 文件内容:
              </h3>
              <p className="p-3 font-mono text-sm text-gray-600 break-all bg-white border rounded">
                {machineIdFileContent}
              </p>
            </div>
          )}

          <div className="flex gap-2 mt-4">
            <button
              onClick={loadCurrentMachineIds}
              className="px-4 py-2 text-sm text-white transition-colors bg-blue-500 rounded hover:bg-blue-600"
            >
              🔄 刷新信息
            </button>
          </div>
        </div>
      ) : (
        <div className="py-6 text-center">
          <div className="mb-4 text-4xl text-gray-400">❓</div>
          <p className="mb-4 text-gray-600">未找到当前机器ID信息</p>
          <p className="mb-4 text-sm text-gray-500">
            可能是因为 Cursor 尚未配置或配置文件不存在
          </p>
          <button
            onClick={loadCurrentMachineIds}
            className="px-4 py-2 text-sm text-white transition-colors bg-blue-500 rounded hover:bg-blue-600"
          >
            🔄 重新检查
          </button>
        </div>
      )}
    </div>
  );

  if (loading && currentStep === "check") {
    return (
      <div className="flex items-center justify-center min-h-screen bg-gray-100">
        <div className="text-center">
          <div className="w-12 h-12 mx-auto mb-4 border-b-2 border-blue-500 rounded-full animate-spin"></div>
          <p className="text-gray-600">正在检查 Cursor 安装...</p>
        </div>
      </div>
    );
  }

  if (cursorInstalled === false) {
    return (
      <div className="flex items-center justify-center min-h-screen bg-gray-100">
        <div className="max-w-md p-8 text-center bg-white rounded-lg shadow-lg">
          <div className="mb-4 text-6xl text-red-500">❌</div>
          <h1 className="mb-4 text-2xl font-bold text-gray-800">
            未找到 Cursor
          </h1>
          <p className="mb-6 text-gray-600">
            系统中未检测到 Cursor 编辑器的安装。请确保已正确安装 Cursor。
          </p>
          <button
            onClick={checkCursorInstallation}
            className="px-6 py-2 text-white transition-colors bg-blue-500 rounded-lg hover:bg-blue-600"
          >
            重新检查
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="min-h-screen bg-gray-100">
      <header className="p-6 text-white bg-blue-600">
        <h1 className="text-3xl font-bold text-center">
          Cursor 机器ID 管理工具
        </h1>
        <p className="mt-2 text-center text-blue-100">
          恢复、重置和管理 Cursor 机器标识
        </p>
      </header>

      <main className="container max-w-4xl px-4 py-8 mx-auto">
        {/* Current Cursor Paths Info */}
        {cursorPaths && (
          <div className="p-6 mb-6 bg-white rounded-lg shadow-md">
            <h2 className="flex items-center mb-4 text-xl font-semibold">
              📁 Cursor 配置路径
            </h2>
            <div className="grid grid-cols-1 gap-4 text-sm md:grid-cols-2">
              <div>
                <p className="font-medium text-gray-700">存储文件:</p>
                <code className="block p-2 mt-1 break-all bg-gray-100 rounded">
                  {cursorPaths[0]}
                </code>
              </div>
              <div>
                <p className="font-medium text-gray-700">数据库文件:</p>
                <code className="block p-2 mt-1 break-all bg-gray-100 rounded">
                  {cursorPaths[1]}
                </code>
              </div>
            </div>
          </div>
        )}

        {/* Current Machine ID Info - Show on menu */}
        {currentStep === "menu" && <CurrentMachineInfo />}

        {/* Menu - Choose Operation */}
        {currentStep === "menu" && (
          <div className="p-6 bg-white rounded-lg shadow-md">
            <h2 className="flex items-center mb-6 text-xl font-semibold">
              🔧 选择操作
            </h2>

            <div className="grid grid-cols-1 gap-4 md:grid-cols-2 lg:grid-cols-4">
              {/* Check User Authorization */}
              <div
                className="p-6 transition-colors border-2 border-green-200 rounded-lg cursor-pointer hover:border-green-400"
                onClick={() => setCurrentStep("auth_check")}
              >
                <div className="text-center">
                  <div className="mb-4 text-4xl">🔑</div>
                  <h3 className="mb-2 font-semibold text-gray-800">
                    检查用户授权
                  </h3>
                  <p className="text-sm text-gray-600">
                    验证 Cursor Token 的有效性
                  </p>
                </div>
              </div>

              {/* Restore from Backup */}
              <div
                className="p-6 transition-colors border-2 border-blue-200 rounded-lg cursor-pointer hover:border-blue-400"
                onClick={() => setCurrentStep("select")}
              >
                <div className="text-center">
                  <div className="mb-4 text-4xl">💾</div>
                  <h3 className="mb-2 font-semibold text-gray-800">
                    从备份恢复
                  </h3>
                  <p className="text-sm text-gray-600">
                    从之前的备份文件恢复机器ID
                  </p>
                </div>
              </div>

              {/* Reset Machine IDs */}
              <div
                className="p-6 transition-colors border-2 border-orange-200 rounded-lg cursor-pointer hover:border-orange-400"
                onClick={() => setCurrentStep("reset")}
              >
                <div className="text-center">
                  <div className="mb-4 text-4xl">🔄</div>
                  <h3 className="mb-2 font-semibold text-gray-800">
                    重置机器ID
                  </h3>
                  <p className="text-sm text-gray-600">
                    生成新的机器ID并更新配置
                  </p>
                </div>
              </div>

              {/* Complete Cursor Reset */}
              <div
                className="p-6 transition-colors border-2 border-red-200 rounded-lg cursor-pointer hover:border-red-400"
                onClick={() => setCurrentStep("complete_reset")}
              >
                <div className="text-center">
                  <div className="mb-4 text-4xl">🔥</div>
                  <h3 className="mb-2 font-semibold text-gray-800">完全重置</h3>
                  <p className="text-sm text-gray-600">
                    重置机器ID并修改Cursor文件
                  </p>
                </div>
              </div>
            </div>
          </div>
        )}

        {/* User Authorization Check */}
        {currentStep === "auth_check" && (
          <div className="p-6 bg-white rounded-lg shadow-md">
            <h2 className="flex items-center mb-4 text-xl font-semibold text-green-600">
              🔑 检查用户授权
            </h2>

            <div className="mb-4">
              <button
                type="button"
                onClick={() => setCurrentStep("menu")}
                className="px-4 py-2 text-white transition-colors bg-gray-500 rounded hover:bg-gray-600"
              >
                ← 返回菜单
              </button>
            </div>

            <div className="p-4 mb-6 border border-green-200 rounded-lg bg-green-50">
              <h3 className="mb-2 font-medium text-green-800">功能说明:</h3>
              <ul className="space-y-1 text-sm text-green-700">
                <li>• 自动检测并获取 Cursor Token</li>
                <li>• 验证您的 Cursor Token 是否有效</li>
                <li>• 检查 API 访问权限</li>
                <li>• 显示详细的授权信息</li>
                <li>• 支持 JWT 格式的 Token 验证</li>
              </ul>
            </div>

            {/* Auto Token Detection */}
            <div className="p-4 mb-6 border border-blue-200 rounded-lg bg-blue-50">
              <div className="flex items-center justify-between mb-3">
                <h3 className="font-medium text-blue-800">自动获取 Token</h3>
                <div className="flex gap-2">
                  <button
                    type="button"
                    onClick={getTokenAuto}
                    disabled={autoTokenLoading}
                    className="px-3 py-1 text-sm text-white transition-colors bg-blue-500 rounded hover:bg-blue-600 disabled:bg-blue-300"
                  >
                    {autoTokenLoading ? "获取中..." : "🔍 自动获取"}
                  </button>
                  <button
                    type="button"
                    onClick={getDebugInfo}
                    className="px-3 py-1 text-sm text-white transition-colors bg-gray-500 rounded hover:bg-gray-600"
                  >
                    🔧 调试信息
                  </button>
                </div>
              </div>

              {tokenInfo && (
                <div className="space-y-2">
                  <div className="flex items-center gap-2">
                    <span
                      className={`text-sm font-medium ${
                        tokenInfo.found ? "text-green-600" : "text-red-600"
                      }`}
                    >
                      {tokenInfo.found ? "✅ 找到 Token" : "❌ 未找到 Token"}
                    </span>
                    <span className="text-xs text-gray-500">
                      来源: {tokenInfo.source}
                    </span>
                  </div>
                  <p className="text-sm text-blue-700">{tokenInfo.message}</p>
                  {tokenInfo.found && tokenInfo.token && (
                    <div className="p-2 bg-white border rounded">
                      <p className="mb-1 text-xs text-gray-600">Token 预览:</p>
                      <p className="font-mono text-xs text-gray-800 break-all">
                        {tokenInfo.token.substring(0, 50)}...
                      </p>
                    </div>
                  )}
                </div>
              )}
            </div>

            {/* Debug Information */}
            {showDebug && debugInfo.length > 0 && (
              <div className="p-4 mb-6 border border-yellow-200 rounded-lg bg-yellow-50">
                <div className="flex items-center justify-between mb-3">
                  <h3 className="font-medium text-yellow-800">调试信息</h3>
                  <button
                    type="button"
                    onClick={() => setShowDebug(false)}
                    className="px-2 py-1 text-xs text-white transition-colors bg-yellow-500 rounded hover:bg-yellow-600"
                  >
                    关闭
                  </button>
                </div>
                <div className="overflow-y-auto max-h-64">
                  {debugInfo.map((info, index) => (
                    <div
                      key={index}
                      className="mb-1 font-mono text-xs text-yellow-800 break-all"
                    >
                      {info}
                    </div>
                  ))}
                </div>
              </div>
            )}

            <div className="mb-6">
              <label className="block mb-2 text-sm font-medium text-gray-700">
                Cursor Token:
              </label>
              <textarea
                value={userToken}
                onChange={(e) => setUserToken(e.target.value)}
                placeholder="请输入您的 Cursor Token 或点击上方自动获取..."
                className="w-full p-3 border border-gray-300 rounded-lg resize-none focus:ring-2 focus:ring-green-500 focus:border-transparent"
                rows={4}
              />
              <p className="mt-1 text-xs text-gray-500">
                支持完整 Token 或 :: 分隔的格式
              </p>
            </div>

            <div className="flex gap-4">
              <button
                type="button"
                onClick={checkUserAuthorization}
                disabled={loading || !userToken.trim()}
                className="px-6 py-2 text-white transition-colors bg-green-500 rounded hover:bg-green-600 disabled:bg-green-300"
              >
                {loading ? "检查中..." : "检查授权"}
              </button>
              <button
                type="button"
                onClick={() => {
                  setUserToken("");
                  setTokenInfo(null);
                }}
                className="px-6 py-2 text-white transition-colors bg-gray-500 rounded hover:bg-gray-600"
              >
                清空
              </button>
            </div>
          </div>
        )}

        {/* Step 1: Select Backup */}
        {currentStep === "select" && (
          <div className="p-6 bg-white rounded-lg shadow-md">
            <h2 className="flex items-center mb-4 text-xl font-semibold">
              💾 选择备份文件
            </h2>

            <div className="mb-4">
              <button
                onClick={() => setCurrentStep("menu")}
                className="px-4 py-2 text-white transition-colors bg-gray-500 rounded hover:bg-gray-600"
              >
                ← 返回菜单
              </button>
            </div>

            {/* Debug Information */}
            {backupDebugInfo && (
              <div className="p-4 mb-6 rounded-lg bg-gray-50">
                <h3 className="mb-2 font-medium text-gray-700">🔧 调试信息</h3>
                <div className="text-sm">
                  <p className="mb-2">
                    <span className="font-medium">备份目录:</span>
                    <code className="px-2 py-1 ml-2 text-xs break-all bg-gray-200 rounded">
                      {backupDebugInfo.directory}
                    </code>
                  </p>
                  <p className="mb-2">
                    <span className="font-medium">
                      发现 {backupDebugInfo.files.length} 个相关文件:
                    </span>
                  </p>
                  {backupDebugInfo.files.length > 0 ? (
                    <div className="overflow-y-auto max-h-32">
                      {backupDebugInfo.files.map((file, index) => (
                        <div
                          key={index}
                          className="p-1 mb-1 ml-4 font-mono text-xs bg-white border rounded"
                        >
                          {file}
                        </div>
                      ))}
                    </div>
                  ) : (
                    <p className="ml-4 text-xs text-gray-500">
                      没有找到任何 storage.json 相关文件
                    </p>
                  )}
                  <button
                    onClick={loadBackupDebugInfo}
                    className="px-3 py-1 mt-2 text-xs text-white transition-colors bg-gray-400 rounded hover:bg-gray-500"
                  >
                    🔄 刷新文件列表
                  </button>
                </div>
              </div>
            )}

            {backups.length === 0 ? (
              <div className="py-8 text-center">
                <div className="mb-4 text-4xl text-yellow-500">⚠️</div>
                <p className="mb-2 text-gray-600">未找到任何备份文件</p>
                <p className="mb-4 text-sm text-gray-500">
                  备份文件应该以 .bak. 或 .backup. 格式结尾
                </p>
                <div className="flex justify-center gap-2">
                  <button
                    onClick={loadBackups}
                    className="px-4 py-2 text-white transition-colors bg-blue-500 rounded hover:bg-blue-600"
                  >
                    🔄 刷新备份列表
                  </button>
                  <button
                    onClick={loadBackupDebugInfo}
                    className="px-4 py-2 text-white transition-colors bg-gray-500 rounded hover:bg-gray-600"
                  >
                    🔧 刷新调试信息
                  </button>
                </div>
              </div>
            ) : (
              <div className="space-y-3">
                <p className="mb-3 text-sm text-gray-600">
                  找到 {backups.length} 个备份文件：
                </p>
                {backups.map((backup, index) => (
                  <div
                    key={index}
                    className="p-4 transition-colors border rounded-lg cursor-pointer hover:bg-gray-50"
                    onClick={() => selectBackup(backup)}
                  >
                    <div className="flex items-center justify-between">
                      <div>
                        <h3 className="font-medium text-gray-800">
                          {backup.filename}
                        </h3>
                        <p className="text-sm text-gray-600">
                          {backup.date_formatted} •{" "}
                          {formatFileSize(backup.size)}
                        </p>
                      </div>
                      <div className="text-blue-500">→</div>
                    </div>
                  </div>
                ))}
              </div>
            )}
          </div>
        )}

        {/* Reset Confirmation */}
        {currentStep === "reset" && (
          <div className="p-6 bg-white rounded-lg shadow-md">
            <h2 className="flex items-center mb-4 text-xl font-semibold text-orange-600">
              🔄 确认重置机器ID
            </h2>

            <div className="p-4 mb-6 border border-orange-200 rounded-lg bg-orange-50">
              <h3 className="mb-2 font-medium text-orange-800">重要提醒:</h3>
              <ul className="space-y-1 text-sm text-orange-700">
                <li>• 此操作将生成全新的机器ID</li>
                <li>• 系统会自动创建当前配置的备份</li>
                <li>• 重置后需要重启 Cursor 编辑器</li>
                <li>• 某些系统级操作可能需要管理员权限</li>
              </ul>
            </div>

            <p className="mb-6 text-gray-600">
              确定要重置机器ID吗？这将生成全新的机器标识。
            </p>

            <div className="flex gap-4">
              <button
                onClick={() => setCurrentStep("menu")}
                className="px-6 py-2 text-white transition-colors bg-gray-500 rounded hover:bg-gray-600"
              >
                取消
              </button>
              <button
                onClick={executeReset}
                disabled={loading}
                className="px-6 py-2 text-white transition-colors bg-orange-500 rounded hover:bg-orange-600 disabled:bg-orange-300"
              >
                {loading ? "重置中..." : "确认重置"}
              </button>
            </div>
          </div>
        )}

        {/* Complete Reset Confirmation */}
        {currentStep === "complete_reset" && (
          <div className="p-6 bg-white rounded-lg shadow-md">
            <h2 className="flex items-center mb-4 text-xl font-semibold text-red-600">
              🔥 确认完全重置 Cursor
            </h2>

            <div className="p-4 mb-6 border border-red-200 rounded-lg bg-red-50">
              <h3 className="mb-2 font-medium text-red-800">重要提醒:</h3>
              <ul className="space-y-1 text-sm text-red-700">
                <li>• 此操作将重置机器ID并修改Cursor程序文件</li>
                <li>• 包括修改 main.js 和 workbench.desktop.main.js</li>
                <li>• 系统会自动创建所有文件的备份</li>
                <li>• 重置后需要重启 Cursor 编辑器</li>
                <li>• 某些操作可能需要管理员权限</li>
              </ul>
            </div>

            <p className="mb-6 text-gray-600">
              确定要执行完全重置吗？这是一个更彻底的重置操作。
            </p>

            <div className="flex gap-4">
              <button
                onClick={() => setCurrentStep("menu")}
                className="px-6 py-2 text-white transition-colors bg-gray-500 rounded hover:bg-gray-600"
              >
                取消
              </button>
              <button
                onClick={executeCompleteReset}
                disabled={loading}
                className="px-6 py-2 text-white transition-colors bg-red-500 rounded hover:bg-red-600 disabled:bg-red-300"
              >
                {loading ? "重置中..." : "确认完全重置"}
              </button>
            </div>
          </div>
        )}

        {/* Step 2: Preview IDs */}
        {currentStep === "preview" && selectedIds && selectedBackup && (
          <div className="p-6 bg-white rounded-lg shadow-md">
            <h2 className="flex items-center mb-4 text-xl font-semibold">
              🔍 预览机器ID信息
            </h2>

            <div className="mb-6">
              <h3 className="mb-2 font-medium text-gray-700">选中的备份:</h3>
              <div className="p-3 bg-gray-100 rounded">
                <p className="font-medium">{selectedBackup.filename}</p>
                <p className="text-sm text-gray-600">
                  {selectedBackup.date_formatted}
                </p>
              </div>
            </div>

            <div className="space-y-4">
              <h3 className="font-medium text-gray-700">将要恢复的机器ID:</h3>

              {Object.entries(selectedIds).map(([key, value]) => (
                <div key={key} className="py-2 pl-4 border-l-4 border-blue-500">
                  <p className="font-mono text-sm text-gray-600">{key}</p>
                  <p className="font-mono text-green-600 break-all">
                    {value || "(空)"}
                  </p>
                </div>
              ))}
            </div>

            <div className="flex gap-4 mt-6">
              <button
                onClick={() => setCurrentStep("select")}
                className="px-6 py-2 text-white transition-colors bg-gray-500 rounded hover:bg-gray-600"
              >
                返回
              </button>
              <button
                onClick={confirmRestore}
                className="px-6 py-2 text-white transition-colors bg-blue-500 rounded hover:bg-blue-600"
              >
                继续恢复
              </button>
            </div>
          </div>
        )}

        {/* Step 3: Confirmation */}
        {currentStep === "confirm" && (
          <div className="p-6 bg-white rounded-lg shadow-md">
            <h2 className="flex items-center mb-4 text-xl font-semibold text-orange-600">
              ⚠️ 确认恢复操作
            </h2>

            <div className="p-4 mb-6 border border-yellow-200 rounded-lg bg-yellow-50">
              <h3 className="mb-2 font-medium text-yellow-800">重要提醒:</h3>
              <ul className="space-y-1 text-sm text-yellow-700">
                <li>• 此操作将覆盖当前的机器ID设置</li>
                <li>• 系统会自动创建当前配置的备份</li>
                <li>• 恢复后需要重启 Cursor 编辑器</li>
                <li>• 某些系统级操作可能需要管理员权限</li>
              </ul>
            </div>

            <p className="mb-6 text-gray-600">
              确定要从备份 <strong>{selectedBackup?.filename}</strong>{" "}
              恢复机器ID吗？
            </p>

            <div className="flex gap-4">
              <button
                onClick={() => setCurrentStep("preview")}
                className="px-6 py-2 text-white transition-colors bg-gray-500 rounded hover:bg-gray-600"
              >
                取消
              </button>
              <button
                onClick={executeRestore}
                disabled={loading}
                className="px-6 py-2 text-white transition-colors bg-red-500 rounded hover:bg-red-600 disabled:bg-red-300"
              >
                {loading ? "恢复中..." : "确认恢复"}
              </button>
            </div>
          </div>
        )}

        {/* Step 4: Result */}
        {currentStep === "result" &&
          (restoreResult || resetResult || authResult) && (
            <div className="p-6 bg-white rounded-lg shadow-md">
              {(() => {
                // Handle authorization result
                if (authResult) {
                  return (
                    <>
                      <h2 className="flex items-center mb-4 text-xl font-semibold">
                        {authResult.success
                          ? "✅ 授权检查完成"
                          : "❌ 授权检查失败"}
                      </h2>

                      <div
                        className={`p-4 rounded-lg mb-6 ${
                          authResult.success
                            ? "bg-green-50 border border-green-200"
                            : "bg-red-50 border border-red-200"
                        }`}
                      >
                        <p
                          className={`font-medium ${
                            authResult.success
                              ? "text-green-800"
                              : "text-red-800"
                          }`}
                        >
                          {authResult.message}
                        </p>
                      </div>

                      {authResult.user_info && (
                        <div className="mb-6">
                          <h3 className="mb-3 font-medium text-gray-700">
                            用户信息:
                          </h3>
                          <div className="space-y-3">
                            <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
                              <div className="p-3 rounded bg-gray-50">
                                <p className="text-sm text-gray-600">
                                  授权状态
                                </p>
                                <p
                                  className={`font-medium ${
                                    authResult.user_info.is_authorized
                                      ? "text-green-600"
                                      : "text-red-600"
                                  }`}
                                >
                                  {authResult.user_info.is_authorized
                                    ? "已授权"
                                    : "未授权"}
                                </p>
                              </div>
                              <div className="p-3 rounded bg-gray-50">
                                <p className="text-sm text-gray-600">
                                  Token 长度
                                </p>
                                <p className="font-medium text-gray-800">
                                  {authResult.user_info.token_length} 字符
                                </p>
                              </div>
                              <div className="p-3 rounded bg-gray-50">
                                <p className="text-sm text-gray-600">
                                  Token 格式
                                </p>
                                <p
                                  className={`font-medium ${
                                    authResult.user_info.token_valid
                                      ? "text-green-600"
                                      : "text-orange-600"
                                  }`}
                                >
                                  {authResult.user_info.token_valid
                                    ? "JWT 格式"
                                    : "非标准格式"}
                                </p>
                              </div>
                              {authResult.user_info.api_status && (
                                <div className="p-3 rounded bg-gray-50">
                                  <p className="text-sm text-gray-600">
                                    API 状态码
                                  </p>
                                  <p className="font-medium text-gray-800">
                                    {authResult.user_info.api_status}
                                  </p>
                                </div>
                              )}
                            </div>
                            {authResult.user_info.checksum && (
                              <div className="p-3 rounded bg-gray-50">
                                <p className="mb-1 text-sm text-gray-600">
                                  生成的校验和
                                </p>
                                <p className="font-mono text-xs text-gray-700 break-all">
                                  {authResult.user_info.checksum}
                                </p>
                              </div>
                            )}

                            {authResult.user_info.account_info && (
                              <div className="mt-4">
                                <h4 className="mb-3 font-medium text-gray-700">
                                  账户信息:
                                </h4>
                                <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
                                  {authResult.user_info.account_info.email && (
                                    <div className="p-3 rounded bg-blue-50">
                                      <p className="text-sm text-gray-600">
                                        📧 邮箱
                                      </p>
                                      <p className="font-medium text-gray-800">
                                        {
                                          authResult.user_info.account_info
                                            .email
                                        }
                                      </p>
                                    </div>
                                  )}
                                  {authResult.user_info.account_info
                                    .username && (
                                    <div className="p-3 rounded bg-blue-50">
                                      <p className="text-sm text-gray-600">
                                        👤 用户名
                                      </p>
                                      <p className="font-medium text-gray-800">
                                        {
                                          authResult.user_info.account_info
                                            .username
                                        }
                                      </p>
                                    </div>
                                  )}
                                  {authResult.user_info.account_info
                                    .subscription_status && (
                                    <div className="p-3 rounded bg-green-50">
                                      <p className="text-sm text-gray-600">
                                        📊 订阅状态
                                      </p>
                                      <p className="font-medium text-green-700">
                                        {
                                          authResult.user_info.account_info
                                            .subscription_status
                                        }
                                      </p>
                                    </div>
                                  )}
                                  {authResult.user_info.account_info
                                    .subscription_type && (
                                    <div className="p-3 rounded bg-green-50">
                                      <p className="text-sm text-gray-600">
                                        💳 订阅类型
                                      </p>
                                      <p className="font-medium text-green-700">
                                        {
                                          authResult.user_info.account_info
                                            .subscription_type
                                        }
                                      </p>
                                    </div>
                                  )}
                                  {authResult.user_info.account_info
                                    .trial_days_remaining !== undefined && (
                                    <div className="p-3 rounded bg-yellow-50">
                                      <p className="text-sm text-gray-600">
                                        ⏰ 试用剩余天数
                                      </p>
                                      <p className="font-medium text-yellow-700">
                                        {
                                          authResult.user_info.account_info
                                            .trial_days_remaining
                                        }{" "}
                                        天
                                      </p>
                                    </div>
                                  )}
                                  {authResult.user_info.account_info
                                    .usage_info && (
                                    <div className="p-3 rounded bg-gray-50">
                                      <p className="text-sm text-gray-600">
                                        📈 使用信息
                                      </p>
                                      <p className="font-medium text-gray-800">
                                        {
                                          authResult.user_info.account_info
                                            .usage_info
                                        }
                                      </p>
                                    </div>
                                  )}
                                </div>
                              </div>
                            )}
                            {authResult.user_info.error_message && (
                              <div className="p-3 border border-red-200 rounded bg-red-50">
                                <p className="mb-1 text-sm text-red-600">
                                  错误信息
                                </p>
                                <p className="text-sm text-red-700">
                                  {authResult.user_info.error_message}
                                </p>
                              </div>
                            )}
                          </div>
                        </div>
                      )}

                      {authResult.details.length > 0 && (
                        <div className="mb-6">
                          <h3 className="mb-3 font-medium text-gray-700">
                            详细信息:
                          </h3>
                          <div className="space-y-2">
                            {authResult.details.map((detail, index) => (
                              <div
                                key={index}
                                className="p-3 text-sm bg-gray-100 rounded"
                              >
                                {detail}
                              </div>
                            ))}
                          </div>
                        </div>
                      )}

                      <button
                        type="button"
                        onClick={resetProcess}
                        className="px-6 py-2 text-white transition-colors bg-blue-500 rounded hover:bg-blue-600"
                      >
                        返回菜单
                      </button>
                    </>
                  );
                }

                // Handle restore/reset results
                const result = restoreResult || resetResult;
                const isRestore = !!restoreResult;
                const operationType = isRestore ? "恢复" : "重置";

                return (
                  <>
                    <h2 className="flex items-center mb-4 text-xl font-semibold">
                      {result!.success
                        ? "✅ " + operationType + "完成"
                        : "❌ " + operationType + "失败"}
                    </h2>

                    <div
                      className={`p-4 rounded-lg mb-6 ${
                        result!.success
                          ? "bg-green-50 border border-green-200"
                          : "bg-red-50 border border-red-200"
                      }`}
                    >
                      <p
                        className={`font-medium ${
                          result!.success ? "text-green-800" : "text-red-800"
                        }`}
                      >
                        {result!.message}
                      </p>
                    </div>

                    {result!.details.length > 0 && (
                      <div className="mb-6">
                        <h3 className="mb-3 font-medium text-gray-700">
                          详细信息:
                        </h3>
                        <div className="space-y-2">
                          {result!.details.map((detail, index) => (
                            <div
                              key={index}
                              className="p-3 text-sm bg-gray-100 rounded"
                            >
                              {detail}
                            </div>
                          ))}
                        </div>
                      </div>
                    )}

                    {resetResult?.new_ids && (
                      <div className="mb-6">
                        <h3 className="mb-3 font-medium text-gray-700">
                          新生成的机器ID:
                        </h3>
                        <div className="space-y-2">
                          {Object.entries(resetResult.new_ids).map(
                            ([key, value]) => (
                              <div
                                key={key}
                                className="py-2 pl-4 border-l-4 border-green-500"
                              >
                                <p className="font-mono text-sm text-gray-600">
                                  {key}
                                </p>
                                <p className="font-mono text-green-600 break-all">
                                  {value}
                                </p>
                              </div>
                            )
                          )}
                        </div>
                      </div>
                    )}

                    {result!.success && (
                      <div className="p-4 mb-6 border border-blue-200 rounded-lg bg-blue-50">
                        <h3 className="mb-2 font-medium text-blue-800">
                          下一步:
                        </h3>
                        <ul className="space-y-1 text-sm text-blue-700">
                          <li>• 关闭 Cursor 编辑器（如果正在运行）</li>
                          <li>• 重新启动 Cursor 编辑器</li>
                          <li>• 检查编辑器是否正常工作</li>
                        </ul>
                      </div>
                    )}

                    <button
                      type="button"
                      onClick={resetProcess}
                      className="px-6 py-2 text-white transition-colors bg-blue-500 rounded hover:bg-blue-600"
                    >
                      返回菜单
                    </button>
                  </>
                );
              })()}
            </div>
          )}

        {loading && currentStep !== "check" && (
          <div className="fixed inset-0 z-50 flex items-center justify-center bg-black bg-opacity-50">
            <div className="p-6 text-center bg-white rounded-lg">
              <div className="w-8 h-8 mx-auto mb-4 border-b-2 border-blue-500 rounded-full animate-spin"></div>
              <p className="text-gray-600">处理中...</p>
            </div>
          </div>
        )}
      </main>
    </div>
  );
}

export default App;
