import React, { useState, useEffect } from "react";
import { AccountInfo } from "../types/account";
import { AggregatedUsageData } from "../types/usage";
import { LoadingSpinner } from "./LoadingSpinner";
import { AggregatedUsageDisplay } from "./AggregatedUsageDisplay";
import { invoke } from "@tauri-apps/api/core";

interface AccountUsageModalProps {
  isOpen: boolean;
  onClose: () => void;
  account: AccountInfo | null;
  onShowToast: (message: string, type: "success" | "error") => void;
}

type TimePeriod = "7days" | "30days" | "thisMonth" | "custom";

export const AccountUsageModal: React.FC<AccountUsageModalProps> = ({
  isOpen,
  onClose,
  account,
  onShowToast,
}) => {
  // 内部状态管理
  const [usageData, setUsageData] = useState<AggregatedUsageData | null>(null);
  const [loading, setLoading] = useState(false);
  const [selectedPeriod, setSelectedPeriod] = useState<TimePeriod>("30days");
  const [customStartDate, setCustomStartDate] = useState("");
  const [customEndDate, setCustomEndDate] = useState("");

  // 当账户变化时，重新获取数据
  useEffect(() => {
    if (isOpen && account) {
      handlePeriodChange("30days");
    }
  }, [isOpen, account]);

  // 计算日期范围
  const getDateRange = (period: TimePeriod) => {
    const now = new Date();
    let startDate: number;
    let endDate: number;

    switch (period) {
      case "7days":
        endDate = Math.floor(now.getTime());
        startDate = endDate - 7 * 24 * 60 * 60 * 1000;
        break;
      case "30days":
        endDate = Math.floor(now.getTime());
        startDate = endDate - 30 * 24 * 60 * 60 * 1000;
        break;
      case "thisMonth":
        const thisMonth = new Date(now.getFullYear(), now.getMonth(), 1);
        startDate = Math.floor(thisMonth.getTime());
        endDate = Math.floor(now.getTime());
        break;
      case "custom":
        if (!customStartDate || !customEndDate) {
          return null;
        }
        startDate = Math.floor(new Date(customStartDate).getTime());
        endDate = Math.floor(new Date(customEndDate + " 23:59:59").getTime());
        break;
      default:
        endDate = Math.floor(now.getTime());
        startDate = endDate - 30 * 24 * 60 * 60 * 1000;
    }

    return { startDate, endDate };
  };

  // 获取用量数据
  const fetchUsageData = async (period: TimePeriod) => {
    if (!account) return;

    const dateRange = getDateRange(period);
    if (!dateRange) {
      onShowToast("请选择有效的日期范围", "error");
      return;
    }

    try {
      setLoading(true);
      setUsageData(null);

      const teamId = -1; // 默认team_id

      const result = await invoke("get_usage_for_period", {
        token: account.token,
        startDate: dateRange.startDate,
        endDate: dateRange.endDate,
        teamId,
      });

      console.log("Usage result:", result);

      if (result && (result as any).success) {
        setUsageData((result as any).data);
        onShowToast("用量数据加载成功", "success");
      } else {
        setUsageData(null);
        onShowToast((result as any)?.message || "获取用量数据失败", "error");
      }
    } catch (error) {
      console.error("Failed to get usage data:", error);
      setUsageData(null);
      onShowToast("获取用量数据失败", "error");
    } finally {
      setLoading(false);
    }
  };

  // 切换时间段
  const handlePeriodChange = async (period: TimePeriod) => {
    setSelectedPeriod(period);
    if (period !== "custom") {
      await fetchUsageData(period);
    }
  };

  // 应用自定义日期范围
  const handleApplyCustomDate = async () => {
    if (!customStartDate || !customEndDate) {
      onShowToast("请选择开始和结束日期", "error");
      return;
    }
    await fetchUsageData("custom");
  };

  // 关闭Modal并重置状态
  const handleClose = () => {
    setSelectedPeriod("30days");
    setCustomStartDate("");
    setCustomEndDate("");
    setUsageData(null);
    setLoading(false);
    onClose();
  };

  if (!isOpen || !account) {
    return null;
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      <div
        className="fixed inset-0 bg-black/50 backdrop-blur-sm"
        onClick={handleClose}
      ></div>
      <div className="panel-floating relative mx-4 max-h-[90vh] w-full max-w-4xl overflow-hidden rounded-lg shadow-lg">
        {/* Modal Header */}
        <div className="flex items-center justify-between border-b border-slate-200 p-6 dark:border-slate-800">
          <h2 className="text-xl font-semibold text-slate-900 dark:text-slate-100">
            📊 账户用量详情 - {account.email}
          </h2>
          <button
            onClick={handleClose}
            title="关闭"
            className="rounded-lg p-2 text-slate-400 hover:bg-slate-100 hover:text-slate-600 dark:text-slate-500 dark:hover:bg-slate-900/60 dark:hover:text-slate-200"
          >
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
                d="M6 18L18 6M6 6l12 12"
              />
            </svg>
          </button>
        </div>

        {/* Modal Body */}
        <div className="p-6 overflow-y-auto max-h-[calc(90vh-120px)]">
          {/* 时间段选择 */}
          <div className="mb-6">
            <h3 className="mb-3 text-sm font-medium text-slate-700 dark:text-slate-200">
              时间段选择
            </h3>
            <div className="flex flex-wrap gap-2 mb-4">
              <button
                onClick={() => handlePeriodChange("7days")}
                className={`px-4 py-2 text-sm font-medium rounded-lg border transition-colors ${
                  selectedPeriod === "7days"
                    ? "bg-blue-600 text-white border-blue-600"
                    : "surface-secondary text-slate-700 border-subtle hover:bg-slate-200/80 dark:text-slate-200 dark:hover:bg-slate-700/70"
                }`}
              >
                最近7天
              </button>
              <button
                onClick={() => handlePeriodChange("30days")}
                className={`px-4 py-2 text-sm font-medium rounded-lg border transition-colors ${
                  selectedPeriod === "30days"
                    ? "bg-blue-600 text-white border-blue-600"
                    : "surface-secondary text-slate-700 border-subtle hover:bg-slate-200/80 dark:text-slate-200 dark:hover:bg-slate-700/70"
                }`}
              >
                最近30天
              </button>
              <button
                onClick={() => handlePeriodChange("thisMonth")}
                className={`px-4 py-2 text-sm font-medium rounded-lg border transition-colors ${
                  selectedPeriod === "thisMonth"
                    ? "bg-blue-600 text-white border-blue-600"
                    : "surface-secondary text-slate-700 border-subtle hover:bg-slate-200/80 dark:text-slate-200 dark:hover:bg-slate-700/70"
                }`}
              >
                本月
              </button>
              <button
                onClick={() => handlePeriodChange("custom")}
                className={`px-4 py-2 text-sm font-medium rounded-lg border transition-colors ${
                  selectedPeriod === "custom"
                    ? "bg-blue-600 text-white border-blue-600"
                    : "surface-secondary text-slate-700 border-subtle hover:bg-slate-200/80 dark:text-slate-200 dark:hover:bg-slate-700/70"
                }`}
              >
                自定义
              </button>
            </div>

            {/* 自定义日期选择 */}
            {selectedPeriod === "custom" && (
              <div className="surface-secondary flex items-end gap-4 rounded-lg p-4">
                <div className="flex-1">
                  <label className="mb-1 block text-sm font-medium text-slate-700 dark:text-slate-200">
                    开始日期
                  </label>
                  <input
                    type="date"
                    value={customStartDate}
                    onChange={(e) => setCustomStartDate(e.target.value)}
                    placeholder="选择开始日期"
                    className="field-input"
                  />
                </div>
                <div className="flex-1">
                  <label className="mb-1 block text-sm font-medium text-slate-700 dark:text-slate-200">
                    结束日期
                  </label>
                  <input
                    type="date"
                    value={customEndDate}
                    onChange={(e) => setCustomEndDate(e.target.value)}
                    placeholder="选择结束日期"
                    className="field-input"
                  />
                </div>
                <button
                  onClick={handleApplyCustomDate}
                  disabled={!customStartDate || !customEndDate}
                  className="rounded-md border border-transparent bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50"
                >
                  应用
                </button>
              </div>
            )}
          </div>

          {/* 用量数据显示 */}
          {loading ? (
            <div className="flex items-center justify-center py-12">
              <LoadingSpinner />
              <span className="ml-3 text-slate-600 dark:text-slate-300">正在加载用量数据...</span>
            </div>
          ) : usageData ? (
            <AggregatedUsageDisplay
              aggregatedUsage={usageData}
              title={`用量统计 - ${
                selectedPeriod === "7days"
                  ? "最近7天"
                  : selectedPeriod === "30days"
                  ? "最近30天"
                  : selectedPeriod === "thisMonth"
                  ? "本月"
                  : "自定义时间段"
              }`}
              variant="detailed"
              token={account.token}
              showDetailsButton={true}
            />
          ) : (
            <div className="py-12 text-center">
              <div className="mb-2 text-lg text-slate-500 dark:text-slate-400">📭</div>
              <p className="text-slate-600 dark:text-slate-300">暂无用量数据</p>
              <p className="mt-1 text-sm text-slate-500 dark:text-slate-400">
                可能是Token无效或者选择的时间段内没有使用记录
              </p>
            </div>
          )}
        </div>

        {/* Modal Footer */}
        <div className="flex justify-end border-t border-slate-200 p-6 dark:border-slate-800">
          <button
            onClick={handleClose}
            className="surface-secondary rounded-md border border-transparent px-4 py-2 text-sm font-medium text-slate-700 hover:bg-slate-200/80 focus:outline-none focus:ring-2 focus:ring-gray-500 focus:ring-offset-2 dark:text-slate-200 dark:hover:bg-slate-700/70"
          >
            关闭
          </button>
        </div>
      </div>
    </div>
  );
};
