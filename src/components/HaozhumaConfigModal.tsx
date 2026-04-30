import React, { useEffect, useState } from "react";
import { Button } from "./Button";
import { Toast } from "./Toast";
import { LoadingSpinner } from "./LoadingSpinner";
import { EMPTY_HAOZHUMA_CONFIG, HaozhumaConfig } from "../types/haozhumaConfig";
import { HaozhumaConfigService } from "../services/haozhumaConfigService";

interface HaozhumaConfigModalProps {
  isOpen: boolean;
  onClose: () => void;
  onSave?: (config: HaozhumaConfig) => void;
}

export const HaozhumaConfigModal: React.FC<HaozhumaConfigModalProps> = ({
  isOpen,
  onClose,
  onSave,
}) => {
  const [config, setConfig] = useState<HaozhumaConfig>(EMPTY_HAOZHUMA_CONFIG);
  const [isLoading, setIsLoading] = useState(false);
  const [toast, setToast] = useState<{
    message: string;
    type: "success" | "error" | "info";
  } | null>(null);
  const [isTesting, setIsTesting] = useState(false);

  useEffect(() => {
    if (isOpen) {
      void loadConfig();
    }
  }, [isOpen]);

  const loadConfig = async () => {
    try {
      const loadedConfig = await HaozhumaConfigService.getHaozhumaConfig();
      setConfig(loadedConfig);
    } catch (error) {
      console.error("加载豪猪配置失败:", error);
      setToast({ message: "加载豪猪配置失败", type: "error" });
    }
  };

  const handleInputChange = (
    field: keyof HaozhumaConfig,
    value: string | boolean,
  ) => {
    setConfig((prev) => ({ ...prev, [field]: value }));
  };

  const handleFilterChange = (
    field: keyof HaozhumaConfig["phone_filters"],
    value: string,
  ) => {
    setConfig((prev) => ({
      ...prev,
      phone_filters: {
        ...prev.phone_filters,
        [field]: value,
      },
    }));
  };

  const handleRetryChange = (
    field: keyof HaozhumaConfig["retry"],
    value: string,
  ) => {
    const parsedValue = Number(value);
    setConfig((prev) => ({
      ...prev,
      retry: {
        ...prev.retry,
        [field]:
          Number.isFinite(parsedValue) && parsedValue > 0 ? parsedValue : 0,
      },
    }));
  };

  const handleSave = async () => {
    const validation = HaozhumaConfigService.validateHaozhumaConfig(config);
    if (!validation.isValid) {
      setToast({
        message: `配置验证失败: ${validation.errors.join(", ")}`,
        type: "error",
      });
      return;
    }

    setIsLoading(true);
    try {
      const result = await HaozhumaConfigService.saveHaozhumaConfig(config);
      if (result.success) {
        setToast({ message: result.message, type: "success" });
        onSave?.(config);
        setTimeout(() => {
          onClose();
        }, 1200);
      } else {
        setToast({ message: result.message, type: "error" });
      }
    } catch (error) {
      setToast({ message: `保存失败: ${error}`, type: "error" });
    } finally {
      setIsLoading(false);
    }
  };

  const handleTestApi = async () => {
    const validation = HaozhumaConfigService.validateHaozhumaConfig(config);
    if (!validation.isValid) {
      setToast({
        message: `配置验证失败: ${validation.errors.join(", ")}`,
        type: "error",
      });
      return;
    }

    setIsTesting(true);
    try {
      const result = await HaozhumaConfigService.testHaozhumaApi(config);
      if (result.success) {
        setToast({
          message: `${result.message}${result.phone_last4 ? `，取到号码后4位: ${result.phone_last4}` : ""}`,
          type: "success",
        });
      } else {
        setToast({
          message: result.message,
          type: "error",
        });
      }
    } catch (error) {
      setToast({ message: `测试失败: ${error}`, type: "error" });
    } finally {
      setIsTesting(false);
    }
  };

  const handleReset = () => {
    setConfig(EMPTY_HAOZHUMA_CONFIG);
    setToast({ message: "已恢复默认豪猪配置", type: "info" });
  };

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black bg-opacity-50">
      <div className="panel-floating mx-4 max-h-[90vh] w-full max-w-4xl overflow-y-auto rounded-lg p-6">
        <div className="flex items-center justify-between mb-6">
          <h2 className="text-2xl font-bold text-slate-900 dark:text-slate-100">
            豪猪手机号配置
          </h2>
          <button
            onClick={onClose}
            className="text-slate-400 hover:text-slate-600 dark:text-slate-500 dark:hover:text-slate-300"
            disabled={isLoading}
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

        <div className="space-y-6">
          <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
            <div className="md:col-span-2">
              <label className="flex items-center gap-2 mb-2 text-sm font-medium text-slate-700 dark:text-slate-300">
                <input
                  type="checkbox"
                  checked={config.enabled}
                  onChange={(e) =>
                    handleInputChange("enabled", e.target.checked)
                  }
                  disabled={isLoading}
                />
                启用豪猪自动手机号
              </label>
            </div>

            <div>
              <label className="block mb-2 text-sm font-medium text-slate-700 dark:text-slate-300">
                API 域名 *
              </label>
              <input
                type="text"
                value={config.api_domain}
                onChange={(e) =>
                  handleInputChange("api_domain", e.target.value)
                }
                placeholder="api.haozhuma.com"
                className="field-input"
                disabled={isLoading}
              />
            </div>

            <div>
              <label className="block mb-2 text-sm font-medium text-slate-700 dark:text-slate-300">
                项目 ID *
              </label>
              <input
                type="text"
                value={config.project_id}
                onChange={(e) =>
                  handleInputChange("project_id", e.target.value)
                }
                placeholder="对应 sid"
                className="field-input"
                disabled={isLoading}
              />
            </div>

            <div>
              <label className="block mb-2 text-sm font-medium text-slate-700 dark:text-slate-300">
                豪猪账号 *
              </label>
              <input
                type="text"
                value={config.username}
                onChange={(e) => handleInputChange("username", e.target.value)}
                placeholder="请输入豪猪账号"
                title="豪猪账号"
                className="field-input"
                disabled={isLoading}
              />
            </div>

            <div>
              <label className="block mb-2 text-sm font-medium text-slate-700 dark:text-slate-300">
                豪猪密码 *
              </label>
              <input
                type="password"
                value={config.password}
                onChange={(e) => handleInputChange("password", e.target.value)}
                placeholder="请输入豪猪密码"
                title="豪猪密码"
                className="field-input"
                disabled={isLoading}
              />
            </div>

            <div>
              <label className="block mb-2 text-sm font-medium text-slate-700 dark:text-slate-300">
                默认国家区号 *
              </label>
              <input
                type="text"
                value={config.default_country_code}
                onChange={(e) =>
                  handleInputChange(
                    "default_country_code",
                    e.target.value.replace(/[^\d]/g, ""),
                  )
                }
                placeholder="86"
                className="field-input"
                disabled={isLoading}
              />
            </div>

            <div>
              <label className="block mb-2 text-sm font-medium text-slate-700 dark:text-slate-300">
                指定手机号（可选）
              </label>
              <input
                type="text"
                value={config.fixed_phone}
                onChange={(e) =>
                  handleInputChange(
                    "fixed_phone",
                    e.target.value.replace(/[^\d+]/g, ""),
                  )
                }
                placeholder="留空则使用豪猪 API 取号"
                className="field-input"
                disabled={isLoading}
              />
              <p className="mt-1 text-xs text-slate-500">
                配置后注册时不再调用豪猪取号接口，直接用此号码发送验证码。
              </p>
            </div>
          </div>

          <div>
            <h3 className="mb-3 text-lg font-semibold text-slate-900 dark:text-slate-100">
              取号筛选参数
            </h3>
            <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
              {(
                [
                  ["isp", "运营商 isp"],
                  ["province", "省份 province"],
                  ["ascription", "号码类型 ascription"],
                  ["paragraph", "只取号段 paragraph"],
                  ["exclude", "排除号段 exclude"],
                  ["uid", "对接码 uid"],
                  ["author", "开发者账号 author"],
                ] as const
              ).map(([key, label]) => (
                <div key={key}>
                  <label className="block mb-2 text-sm font-medium text-slate-700 dark:text-slate-300">
                    {label}
                  </label>
                  <input
                    type="text"
                    value={config.phone_filters[key]}
                    onChange={(e) => handleFilterChange(key, e.target.value)}
                    placeholder={label}
                    title={label}
                    className="field-input"
                    disabled={isLoading}
                  />
                </div>
              ))}
            </div>
          </div>

          <div>
            <h3 className="mb-3 text-lg font-semibold text-slate-900 dark:text-slate-100">
              重试与轮询
            </h3>
            <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
              {(
                [
                  ["max_phone_retry", "最大换号次数"],
                  ["poll_interval_seconds", "轮询间隔(秒)"],
                  ["send_check_timeout_seconds", "发码检测超时(秒)"],
                  ["sms_poll_timeout_seconds", "短信轮询超时(秒)"],
                ] as const
              ).map(([key, label]) => (
                <div key={key}>
                  <label className="block mb-2 text-sm font-medium text-slate-700 dark:text-slate-300">
                    {label}
                  </label>
                  <input
                    type="number"
                    min="1"
                    value={config.retry[key]}
                    onChange={(e) => handleRetryChange(key, e.target.value)}
                    placeholder={label}
                    title={label}
                    className="field-input"
                    disabled={isLoading}
                  />
                </div>
              ))}
            </div>
          </div>

          <div className="p-4 text-sm rounded-md status-info">
            <p className="text-blue-800 dark:text-blue-100">
              这里保存的是豪猪 API 的持久化配置，注册时会把它合并进运行时
              `config_json`，Python 侧只负责在手机号验证页面执行自动化流程。
            </p>
          </div>

          <div className="flex justify-between">
            <div className="flex gap-3">
              <Button
                variant="secondary"
                onClick={handleTestApi}
                disabled={isLoading || isTesting}
              >
                {isTesting ? (
                  <span className="flex items-center gap-2">
                    <LoadingSpinner size="sm" />
                    测试中
                  </span>
                ) : (
                  "测试豪猪 API"
                )}
              </Button>
              <Button
                variant="secondary"
                onClick={handleReset}
                disabled={isLoading || isTesting}
              >
                重置默认值
              </Button>
            </div>
            <div className="flex gap-3">
              <Button
                variant="secondary"
                onClick={onClose}
                disabled={isLoading}
              >
                取消
              </Button>
              <Button
                variant="primary"
                onClick={handleSave}
                disabled={isLoading}
              >
                {isLoading ? (
                  <span className="flex items-center gap-2">
                    <LoadingSpinner size="sm" />
                    保存中
                  </span>
                ) : (
                  "保存配置"
                )}
              </Button>
            </div>
          </div>
        </div>
      </div>
      {toast && (
        <Toast
          message={toast.message}
          type={toast.type}
          onClose={() => setToast(null)}
        />
      )}
    </div>
  );
};
