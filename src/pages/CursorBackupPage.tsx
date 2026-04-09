import React, { useState, useEffect } from "react";
import { Button } from "../components/Button";
import { LoadingSpinner } from "../components/LoadingSpinner";
import { Toast } from "../components/Toast";
import { WorkspaceDetailsModal } from "../components/WorkspaceDetailsModal";
import {
  CursorBackupService,
  BackupInfo,
  BackupProgress,
  BackupListItem,
  BackupType,
} from "../services/cursorBackupService";
import { listen } from "@tauri-apps/api/event";

export const CursorBackupPage: React.FC = () => {
  const [backupInfo, setBackupInfo] = useState<BackupInfo | null>(null);
  const [loading, setLoading] = useState(true);
  const [isBackingUp, setIsBackingUp] = useState(false);
  const [isRestoring, setIsRestoring] = useState(false);
  const [backupProgress, setBackupProgress] = useState<BackupProgress | null>(
    null
  );
  const [restoreProgress, setRestoreProgress] = useState<BackupProgress | null>(
    null
  );
  const [backupList, setBackupList] = useState<BackupListItem[]>([]);
  const [selectedBackup, setSelectedBackup] = useState<string | null>(null);
  const [currentBackupId, setCurrentBackupId] = useState<string | null>(null);
  const [deletingBackup, setDeletingBackup] = useState<string | null>(null);
  const [showDeleteConfirm, setShowDeleteConfirm] = useState<{
    backupName: string;
    backupDisplayName: string;
  } | null>(null);
  const [toast, setToast] = useState<{
    message: string;
    type: "success" | "error" | "info";
  } | null>(null);
  const [showWorkspaceDetails, setShowWorkspaceDetails] = useState(false);

  const [activeTab, setActiveTab] = useState<"backup" | "restore">("backup");

  useEffect(() => {
    loadCursorInfo();
    loadBackupList();

    // 监听备份进度事件
    const setupProgressListener = async () => {
      try {
        const progressUnlisten = await listen<BackupProgress>(
          "backup-progress",
          (event) => {
            const progress = event.payload;
            console.log("收到备份进度:", progress);
            setBackupProgress(progress);
          }
        );

        // 监听备份开始事件
        const startUnlisten = await listen<string>(
          "backup-started",
          (event) => {
            const backupId = event.payload;
            console.log("备份开始:", backupId);
            setCurrentBackupId(backupId);
          }
        );

        // 在组件卸载时清理监听器
        return () => {
          progressUnlisten();
          startUnlisten();
        };
      } catch (error) {
        console.error("设置监听器失败:", error);
      }
    };

    setupProgressListener();
  }, []);

  const loadCursorInfo = async () => {
    try {
      setLoading(true);
      const info = await CursorBackupService.getBackupInfo();
      setBackupInfo(info);
    } catch (error) {
      console.error("获取 Cursor 信息失败:", error);
      setToast({ message: `获取 Cursor 信息失败: ${error}`, type: "error" });
    } finally {
      setLoading(false);
    }
  };

  const loadBackupList = async () => {
    try {
      const list = await CursorBackupService.getBackupList();
      setBackupList(list);
    } catch (error) {
      console.error("获取备份列表失败:", error);
    }
  };

  const handleBackup = async (type: BackupType) => {
    try {
      setIsBackingUp(true);
      setBackupProgress({
        total: 100,
        current: 0,
        status: "准备备份...",
        percentage: 0,
      });

      // 开始备份并监听进度事件
      await CursorBackupService.backupData(type);

      setToast({ message: "备份完成！", type: "success" });
      await loadBackupList();
    } catch (error) {
      console.error("备份失败:", error);
      const errorMsg = error?.toString() || "未知错误";
      if (errorMsg.includes("已取消")) {
        setToast({ message: "备份已取消", type: "info" });
      } else {
        setToast({ message: `备份失败: ${errorMsg}`, type: "error" });
      }
    } finally {
      setIsBackingUp(false);
      setCurrentBackupId(null);
      // 延迟清空进度，让用户看到100%完成状态
      setTimeout(() => {
        setBackupProgress(null);
      }, 2000);
    }
  };

  const handleCancelBackup = async () => {
    if (!currentBackupId) return;

    try {
      await CursorBackupService.cancelBackup(currentBackupId);
      setToast({ message: "正在取消备份...", type: "info" });
    } catch (error) {
      console.error("取消备份失败:", error);
      setToast({ message: `取消备份失败: ${error}`, type: "error" });
    }
  };

  const handleRestore = async () => {
    if (!selectedBackup) return;

    try {
      setIsRestoring(true);
      setRestoreProgress({
        total: 100,
        current: 0,
        status: "准备恢复...",
        percentage: 0,
      });

      // 模拟恢复进度更新
      setTimeout(() => {
        setRestoreProgress({
          total: 100,
          current: 30,
          status: "正在恢复备份文件...",
          percentage: 30,
        });
      }, 500);

      setTimeout(() => {
        setRestoreProgress({
          total: 100,
          current: 60,
          status: "检查 Cursor 进程...",
          percentage: 60,
        });
      }, 1500);

      setTimeout(() => {
        setRestoreProgress({
          total: 100,
          current: 90,
          status: "重启 Cursor...",
          percentage: 90,
        });
      }, 3000);

      // 监听恢复进度事件
      const restoreResult = await CursorBackupService.restoreData(
        selectedBackup
      );

      setToast({
        message: restoreResult,
        type: "success",
      });
      await loadCursorInfo();
    } catch (error) {
      console.error("恢复失败:", error);
      setToast({ message: `恢复失败: ${error}`, type: "error" });
    } finally {
      setIsRestoring(false);
      setRestoreProgress(null);
    }
  };

  const handleDeleteBackup = async (backupName: string) => {
    try {
      setDeletingBackup(backupName);
      await CursorBackupService.deleteBackup(backupName);

      // 如果删除的是当前选中的备份，清除选择
      if (selectedBackup === backupName) {
        setSelectedBackup(null);
      }

      // 重新加载备份列表
      await loadBackupList();
      setToast({ message: "备份已删除", type: "success" });
    } catch (error) {
      console.error("删除备份失败:", error);
      setToast({ message: `删除失败: ${error}`, type: "error" });
    } finally {
      setDeletingBackup(null);
      setShowDeleteConfirm(null);
    }
  };

  const confirmDeleteBackup = (backup: BackupListItem) => {
    setShowDeleteConfirm({
      backupName: backup.name,
      backupDisplayName: backup.name,
    });
  };

  const cancelDelete = () => {
    setShowDeleteConfirm(null);
  };

  const handleOpenSettingsDir = async () => {
    try {
      const result = await CursorBackupService.openSettingsDir();
      setToast({ message: result, type: "success" });
    } catch (error) {
      console.error("打开设置目录失败:", error);
      setToast({ message: `打开设置目录失败: ${error}`, type: "error" });
    }
  };

  const handleOpenWorkspaceDir = async () => {
    try {
      const result = await CursorBackupService.openWorkspaceDir();
      setToast({ message: result, type: "success" });
    } catch (error) {
      console.error("打开工作区目录失败:", error);
      setToast({ message: `打开工作区目录失败: ${error}`, type: "error" });
    }
  };

  const handleOpenBackupDir = async () => {
    try {
      const result = await CursorBackupService.openBackupDir();
      setToast({ message: result, type: "success" });
    } catch (error) {
      console.error("打开备份目录失败:", error);
      setToast({ message: `打开备份目录失败: ${error}`, type: "error" });
    }
  };

  const formatFileSize = CursorBackupService.formatFileSize;
  const formatDate = CursorBackupService.formatDate;

  if (loading) {
    return (
      <div className="flex items-center justify-center h-64">
        <LoadingSpinner />
      </div>
    );
  }

  return (
    <div className="max-w-6xl p-6 mx-auto space-y-6">
      <div className="surface-primary rounded-xl border shadow-sm">
        <div className="border-b border-slate-200 p-6 dark:border-slate-800">
          <h1 className="flex items-center gap-3 text-2xl font-bold text-slate-900 dark:text-slate-100">
            <div className="flex items-center justify-center w-10 h-10 rounded-lg bg-gradient-to-br from-blue-500 to-purple-600">
              <svg
                className="w-6 h-6 text-white"
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
            </div>
            Cursor 数据备份
          </h1>
          <p className="text-subtle mt-2">
            备份和恢复您的 Cursor 设置和对话记录，确保数据安全
          </p>
        </div>

        {/* Tab Navigation */}
        <div className="flex border-b border-slate-200 dark:border-slate-800">
          <button
            onClick={() => setActiveTab("backup")}
            className={`px-6 py-4 text-sm font-medium border-b-2 transition-colors focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500/60 focus-visible:ring-offset-2 focus-visible:ring-offset-white dark:focus-visible:ring-offset-slate-950 ${
              activeTab === "backup"
                ? "border-blue-500 bg-blue-50 text-blue-700 dark:border-blue-400/60 dark:bg-blue-500/20 dark:text-blue-200"
                : "border-transparent bg-transparent text-slate-500 hover:bg-slate-50 hover:text-slate-700 dark:text-slate-400 dark:hover:bg-slate-900/50 dark:hover:text-slate-200"
            }`}
          >
            数据备份
          </button>
          <button
            onClick={() => setActiveTab("restore")}
            className={`px-6 py-4 text-sm font-medium border-b-2 transition-colors focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500/60 focus-visible:ring-offset-2 focus-visible:ring-offset-white dark:focus-visible:ring-offset-slate-950 ${
              activeTab === "restore"
                ? "border-blue-500 bg-blue-50 text-blue-700 dark:border-blue-400/60 dark:bg-blue-500/20 dark:text-blue-200"
                : "border-transparent bg-transparent text-slate-500 hover:bg-slate-50 hover:text-slate-700 dark:text-slate-400 dark:hover:bg-slate-900/50 dark:hover:text-slate-200"
            }`}
          >
            数据恢复
          </button>
        </div>

        <div className="p-6">
          {activeTab === "backup" && (
            <div className="space-y-6">
              {/* Cursor 信息展示 */}
              <div className="grid grid-cols-1 gap-6 md:grid-cols-2">
                {/* 设置信息 */}
                <div className="status-success rounded-lg p-6">
                  <div className="mb-4 flex items-center justify-between">
                    <h3 className="flex items-center gap-2 text-lg font-semibold text-green-800">
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
                      </svg>
                      Cursor 设置
                    </h3>
                    <div
                      className={`w-3 h-3 rounded-full ${
                        backupInfo?.cursor_settings.exists
                          ? "bg-green-400"
                          : "bg-red-400"
                      }`}
                    ></div>
                  </div>

                  {backupInfo?.cursor_settings.exists ? (
                    <div className="space-y-3">
                      <div className="text-sm text-green-700">
                        <div className="flex items-center justify-between font-medium">
                          <span>路径</span>
                          <button
                            onClick={handleOpenSettingsDir}
                            className="rounded p-1 text-green-700 transition-colors hover:bg-green-500/15 hover:text-green-900 dark:text-green-200 dark:hover:bg-green-500/20"
                            title="打开设置目录"
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
                                d="M10 6H6a2 2 0 00-2 2v10a2 2 0 002 2h10a2 2 0 002-2v-4M14 4h6m0 0v6m0-6L10 14"
                              />
                            </svg>
                          </button>
                        </div>
                        <div className="panel-code mt-1 break-all rounded px-2 py-1 text-xs text-green-100">
                          {backupInfo.cursor_settings.path}
                        </div>
                      </div>
                      {backupInfo.cursor_settings.size && (
                        <div className="text-sm text-green-700">
                          <span className="font-medium">大小: </span>
                          {formatFileSize(backupInfo.cursor_settings.size)}
                        </div>
                      )}
                      {backupInfo.cursor_settings.lastModified && (
                        <div className="text-sm text-green-700">
                          <span className="font-medium">修改时间: </span>
                          {formatDate(backupInfo.cursor_settings.lastModified)}
                        </div>
                      )}
                    </div>
                  ) : (
                    <p className="text-sm text-red-600">
                      未找到 Cursor 设置文件
                    </p>
                  )}
                </div>

                {/* 工作区数据 */}
                <div className="status-info rounded-lg p-6">
                  <div className="mb-4 flex items-center justify-between">
                    <h3 className="flex items-center gap-2 text-lg font-semibold text-blue-800">
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
                          d="M8 7v8a2 2 0 002 2h6M8 7V5a2 2 0 012-2h4.586a1 1 0 01.707.293l4.414 4.414a1 1 0 01.293.707V15a2 2 0 01-2 2h-2M8 7H6a2 2 0 00-2 2v10a2 2 0 002 2h8a2 2 0 002-2v-2"
                        />
                      </svg>
                      对话记录
                    </h3>
                    <div
                      className={`w-3 h-3 rounded-full ${
                        backupInfo?.workspace_storage.exists
                          ? "bg-blue-400"
                          : "bg-red-400"
                      }`}
                    ></div>
                  </div>

                  {backupInfo?.workspace_storage.exists ? (
                    <div className="space-y-3">
                      <div className="text-sm text-blue-700">
                        <div className="flex items-center justify-between font-medium">
                          <span>路径</span>
                          <div className="flex items-center gap-2">
                            <button
                              onClick={() => setShowWorkspaceDetails(true)}
                              className="rounded p-1 text-blue-700 transition-colors hover:bg-blue-500/15 hover:text-blue-900 dark:text-blue-200 dark:hover:bg-blue-500/20"
                              title="查看对话详情"
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
                                  d="M9 5H7a2 2 0 00-2 2v10a2 2 0 002 2h8a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2m-6 9l2 2 4-4"
                                />
                              </svg>
                            </button>
                            <button
                              onClick={handleOpenWorkspaceDir}
                              className="rounded p-1 text-blue-700 transition-colors hover:bg-blue-500/15 hover:text-blue-900 dark:text-blue-200 dark:hover:bg-blue-500/20"
                              title="打开对话目录"
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
                                  d="M10 6H6a2 2 0 00-2 2v10a2 2 0 002 2h10a2 2 0 002-2v-4M14 4h6m0 0v6m0-6L10 14"
                                />
                              </svg>
                            </button>
                          </div>
                        </div>
                        <div className="panel-code mt-1 break-all rounded px-2 py-1 text-xs text-blue-100">
                          {backupInfo.workspace_storage.path}
                        </div>
                      </div>
                      {backupInfo.workspace_storage.size && (
                        <div className="text-sm text-blue-700">
                          <span className="font-medium">大小: </span>
                          {formatFileSize(backupInfo.workspace_storage.size)}
                        </div>
                      )}
                      {backupInfo.workspace_storage.itemCount && (
                        <div className="text-sm text-blue-700">
                          <span className="font-medium">工作区数量: </span>
                          {backupInfo.workspace_storage.itemCount}
                        </div>
                      )}
                      {backupInfo.workspace_storage.lastModified && (
                        <div className="text-sm text-blue-700">
                          <span className="font-medium">修改时间: </span>
                          {formatDate(
                            backupInfo.workspace_storage.lastModified
                          )}
                        </div>
                      )}
                    </div>
                  ) : (
                    <p className="text-sm text-red-600">
                      未找到 Cursor 对话记录
                    </p>
                  )}
                </div>
              </div>

              {/* 备份进度 */}
              {isBackingUp && backupProgress && (
                <div className="status-info rounded-lg p-6">
                  <h3 className="mb-4 text-lg font-semibold text-blue-800">
                    备份进度
                  </h3>
                  <div className="space-y-3">
                    <div className="flex items-center justify-between">
                      <span className="text-blue-700">
                        {backupProgress.status}
                      </span>
                      <span className="font-medium text-blue-600">
                        {Math.round(backupProgress.percentage)}%
                      </span>
                    </div>
                    <div className="w-full h-2 bg-blue-200 rounded-full">
                      <div
                        className={`bg-blue-600 h-2 rounded-full transition-all duration-300 ease-out`}
                        style={{ width: `${backupProgress.percentage}%` }}
                      ></div>
                    </div>

                    {/* 取消备份按钮 */}
                    {currentBackupId && (
                      <div className="flex justify-center mt-4">
                        <Button
                          onClick={handleCancelBackup}
                          className="px-6 py-2 text-sm bg-gradient-to-r from-red-500 to-red-600 hover:from-red-600 hover:to-red-700"
                        >
                          <svg
                            className="w-4 h-4 mr-2"
                            fill="none"
                            stroke="currentColor"
                            viewBox="0 0 24 24"
                          >
                            <path
                              strokeLinecap="round"
                              strokeLinejoin="round"
                              strokeWidth={2}
                              d="M6 18L18 6M6 6l12 12"
                            />
                          </svg>
                          取消备份
                        </Button>
                      </div>
                    )}
                  </div>
                </div>
              )}

              {/* 备份操作按钮 */}
              <div className="grid grid-cols-1 gap-4 md:grid-cols-3">
                <Button
                  onClick={() => handleBackup("full")}
                  disabled={isBackingUp || !backupInfo}
                  className="flex items-center justify-center gap-2 bg-gradient-to-r from-blue-500 to-purple-600 hover:from-blue-600 hover:to-purple-700"
                >
                  {isBackingUp ? (
                    <LoadingSpinner size="sm" />
                  ) : (
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
                        d="M7 16a4 4 0 01-.88-7.903A5 5 0 1115.9 6L16 6a5 5 0 011 9.9M9 19l3 3m0 0l3-3m-3 3V10"
                      />
                    </svg>
                  )}
                  完整备份
                </Button>

                <Button
                  onClick={() => handleBackup("settings")}
                  disabled={isBackingUp || !backupInfo?.cursor_settings.exists}
                  className="flex items-center justify-center gap-2 bg-gradient-to-r from-green-500 to-emerald-600 hover:from-green-600 hover:to-emerald-700"
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
                      d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z"
                    />
                  </svg>
                  仅备份设置
                </Button>

                <Button
                  onClick={() => handleBackup("workspace")}
                  disabled={
                    isBackingUp || !backupInfo?.workspace_storage.exists
                  }
                  className="flex items-center justify-center gap-2 bg-gradient-to-r from-indigo-500 to-blue-600 hover:from-indigo-600 hover:to-blue-700"
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
                      d="M8 7v8a2 2 0 002 2h6M8 7V5a2 2 0 012-2h4.586a1 1 0 01.707.293l4.414 4.414a1 1 0 01.293.707V15a2 2 0 01-2 2h-2M8 7H6a2 2 0 00-2 2v10a2 2 0 002 2h8a2 2 0 002-2v-2"
                    />
                  </svg>
                  仅备份对话
                </Button>
              </div>
            </div>
          )}

          {activeTab === "restore" && (
            <div className="space-y-6">
              {/* 备份列表 */}
              <div className="surface-secondary rounded-lg p-6">
                <div className="flex items-center justify-between mb-4">
                  <h3 className="flex items-center gap-2 text-lg font-semibold text-slate-800 dark:text-slate-100">
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
                        d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2H5a2 2 0 00-2-2V7z"
                      />
                    </svg>
                    备份列表
                  </h3>
                  <button
                    onClick={handleOpenBackupDir}
                    className="surface-elevated inline-flex items-center gap-2 rounded-md border border-slate-200 px-3 py-1.5 text-xs font-medium text-slate-600 shadow-sm transition-all duration-150 hover:bg-slate-50 hover:text-slate-700 hover:border-slate-300 dark:border-slate-700 dark:text-slate-300 dark:hover:bg-slate-800 dark:hover:text-slate-100"
                    title="打开备份目录"
                  >
                    <svg
                      className="w-3.5 h-3.5"
                      fill="none"
                      stroke="currentColor"
                      viewBox="0 0 24 24"
                    >
                      <path
                        strokeLinecap="round"
                        strokeLinejoin="round"
                        strokeWidth={2}
                        d="M10 6H6a2 2 0 00-2 2v10a2 2 0 002 2h10a2 2 0 002-2v-4M14 4h6m0 0v6m0-6L10 14"
                      />
                    </svg>
                    <span>打开目录</span>
                  </button>
                </div>

                {backupList.length > 0 ? (
                  <div className="space-y-3">
                    {backupList.map((backup) => (
                      <div
                        key={backup.name}
                        className={`p-4 rounded-lg border-2 cursor-pointer transition-all duration-200 ${
                          selectedBackup === backup.name
                            ? "border-blue-500 bg-blue-50 dark:border-blue-500/40 dark:bg-blue-500/12"
                            : "surface-elevated border-slate-200 hover:border-slate-300 hover:bg-slate-50 dark:border-slate-700 dark:hover:border-slate-500 dark:hover:bg-slate-800"
                        }`}
                        onClick={() => setSelectedBackup(backup.name)}
                      >
                        <div className="flex items-center justify-between">
                          <div className="flex-1">
                            <div className="flex items-center gap-3">
                              <div
                                className={`w-3 h-3 rounded-full ${
                                  selectedBackup === backup.name
                                    ? "bg-blue-500"
                                    : "bg-slate-300 dark:bg-slate-600"
                                }`}
                              ></div>
                              <div>
                                <h4 className="font-medium text-slate-900 dark:text-slate-100">
                                  {backup.name}
                                </h4>
                                <div className="flex items-center gap-4 mt-1 text-sm text-slate-500 dark:text-slate-400">
                                  <span>{formatDate(backup.created_at)}</span>
                                  <span>{formatFileSize(backup.size)}</span>
                                  <span
                                    className={`px-2 py-1 rounded text-xs font-medium ${CursorBackupService.getBackupTypeColor(
                                      backup.type
                                    )}`}
                                  >
                                    {CursorBackupService.getBackupTypeName(
                                      backup.type
                                    )}
                                  </span>
                                </div>
                              </div>
                            </div>
                          </div>

                          {/* 删除按钮 */}
                          <button
                            onClick={(e) => {
                              e.stopPropagation(); // 阻止选择备份
                              confirmDeleteBackup(backup);
                            }}
                            disabled={deletingBackup === backup.name}
                            className="ml-3 rounded-lg p-2 text-slate-400 transition-colors duration-200 hover:bg-red-50 hover:text-red-600 disabled:cursor-not-allowed disabled:opacity-50 dark:text-slate-500 dark:hover:bg-red-500/15 dark:hover:text-red-300"
                            title="删除备份"
                          >
                            {deletingBackup === backup.name ? (
                              <div className="w-4 h-4 border-2 border-red-300 rounded-full animate-spin border-t-red-600"></div>
                            ) : (
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
                                  d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"
                                />
                              </svg>
                            )}
                          </button>
                        </div>
                      </div>
                    ))}
                  </div>
                ) : (
                  <div className="py-8 text-center text-slate-500 dark:text-slate-400">
                    <svg
                      className="mx-auto mb-4 h-16 w-16 text-slate-300 dark:text-slate-600"
                      fill="none"
                      stroke="currentColor"
                      viewBox="0 0 24 24"
                    >
                      <path
                        strokeLinecap="round"
                        strokeLinejoin="round"
                        strokeWidth={2}
                        d="M20 13V6a2 2 0 00-2-2H6a2 2 0 00-2 2v7m16 0v5a2 2 0 01-2 2H6a2 2 0 01-2-2v-5m16 0h-2.586a1 1 0 00-.707.293l-2.414 2.414a1 1 0 01-.707.293h-3.172a1 1 0 01-.707-.293l-2.414-2.414A1 1 0 006.586 13H4"
                      />
                    </svg>
                    暂无备份文件
                  </div>
                )}
              </div>

              {/* 恢复进度 */}
              {isRestoring && restoreProgress && (
                <div className="status-success rounded-lg p-6">
                  <h3 className="mb-4 text-lg font-semibold text-green-800">
                    恢复进度
                  </h3>
                  <div className="space-y-3">
                    <div className="flex items-center justify-between">
                      <span className="text-green-700">
                        {restoreProgress.status}
                      </span>
                      <span className="font-medium text-green-600">
                        {Math.round(restoreProgress.percentage)}%
                      </span>
                    </div>
                    <div className="w-full h-2 bg-green-200 rounded-full">
                      <div
                        className={`bg-green-600 h-2 rounded-full transition-all duration-300 ease-out`}
                        style={{ width: `${restoreProgress.percentage}%` }}
                      ></div>
                    </div>
                  </div>
                </div>
              )}

              {/* 恢复按钮 */}
              <div className="flex justify-center">
                <Button
                  onClick={handleRestore}
                  disabled={isRestoring || !selectedBackup}
                  className="flex items-center gap-2 px-8 py-3 bg-gradient-to-r from-green-500 to-emerald-600 hover:from-green-600 hover:to-emerald-700"
                >
                  {isRestoring ? (
                    <LoadingSpinner size="sm" />
                  ) : (
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
                        d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-8l-4-4m0 0L8 8m4-4v12"
                      />
                    </svg>
                  )}
                  恢复选中的备份
                </Button>
              </div>
            </div>
          )}
        </div>
      </div>

      {toast && (
        <Toast
          message={toast.message}
          type={toast.type}
          onClose={() => setToast(null)}
        />
      )}

      {/* 删除确认对话框 */}
      {showDeleteConfirm && (
        <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black bg-opacity-50">
          <div className="panel-floating w-full max-w-md rounded-lg p-6">
            <div className="flex items-center gap-3 mb-4">
              <div className="flex h-10 w-10 items-center justify-center rounded-full bg-red-100 text-red-600 dark:bg-red-500/15 dark:text-red-300">
                <svg
                  className="w-6 h-6 text-red-600"
                  fill="none"
                  stroke="currentColor"
                  viewBox="0 0 24 24"
                >
                  <path
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    strokeWidth={2}
                    d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-2.5L13.732 4c-.77-.833-1.964-.833-2.732 0L4.082 16.5c-.77.833.192 2.5 1.732 2.5z"
                  />
                </svg>
              </div>
              <div>
                <h3 className="text-lg font-semibold text-slate-900 dark:text-slate-100">
                  确认删除备份
                </h3>
                <p className="text-sm text-slate-500 dark:text-slate-400">此操作不可撤销</p>
              </div>
            </div>

            <p className="mb-6 text-slate-700 dark:text-slate-200">
              您确定要删除备份{" "}
              <span className="font-medium text-slate-900 dark:text-slate-100">
                "{showDeleteConfirm.backupDisplayName}"
              </span>{" "}
              吗？
              <br />
              <span className="text-red-600">删除后将无法恢复此备份文件。</span>
            </p>

            <div className="flex gap-3">
              <Button
                onClick={cancelDelete}
                className="surface-secondary flex-1 text-slate-800 dark:text-slate-100 hover:bg-slate-200/80 dark:hover:bg-slate-700/70"
              >
                取消
              </Button>
              <Button
                onClick={() => handleDeleteBackup(showDeleteConfirm.backupName)}
                disabled={deletingBackup === showDeleteConfirm.backupName}
                className="flex-1 bg-gradient-to-r from-red-500 to-red-600 hover:from-red-600 hover:to-red-700"
              >
                {deletingBackup === showDeleteConfirm.backupName ? (
                  <div className="flex items-center justify-center gap-2">
                    <div className="w-4 h-4 border-2 border-white rounded-full animate-spin border-t-transparent"></div>
                    删除中...
                  </div>
                ) : (
                  "确认删除"
                )}
              </Button>
            </div>
          </div>
        </div>
      )}

      {/* 工作区详情Modal */}
      <WorkspaceDetailsModal
        isOpen={showWorkspaceDetails}
        onClose={() => setShowWorkspaceDetails(false)}
      />
    </div>
  );
};
