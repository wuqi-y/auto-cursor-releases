import React, { useState, useEffect } from "react";
import { Button } from "./Button";
import { LoadingSpinner } from "./LoadingSpinner";
import {
  CursorBackupService,
  WorkspaceStorageItem,
  WorkspaceDetails,
} from "../services/cursorBackupService";

interface WorkspaceDetailsModalProps {
  isOpen: boolean;
  onClose: () => void;
}

export const WorkspaceDetailsModal: React.FC<WorkspaceDetailsModalProps> = ({
  isOpen,
  onClose,
}) => {
  const [workspaceItems, setWorkspaceItems] = useState<WorkspaceStorageItem[]>(
    []
  );
  const [selectedWorkspace, setSelectedWorkspace] = useState<string | null>(
    null
  );
  const [workspaceDetails, setWorkspaceDetails] =
    useState<WorkspaceDetails | null>(null);
  // 移除聊天记录详情相关状态
  const [loading, setLoading] = useState(false);
  const [detailsLoading, setDetailsLoading] = useState(false);
  // 移除聊天加载状态
  const [debugLoading, setDebugLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (isOpen) {
      loadWorkspaceItems();
    }
  }, [isOpen]);

  const loadWorkspaceItems = async () => {
    try {
      setLoading(true);
      setError(null);
      const items = await CursorBackupService.getWorkspaceStorageItems();
      setWorkspaceItems(items);
    } catch (error) {
      setError(`加载工作区列表失败: ${error}`);
    } finally {
      setLoading(false);
    }
  };

  const loadWorkspaceDetails = async (workspaceId: string) => {
    try {
      setDetailsLoading(true);
      setError(null);
      const details = await CursorBackupService.getWorkspaceDetails(
        workspaceId
      );
      console.log("🔍 前端接收到的工作区详情:", details);
      console.log("📋 对话数量:", details.conversations?.length || 0);
      if (details.conversations?.length > 0) {
        console.log("📋 第一个对话:", details.conversations[0]);
      }
      setWorkspaceDetails(details);
      setSelectedWorkspace(workspaceId);
    } catch (error) {
      console.error("❌ 加载工作区详情失败:", error);
      setError(`加载工作区详情失败: ${error}`);
    } finally {
      setDetailsLoading(false);
    }
  };

  const debugSqlite = async (workspaceId: string) => {
    try {
      setDebugLoading(true);
      setError(null);
      const result = await CursorBackupService.debugWorkspaceSqlite(
        workspaceId
      );
      alert(
        `调试完成！${result}\n请查看应用日志以获取详细的SQLite数据库内容。`
      );
    } catch (error) {
      setError(`调试SQLite失败: ${error}`);
    } finally {
      setDebugLoading(false);
    }
  };

  // 移除聊天详情加载函数

  // 移除返回功能

  const formatFileSize = CursorBackupService.formatFileSize;
  const formatDate = CursorBackupService.formatDate;

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black bg-opacity-50">
      <div className="flex flex-col w-full max-w-6xl bg-white rounded-lg shadow-xl h-5/6">
        {/* Header */}
        <div className="flex items-center justify-between p-6 border-b border-gray-200">
          <h2 className="text-2xl font-bold text-gray-900">对话记录详情</h2>
          <button
            onClick={onClose}
            className="p-2 text-gray-400 transition-colors hover:text-gray-600"
            title="关闭"
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
                d="M6 18L18 6M6 6l12 12"
              />
            </svg>
          </button>
        </div>

        {/* Content */}
        <div className="flex flex-1 overflow-hidden">
          {/* 左侧工作区列表 */}
          <div className="w-1/3 overflow-y-auto border-r border-gray-200 bg-gray-50">
            <div className="p-4">
              <div className="flex items-center justify-between mb-4">
                <h3 className="text-lg font-semibold text-gray-800">
                  工作区列表
                </h3>
                <button
                  onClick={loadWorkspaceItems}
                  disabled={loading}
                  className="p-2 text-blue-600 transition-colors hover:text-blue-800"
                  title="刷新列表"
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
                      d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15"
                    />
                  </svg>
                </button>
              </div>

              {loading ? (
                <div className="flex justify-center py-8">
                  <LoadingSpinner />
                </div>
              ) : error ? (
                <div className="p-4 text-sm text-red-600 rounded-lg bg-red-50">
                  {error}
                </div>
              ) : workspaceItems.length === 0 ? (
                <div className="py-8 text-center text-gray-500">
                  <svg
                    className="w-16 h-16 mx-auto mb-4 text-gray-300"
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
                  暂无工作区数据
                </div>
              ) : (
                <div className="space-y-2">
                  {workspaceItems.map((item) => (
                    <div
                      key={item.id}
                      onClick={() => loadWorkspaceDetails(item.id)}
                      className={`p-4 rounded-lg border cursor-pointer transition-all ${
                        selectedWorkspace === item.id
                          ? "border-blue-500 bg-blue-50"
                          : "border-gray-200 bg-white hover:border-gray-300 hover:bg-gray-50"
                      }`}
                    >
                      <div className="flex items-center justify-between mb-2">
                        <h4
                          className="flex-1 font-medium text-gray-900 truncate"
                          title={item.name}
                        >
                          {item.name}
                        </h4>
                        <div className="flex items-center gap-2">
                          <button
                            onClick={(e) => {
                              e.stopPropagation();
                              debugSqlite(item.id);
                            }}
                            disabled={debugLoading}
                            className="p-1 text-xs text-gray-500 transition-colors hover:text-blue-600 disabled:opacity-50"
                            title="调试SQLite数据库"
                          >
                            {debugLoading ? (
                              <div className="w-3 h-3 border border-gray-300 rounded-full animate-spin border-t-blue-600"></div>
                            ) : (
                              <svg
                                className="w-3 h-3"
                                fill="none"
                                stroke="currentColor"
                                viewBox="0 0 24 24"
                              >
                                <path
                                  strokeLinecap="round"
                                  strokeLinejoin="round"
                                  strokeWidth={2}
                                  d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10"
                                />
                              </svg>
                            )}
                          </button>
                          <div
                            className={`w-2 h-2 rounded-full ${
                              selectedWorkspace === item.id
                                ? "bg-blue-500"
                                : "bg-gray-300"
                            }`}
                          ></div>
                        </div>
                      </div>

                      {item.workspaceInfo && (
                        <p
                          className="mb-2 text-xs text-gray-600 truncate"
                          title={item.workspaceInfo.folder}
                        >
                          {item.workspaceInfo.folder}
                        </p>
                      )}

                      <div className="flex items-center justify-between text-xs text-gray-500">
                        <span>{item.conversationCount} 个对话</span>
                        <span>{formatFileSize(item.size)}</span>
                      </div>

                      <div className="mt-1 space-y-1 text-xs text-gray-400">
                        <div>修改: {formatDate(item.lastModified)}</div>
                        <div>创建: {formatDate(item.createdAt)}</div>
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </div>
          </div>

          {/* 右侧详情显示 */}
          <div className="flex-1 overflow-y-auto">
            {selectedWorkspace ? (
              <div className="p-6">
                {detailsLoading ? (
                  <div className="flex items-center justify-center h-64">
                    <LoadingSpinner />
                  </div>
                ) : workspaceDetails ? (
                  <div className="space-y-6">
                    {/* 工作区信息 */}
                    <div className="p-4 border border-blue-200 rounded-lg bg-blue-50">
                      <h3 className="mb-3 text-lg font-semibold text-blue-800">
                        工作区信息
                      </h3>
                      <div className="space-y-2">
                        <div className="flex items-start">
                          <span className="flex-shrink-0 w-20 font-medium text-blue-700">
                            项目路径:
                          </span>
                          <span className="text-blue-600 break-all">
                            {workspaceDetails.workspaceInfo.folder}
                          </span>
                        </div>
                        <div className="flex items-center">
                          <span className="flex-shrink-0 w-20 font-medium text-blue-700">
                            总大小:
                          </span>
                          <span className="text-blue-600">
                            {formatFileSize(workspaceDetails.totalSize)}
                          </span>
                        </div>
                        <div className="flex items-center">
                          <span className="flex-shrink-0 w-20 font-medium text-blue-700">
                            对话数量:
                          </span>
                          <span className="text-blue-600">
                            {workspaceDetails.conversations.length} 个
                          </span>
                        </div>
                      </div>
                    </div>

                    {/* 对话列表 */}
                    <div className="space-y-4">
                      <h3 className="text-lg font-semibold text-gray-800">
                        对话记录
                      </h3>

                      {workspaceDetails.conversations.length === 0 ? (
                        <div className="py-8 text-center text-gray-500">
                          <svg
                            className="w-16 h-16 mx-auto mb-4 text-gray-300"
                            fill="none"
                            stroke="currentColor"
                            viewBox="0 0 24 24"
                          >
                            <path
                              strokeLinecap="round"
                              strokeLinejoin="round"
                              strokeWidth={2}
                              d="M8 12h.01M12 12h.01M16 12h.01M21 12c0 4.418-4.03 8-9 8a9.863 9.863 0 01-4.255-.949L3 20l1.395-3.72C3.512 15.042 3 13.574 3 12c0-4.418 4.03-8 9-8s9 3.582 9 8z"
                            />
                          </svg>
                          暂无对话记录
                        </div>
                      ) : (
                        <div className="grid gap-4">
                          {workspaceDetails.conversations.map(
                            (conversation) => (
                              <div
                                key={conversation.id}
                                className="p-4 bg-white border border-gray-200 rounded-lg shadow-sm"
                              >
                                <div className="flex items-start justify-between mb-2">
                                  <h4 className="flex-1 font-medium text-gray-900">
                                    {conversation.title || "未命名对话"}
                                  </h4>
                                </div>

                                <div className="flex items-center justify-between text-xs text-gray-500">
                                  <span>
                                    创建时间:{" "}
                                    {formatDate(conversation.createdAt)}
                                  </span>
                                  <span>ID: {conversation.id}</span>
                                </div>
                              </div>
                            )
                          )}
                        </div>
                      )}
                    </div>
                  </div>
                ) : (
                  <div className="flex items-center justify-center h-64 text-gray-500">
                    加载详情失败
                  </div>
                )}
              </div>
            ) : (
              <div className="flex items-center justify-center h-full text-gray-500">
                <div className="text-center">
                  <svg
                    className="w-16 h-16 mx-auto mb-4 text-gray-300"
                    fill="none"
                    stroke="currentColor"
                    viewBox="0 0 24 24"
                  >
                    <path
                      strokeLinecap="round"
                      strokeLinejoin="round"
                      strokeWidth={2}
                      d="M9 5H7a2 2 0 00-2 2v10a2 2 0 002 2h8a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2"
                    />
                  </svg>
                  选择一个工作区查看详情
                </div>
              </div>
            )}
          </div>
        </div>

        {/* Footer */}
        <div className="flex justify-end p-6 border-t border-gray-200">
          <Button onClick={onClose} className="bg-gray-500 hover:bg-gray-600">
            关闭
          </Button>
        </div>
      </div>
    </div>
  );
};
