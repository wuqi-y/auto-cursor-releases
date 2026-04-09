import React, { useState, useEffect } from "react";
import type {
  UserAnalyticsData,
  FilteredUsageEventsData,
} from "../types/analytics";
import { AnalyticsService } from "../services/analyticsService";

interface UsageDetailsModalProps {
  isOpen: boolean;
  onClose: () => void;
  token: string;
}

export const UsageDetailsModal: React.FC<UsageDetailsModalProps> = ({
  isOpen,
  onClose,
  token,
}) => {
  const [activeTab, setActiveTab] = useState<"analytics" | "events">("events");
  const [analyticsData, setAnalyticsData] = useState<UserAnalyticsData | null>(
    null
  );
  const [eventsData, setEventsData] = useState<FilteredUsageEventsData | null>(
    null
  );
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [currentPage, setCurrentPage] = useState(1);
  const pageSize = 20;

  // 获取最近30天的时间范围
  const getDateRange = () => {
    const endDate = new Date();
    const startDate = new Date().getTime() - 30 * 24 * 60 * 60 * 1000;
    // startDate.setDate(startDate.getDate() - 7);
    console.log("startDate", startDate);
    console.log("endDate", endDate);

    return {
      startDate: AnalyticsService.dateToTimestamp(new Date(startDate)),
      endDate: AnalyticsService.dateToTimestamp(endDate),
    };
  };

  // 加载数据
  const loadData = async () => {
    if (!isOpen) return;

    console.log(
      `🔄 Loading data - Tab: ${activeTab}, Page: ${currentPage}, PageSize: ${pageSize}`
    );

    setLoading(true);
    setError(null);

    try {
      const { startDate, endDate } = getDateRange();

      if (activeTab === "analytics") {
        const result = await AnalyticsService.getUserAnalytics(
          token,
          0, // teamId
          0, // userId
          startDate,
          endDate
        );

        if (result.success && result.data) {
          setAnalyticsData(result.data);
        } else {
          setError(result.message);
        }
      } else {
        const result = await AnalyticsService.getUsageEvents(
          token,
          0, // teamId
          startDate,
          endDate,
          currentPage,
          pageSize
        );

        console.log(`📊 Usage events result:`, result);

        if (result.success && result.data) {
          console.log(`✅ Events data loaded successfully:`, result.data);
          setEventsData(result.data);
        } else {
          console.error(`❌ Events data loading failed:`, result.message);
          setError(result.message);
        }
      }
    } catch (err) {
      setError(`加载数据失败: ${err}`);
    } finally {
      setLoading(false);
    }
  };

  // 当模态框打开或标签页切换时加载数据
  useEffect(() => {
    loadData();
  }, [isOpen, activeTab, currentPage]);

  // 格式化时间戳
  const formatTimestamp = (timestamp: string | null | undefined) => {
    if (!timestamp) {
      return "-";
    }
    try {
      const date = AnalyticsService.timestampToDate(timestamp);
      if (isNaN(date.getTime())) {
        return "-";
      }
      return date.toLocaleString("zh-CN");
    } catch (error) {
      console.warn("Invalid timestamp:", timestamp);
      return "-";
    }
  };

  // 格式化日期（仅日期部分）
  const formatDate = (timestamp: string | null | undefined) => {
    if (!timestamp) {
      return "-";
    }
    try {
      const date = AnalyticsService.timestampToDate(timestamp);
      if (isNaN(date.getTime())) {
        return "-";
      }
      return date.toLocaleDateString("zh-CN");
    } catch (error) {
      console.warn("Invalid date timestamp:", timestamp);
      return "-";
    }
  };

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-50 overflow-y-auto bg-black/50 backdrop-blur-sm">
      <div className="flex min-h-screen items-center justify-center px-4 py-8 text-center">
        {/* Modal */}
        <div className="panel-floating inline-block w-full max-w-6xl rounded-xl p-6 text-left align-middle shadow-xl transition-all transform">
          {/* Header */}
          <div className="mb-4 flex items-center justify-between">
            <h3 className="text-lg font-medium text-slate-900 dark:text-slate-100">
              📊 使用详情 (最近30天)
            </h3>
            <button
              onClick={onClose}
              className="text-slate-400 hover:text-slate-600 focus:outline-none dark:text-slate-500 dark:hover:text-slate-300"
            >
              ✕
            </button>
          </div>

          {/* Tab Navigation */}
          <div className="mb-4 border-b border-slate-200 dark:border-slate-800">
            <nav className="flex -mb-px space-x-8">
              <button
                onClick={() => setActiveTab("events")}
                className={`py-2 px-1 border-b-2 font-medium text-sm ${
                  activeTab === "events"
                    ? "border-blue-500 text-blue-600 dark:text-blue-300"
                    : "border-transparent text-slate-500 hover:text-slate-700 hover:border-slate-300 dark:text-slate-400 dark:hover:text-slate-200 dark:hover:border-slate-700"
                }`}
              >
                🔍 使用事件明细
              </button>
              <button
                onClick={() => setActiveTab("analytics")}
                className={`py-2 px-1 border-b-2 font-medium text-sm ${
                  activeTab === "analytics"
                    ? "border-blue-500 text-blue-600 dark:text-blue-300"
                    : "border-transparent text-slate-500 hover:text-slate-700 hover:border-slate-300 dark:text-slate-400 dark:hover:text-slate-200 dark:hover:border-slate-700"
                }`}
              >
                📈 用户分析数据
              </button>
            </nav>
          </div>

          {/* Content */}
          <div className="overflow-y-auto max-h-96">
            {loading ? (
              <div className="flex items-center justify-center py-8">
                <div className="inline-flex items-center">
                  <svg
                    className="w-4 h-4 mr-2 animate-spin"
                    viewBox="0 0 24 24"
                  >
                    <circle
                      className="opacity-25"
                      cx="12"
                      cy="12"
                      r="10"
                      stroke="currentColor"
                      strokeWidth="4"
                      fill="none"
                    />
                    <path
                      className="opacity-75"
                      fill="currentColor"
                      d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
                    />
                  </svg>
                  <span className="text-sm text-slate-500 dark:text-slate-400">加载数据中...</span>
                </div>
              </div>
            ) : error ? (
              <div className="status-error rounded-md p-4">
                <p className="text-sm text-red-600 dark:text-red-300">❌ {error}</p>
                <div className="mt-2 text-xs text-slate-600 dark:text-slate-300">
                  当前页码: {currentPage}, 页大小: {pageSize}
                  {eventsData && (
                    <span>, 总记录数: {eventsData.totalUsageEventsCount}</span>
                  )}
                </div>
                <button
                  onClick={loadData}
                  className="mt-2 text-sm text-red-700 underline hover:text-red-800"
                >
                  重试
                </button>
              </div>
            ) : activeTab === "events" && eventsData ? (
              <div>
                {/* Events Table */}
                <div className="overflow-x-auto">
                  <table className="min-w-full divide-y divide-slate-200 dark:divide-slate-800">
                    <thead className="bg-slate-50 dark:bg-slate-900/60">
                      <tr>
                        <th className="px-3 py-2 text-left text-xs font-medium uppercase tracking-wider text-slate-500 dark:text-slate-400">
                          时间
                        </th>
                        <th className="px-3 py-2 text-left text-xs font-medium uppercase tracking-wider text-slate-500 dark:text-slate-400">
                          模型
                        </th>
                        <th className="px-3 py-2 text-left text-xs font-medium uppercase tracking-wider text-slate-500 dark:text-slate-400">
                          类型
                        </th>
                        <th className="px-3 py-2 text-left text-xs font-medium uppercase tracking-wider text-slate-500 dark:text-slate-400">
                          Token用量
                        </th>
                        <th className="px-3 py-2 text-left text-xs font-medium uppercase tracking-wider text-slate-500 dark:text-slate-400">
                          费用
                        </th>
                      </tr>
                    </thead>
                    <tbody className="divide-y divide-slate-200 bg-white/60 dark:divide-slate-800 dark:bg-slate-950/30">
                      {(eventsData?.usageEventsDisplay || []).map(
                        (event, index) => (
                          <tr key={index} className="hover:bg-slate-50 dark:hover:bg-slate-900/60">
                            <td className="px-3 py-2 text-sm text-slate-900 dark:text-slate-100">
                              {formatTimestamp(event.timestamp)}
                            </td>
                            <td className="px-3 py-2 text-sm text-slate-900 dark:text-slate-100">
                              <span className="inline-flex rounded-full bg-blue-100 px-2 py-1 text-xs font-medium text-blue-800 dark:bg-blue-500/15 dark:text-blue-200">
                                {AnalyticsService.getModelDisplayName(
                                  event.model
                                )}
                              </span>
                            </td>
                            <td className="px-3 py-2 text-sm text-slate-900 dark:text-slate-100">
                              <span
                                className={`inline-flex rounded-full px-2 py-1 text-xs font-medium ${
                                  event.kind.includes("INCLUDED_IN_PRO")
                                    ? "bg-green-100 text-green-800 dark:bg-green-500/15 dark:text-green-200"
                                    : event.kind.includes("ERRORED")
                                    ? "bg-red-100 text-red-800 dark:bg-red-500/15 dark:text-red-200"
                                    : "bg-yellow-100 text-yellow-800 dark:bg-yellow-500/15 dark:text-yellow-200"
                                }`}
                              >
                                {AnalyticsService.getEventKindDisplay(
                                  event.kind
                                )}
                              </span>
                            </td>
                            <td className="px-3 py-2 text-sm text-slate-900 dark:text-slate-100">
                              {event.tokenUsage ? (
                                <div className="space-y-1">
                                  <div>
                                    输入:{" "}
                                    {AnalyticsService.formatNumber(
                                      event.tokenUsage.inputTokens
                                    )}
                                  </div>
                                  <div>
                                    输出:{" "}
                                    {AnalyticsService.formatNumber(
                                      event.tokenUsage.outputTokens
                                    )}
                                  </div>
                                  <div className="text-xs text-slate-500 dark:text-slate-400">
                                    缓存:{" "}
                                    {AnalyticsService.formatNumber(
                                      event.tokenUsage.cacheReadTokens
                                    )}
                                  </div>
                                </div>
                              ) : (
                                <span className="text-slate-400 dark:text-slate-500">-</span>
                              )}
                            </td>
                            <td className="px-3 py-2 text-sm text-slate-900 dark:text-slate-100">
                              {event.tokenUsage?.totalCents !== undefined ? (
                                <span className="font-medium">
                                  {AnalyticsService.formatCents(
                                    event.tokenUsage.totalCents
                                  )}
                                </span>
                              ) : (
                                <span className="text-slate-400 dark:text-slate-500">
                                  {event.usageBasedCosts || "-"}
                                </span>
                              )}
                            </td>
                          </tr>
                        )
                      )}
                    </tbody>
                  </table>
                </div>

                {/* Pagination */}
                {eventsData && (
                  <div className="flex items-center justify-between px-2 mt-4">
                    <div className="text-sm text-slate-700 dark:text-slate-300">
                      显示 {(currentPage - 1) * pageSize + 1} -{" "}
                      {Math.min(
                        currentPage * pageSize,
                        eventsData.totalUsageEventsCount
                      )}
                      ，共 {eventsData.totalUsageEventsCount} 条记录
                    </div>
                    <div className="flex space-x-2">
                      <button
                        onClick={() =>
                          setCurrentPage(Math.max(1, currentPage - 1))
                        }
                        disabled={currentPage === 1}
                        className="surface-secondary rounded border border-subtle px-3 py-1 text-sm text-slate-700 hover:bg-slate-200/80 disabled:cursor-not-allowed disabled:opacity-50 dark:text-slate-200 dark:hover:bg-slate-700/70"
                      >
                        上一页
                      </button>
                      <span className="px-3 py-1 text-sm">
                        第 {currentPage} 页
                      </span>
                      <button
                        onClick={() => {
                          const nextPage = currentPage + 1;
                          const maxPage = Math.ceil(
                            eventsData.totalUsageEventsCount / pageSize
                          );
                          console.log(
                            `📄 Next page click: ${nextPage}, Max page: ${maxPage}`
                          );
                          if (nextPage <= maxPage) {
                            setCurrentPage(nextPage);
                          }
                        }}
                        disabled={
                          currentPage >=
                          Math.ceil(eventsData.totalUsageEventsCount / pageSize)
                        }
                        className="surface-secondary rounded border border-subtle px-3 py-1 text-sm text-slate-700 hover:bg-slate-200/80 disabled:cursor-not-allowed disabled:opacity-50 dark:text-slate-200 dark:hover:bg-slate-700/70"
                      >
                        下一页
                      </button>
                    </div>
                  </div>
                )}
              </div>
            ) : activeTab === "analytics" && analyticsData ? (
              <div>
                {/* Analytics Summary */}
                <div className="surface-secondary mb-4 rounded-lg p-4">
                  <h4 className="mb-2 font-medium text-slate-900 dark:text-slate-100">
                    📊 总览信息
                  </h4>
                  <div className="grid grid-cols-2 gap-4 text-sm">
                    <div>
                      <span className="text-slate-600 dark:text-slate-300">时间范围:</span>
                      <span className="ml-2 font-medium">
                        {analyticsData?.period
                          ? `${formatDate(
                              analyticsData.period.startDate
                            )} - ${formatDate(analyticsData.period.endDate)}`
                          : "-"}
                      </span>
                    </div>
                    <div>
                      <span className="text-slate-600 dark:text-slate-300">团队成员数:</span>
                      <span className="ml-2 font-medium">
                        {analyticsData?.totalMembersInTeam || 1}
                      </span>
                    </div>
                  </div>
                </div>

                {/* Daily Metrics */}
                <div className="space-y-4">
                  <h4 className="font-medium text-slate-900 dark:text-slate-100">📈 每日指标</h4>
                  <div className="overflow-x-auto">
                    <table className="min-w-full divide-y divide-slate-200 dark:divide-slate-800">
                      <thead className="bg-slate-50 dark:bg-slate-900/60">
                        <tr>
                          <th className="px-3 py-2 text-left text-xs font-medium uppercase text-slate-500 dark:text-slate-400">
                            日期
                          </th>
                          <th className="px-3 py-2 text-left text-xs font-medium uppercase text-slate-500 dark:text-slate-400">
                            活跃用户
                          </th>
                          <th className="px-3 py-2 text-left text-xs font-medium uppercase text-slate-500 dark:text-slate-400">
                            代码接受
                          </th>
                          <th className="px-3 py-2 text-left text-xs font-medium uppercase text-slate-500 dark:text-slate-400">
                            请求数
                          </th>
                          <th className="px-3 py-2 text-left text-xs font-medium uppercase text-slate-500 dark:text-slate-400">
                            模型使用
                          </th>
                        </tr>
                      </thead>
                      <tbody className="divide-y divide-slate-200 bg-white/60 dark:divide-slate-800 dark:bg-slate-950/30">
                        {analyticsData?.dailyMetrics?.map((metric, index) => (
                          <tr key={index} className="hover:bg-slate-50 dark:hover:bg-slate-900/60">
                            <td className="px-3 py-2 text-sm text-slate-900 dark:text-slate-100">
                              {formatDate(metric.date)}
                            </td>
                            <td className="px-3 py-2 text-sm text-slate-900 dark:text-slate-100">
                              {metric.activeUsers ?? 0}
                            </td>
                            <td className="px-3 py-2 text-sm text-slate-900 dark:text-slate-100">
                              <div className="space-y-1">
                                {metric.acceptedLinesAdded && (
                                  <div className="text-green-600">
                                    +
                                    {AnalyticsService.formatNumber(
                                      metric.acceptedLinesAdded
                                    )}{" "}
                                    行
                                  </div>
                                )}
                                {metric.acceptedLinesDeleted && (
                                  <div className="text-red-600">
                                    -
                                    {AnalyticsService.formatNumber(
                                      metric.acceptedLinesDeleted
                                    )}{" "}
                                    行
                                  </div>
                                )}
                                {metric.totalAccepts && (
                                  <div className="text-xs text-slate-500 dark:text-slate-400">
                                    接受率: {metric.totalAccepts}/
                                    {metric.totalApplies || 0}
                                  </div>
                                )}
                              </div>
                            </td>
                            <td className="px-3 py-2 text-sm text-slate-900 dark:text-slate-100">
                              <div className="space-y-1">
                                {metric.composerRequests && (
                                  <div>
                                    编写:{" "}
                                    {AnalyticsService.formatNumber(
                                      metric.composerRequests
                                    )}
                                  </div>
                                )}
                                {metric.agentRequests && (
                                  <div>
                                    助手:{" "}
                                    {AnalyticsService.formatNumber(
                                      metric.agentRequests
                                    )}
                                  </div>
                                )}
                                {metric.subscriptionIncludedReqs && (
                                  <div className="text-xs text-slate-500 dark:text-slate-400">
                                    订阅:{" "}
                                    {AnalyticsService.formatNumber(
                                      metric.subscriptionIncludedReqs
                                    )}
                                  </div>
                                )}
                              </div>
                            </td>
                            <td className="px-3 py-2 text-sm text-slate-900 dark:text-slate-100">
                              {metric.modelUsage &&
                              metric.modelUsage.length > 0 ? (
                                <div className="space-y-1">
                                  {metric.modelUsage.map((model, idx) => (
                                    <div
                                      key={idx}
                                      className="flex items-center space-x-2"
                                    >
                                      <span className="inline-flex rounded bg-blue-100 px-2 py-1 text-xs text-blue-800 dark:bg-blue-500/15 dark:text-blue-200">
                                        {AnalyticsService.getModelDisplayName(
                                          model.name
                                        )}
                                      </span>
                                      <span className="text-xs text-slate-500 dark:text-slate-400">
                                        {model.count}次
                                      </span>
                                    </div>
                                  ))}
                                </div>
                              ) : (
                                "-"
                              )}
                            </td>
                          </tr>
                        )) || []}
                      </tbody>
                    </table>
                  </div>
                </div>
              </div>
            ) : (
              <div className="py-8 text-center text-slate-500 dark:text-slate-400">暂无数据</div>
            )}
          </div>

          {/* Footer */}
          <div className="flex justify-end mt-6">
            <button
              onClick={onClose}
              className="surface-secondary rounded-md border border-transparent px-4 py-2 text-sm font-medium text-slate-700 hover:bg-slate-200/80 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-gray-500 dark:text-slate-200 dark:hover:bg-slate-700/70"
            >
              关闭
            </button>
          </div>
        </div>
      </div>
    </div>
  );
};
