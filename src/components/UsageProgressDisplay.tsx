import React, { useState, useEffect } from "react";
import { CursorService } from "../services/cursorService";
import { LoadingSpinner } from "./LoadingSpinner";

interface UsageProgressDisplayProps {
  token: string;
  onShowToast: (message: string, type: "success" | "error") => void;
}

export const UsageProgressDisplay: React.FC<UsageProgressDisplayProps> = ({
  token,
  onShowToast,
}) => {
  const [loading, setLoading] = useState(false);
  const [progressData, setProgressData] = useState<any>(null);
  const [displayProgress, setDisplayProgress] = useState(0);

  useEffect(() => {
    if (token) {
      fetchUsageProgress();
    }
  }, [token]);

  // Trigger progress bar animation when data loads
  useEffect(() => {
    if (
      progressData?.parsed_data?.parsed?.usageProgressPercentage !== undefined
    ) {
      setDisplayProgress(0);
      requestAnimationFrame(() => {
        requestAnimationFrame(() => {
          setDisplayProgress(
            progressData.parsed_data.parsed.usageProgressPercentage
          );
        });
      });
    }
  }, [progressData]);

  const fetchUsageProgress = async () => {
    setLoading(true);
    // 不重置 displayProgress，让动画自然过渡
    try {
      const result = await CursorService.getCurrentPeriodUsage(token);
      console.log("📊 使用进度数据:", result);
      setProgressData(result);
    } catch (error: any) {
      console.error("❌ 获取使用进度失败:", error);
      onShowToast(`获取使用进度失败: ${error}`, "error");
    } finally {
      setLoading(false);
    }
  };

  const parsed = progressData?.parsed_data?.parsed;
  const progressMessage = parsed?.usageProgressMessage || "未知";
  const progressPercentage = parsed?.usageProgressPercentage || 0;

  // 修正 spendLimit 数据：如果 individualLimit 不合理，根据进度百分比计算
  let spendLimit = parsed?.spendLimitUsage;
  if (spendLimit && progressPercentage > 0) {
    const individualUsed = spendLimit.individualUsed || 0;
    const individualLimit = spendLimit.individualLimit || 0;

    // 如果限额小于等于已使用量，说明解析错误，需要计算真实限额
    if (individualLimit <= individualUsed) {
      const calculatedLimit = Math.round(
        individualUsed / (progressPercentage / 100)
      );
      console.log(
        `📊 限额修正: 原始=${individualLimit} cents, 计算=${calculatedLimit} cents (已使用=${individualUsed}, 进度=${progressPercentage}%)`
      );

      spendLimit = {
        ...spendLimit,
        individualLimit: calculatedLimit,
        individualLimitDollars: calculatedLimit / 100,
      };
    }
  }

  const planUsage = parsed?.planUsage;
  const enabled = parsed?.enabled;
  const displayThreshold = parsed?.displayThreshold;
  const billingCycleStart = parsed?.billingCycleStart;
  const billingCycleEnd = parsed?.billingCycleEnd;

  const getProgressColor = (progress: number) => {
    if (progress >= 100) return "bg-red-500";
    if (progress >= 80) return "bg-orange-500";
    if (progress >= 50) return "bg-yellow-500";
    return "bg-green-500";
  };

  const formatDate = (timestamp: number) => {
    if (!timestamp) return "未知";
    const date = new Date(Number(timestamp));
    return date.toLocaleString("zh-CN");
  };

  return (
    <div className="surface-primary rounded-lg shadow">
      <div className="px-4 py-5 sm:p-6">
        <div className="flex items-center justify-between mb-4">
          <h3 className="text-lg font-medium leading-6 text-slate-900 dark:text-slate-100">
            📊 使用进度详情
          </h3>
          <button
            onClick={fetchUsageProgress}
            disabled={loading}
            className="inline-flex items-center rounded border border-transparent bg-blue-100 px-3 py-1 text-sm font-medium text-blue-700 hover:bg-blue-200 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-blue-500 disabled:opacity-50 dark:bg-blue-500/15 dark:text-blue-200 dark:hover:bg-blue-500/25"
          >
            {loading ? "🔄 刷新中..." : "🔄 刷新"}
          </button>
        </div>

        {/* 只在首次加载（无数据）时显示 loading spinner，刷新时保持数据显示 */}
        {loading && !progressData ? (
          <div className="flex items-center justify-center py-12">
            <LoadingSpinner />
          </div>
        ) : progressData ? (
          <div
            className={`space-y-6 transition-opacity duration-200 ${
              loading ? "opacity-60 pointer-events-none" : "opacity-100"
            }`}
          >
            {/* 进度条部分 */}
            <div className="surface-secondary rounded-lg p-6 shadow-sm">
              <div className="mb-4">
                <div className="flex items-center justify-between mb-2">
                  <span className="text-lg font-semibold text-slate-700 dark:text-slate-200">
                    使用进度
                  </span>
                  <span
                    className={`text-2xl font-bold ${
                      displayProgress >= 100
                        ? "text-red-600"
                        : displayProgress >= 80
                        ? "text-orange-600"
                        : "text-green-600"
                    }`}
                  >
                    {displayProgress}%
                  </span>
                </div>

                {/* 进度条 */}
                <div className="relative h-8 overflow-hidden rounded-full bg-slate-200 dark:bg-slate-800">
                  <div
                    className={`h-full ${getProgressColor(
                      displayProgress
                    )} flex items-center justify-end pr-3 transition-all duration-1000 ease-out`}
                    style={{ width: `${displayProgress}%` }}
                  >
                    {displayProgress > 10 && (
                      <span className="text-sm font-medium text-white">
                        {displayProgress}%
                      </span>
                    )}
                  </div>
                </div>

                {/* 进度消息 */}
                <p className="mt-3 text-center text-sm italic text-slate-600 dark:text-slate-300">
                  "{progressMessage}"
                </p>
              </div>

              {/* 状态指示 */}
              <div className="flex items-center justify-center gap-2 mt-4">
                <span
                  className={`rounded-full px-3 py-1 text-sm font-medium ${
                    enabled
                      ? "bg-green-100 text-green-800 dark:bg-green-500/15 dark:text-green-200"
                      : "bg-red-100 text-red-800 dark:bg-red-500/15 dark:text-red-200"
                  }`}
                >
                  {enabled ? "✅ 已启用" : "❌ 未启用"}
                </span>
                {displayThreshold && (
                  <span className="rounded-full bg-blue-100 px-3 py-1 text-sm font-medium text-blue-800 dark:bg-blue-500/15 dark:text-blue-200">
                    显示阈值: {displayThreshold}%
                  </span>
                )}
              </div>
            </div>

            {/* 个人限额使用情况 */}
            {spendLimit && (
              <div className="surface-elevated rounded-lg p-6 shadow-sm">
                <h3 className="mb-4 text-lg font-semibold text-slate-900 dark:text-slate-100">
                  💳 个人限额使用
                </h3>
                <div className="grid grid-cols-2 gap-4">
                  <div className="p-4 rounded-lg bg-blue-50">
                    <p className="text-sm text-slate-600 dark:text-slate-300">已使用</p>
                    <p className="text-2xl font-bold text-blue-600">
                      ${spendLimit.individualUsedDollars?.toFixed(2) || "0.00"}
                    </p>
                    <p className="text-xs text-slate-500 dark:text-slate-400">
                      {spendLimit.individualUsed || 0} cents
                    </p>
                  </div>
                  <div className="p-4 rounded-lg bg-purple-50">
                    <p className="text-sm text-slate-600 dark:text-slate-300">剩余额度</p>
                    <p className="text-2xl font-bold text-purple-600">
                      ${spendLimit.individualLimitDollars?.toFixed(2) || "0.00"}
                    </p>
                    <p className="text-xs text-slate-500 dark:text-slate-400">
                      {spendLimit.individualLimit || 0} cents
                    </p>
                  </div>
                </div>
              </div>
            )}

            {/* 计划使用情况 */}
            {planUsage && (
              <div className="surface-elevated rounded-lg p-6 shadow-sm">
                <h3 className="mb-4 text-lg font-semibold text-slate-900 dark:text-slate-100">
                  📈 计划使用情况
                </h3>
                <div className="grid grid-cols-2 gap-4 mb-4">
                  <div className="p-4 rounded-lg bg-orange-50">
                    <p className="text-sm text-slate-600 dark:text-slate-300">总花费</p>
                    <p className="text-2xl font-bold text-orange-600">
                      ${planUsage.totalSpendDollars?.toFixed(2) || "0.00"}
                    </p>
                    <p className="text-xs text-slate-500 dark:text-slate-400">
                      {planUsage.totalSpend || 0} cents
                    </p>
                  </div>
                  <div className="p-4 rounded-lg bg-green-50">
                    <p className="text-sm text-slate-600 dark:text-slate-300">已用额度</p>
                    <p className="text-2xl font-bold text-green-600">
                      ${planUsage.includedSpendDollars?.toFixed(2) || "0.00"}
                    </p>
                    <p className="text-xs text-slate-500 dark:text-slate-400">
                      {planUsage.includedSpend || 0} cents
                    </p>
                  </div>
                  <div className="p-4 rounded-lg bg-yellow-50">
                    <p className="text-sm text-slate-600 dark:text-slate-300">奖励额度</p>
                    <p className="text-2xl font-bold text-yellow-600">
                      ${planUsage.bonusSpendDollars?.toFixed(2) || "0.00"}
                    </p>
                    <p className="text-xs text-slate-500 dark:text-slate-400">
                      {planUsage.bonusSpend || 0} cents
                    </p>
                  </div>
                  <div className="p-4 rounded-lg bg-indigo-50">
                    <p className="text-sm text-slate-600 dark:text-slate-300">剩余额度</p>
                    <p className="text-2xl font-bold text-indigo-600">
                      ${planUsage.limitDollars?.toFixed(2) || "0.00"}
                    </p>
                    <p className="text-xs text-slate-500 dark:text-slate-400">
                      {planUsage.limit || 0} cents
                    </p>
                  </div>
                </div>

                {planUsage.remainingBonus !== undefined && (
                  <div className="flex items-center gap-2 p-3 rounded bg-blue-50">
                    <span className="text-sm font-medium text-blue-800">
                      {planUsage.remainingBonus
                        ? "🎁 有剩余奖励额度"
                        : "📭 无剩余奖励额度"}
                    </span>
                  </div>
                )}

                {planUsage.bonusTooltip && (
                  <div className="surface-secondary mt-3 rounded p-3">
                    <p className="text-sm text-slate-600 dark:text-slate-300">
                      💡 {planUsage.bonusTooltip}
                    </p>
                  </div>
                )}
              </div>
            )}

            {/* 账单周期 */}
            {(billingCycleStart || billingCycleEnd) && (
              <div className="p-4 rounded-lg bg-gradient-to-r from-purple-50 to-blue-50">
                <h4 className="mb-3 text-sm font-semibold text-slate-700 dark:text-slate-200">
                  📅 账单周期
                </h4>
                <div className="space-y-2">
                  {billingCycleStart && (
                    <div className="flex items-center gap-2">
                      <span className="text-sm text-slate-600 dark:text-slate-300">开始时间:</span>
                      <span className="text-sm font-semibold text-purple-600">
                        {formatDate(billingCycleStart)}
                      </span>
                    </div>
                  )}
                  {billingCycleEnd && (
                    <div className="flex items-center gap-2">
                      <span className="text-sm text-slate-600 dark:text-slate-300">结束时间:</span>
                      <span className="text-sm font-semibold text-purple-600">
                        {formatDate(billingCycleEnd)}
                      </span>
                    </div>
                  )}
                </div>
              </div>
            )}

            {/* 原始数据（可折叠） */}
            {/* <details className="p-4 rounded-lg bg-gray-50">
              <summary className="font-medium text-gray-700 cursor-pointer hover:text-gray-900">
                🔍 查看原始数据
              </summary>
              <pre className="p-4 mt-4 overflow-x-auto text-xs text-gray-800 bg-white rounded">
                {JSON.stringify(progressData, null, 2)}
              </pre>
            </details> */}
          </div>
        ) : (
          <div className="py-12 text-center text-slate-500 dark:text-slate-400">暂无数据</div>
        )}
      </div>
    </div>
  );
};
