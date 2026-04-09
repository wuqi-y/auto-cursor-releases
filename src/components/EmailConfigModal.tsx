import React, { useState, useEffect } from "react";
import { Button } from "./Button";
import { Toast } from "./Toast";
import { LoadingSpinner } from "./LoadingSpinner";
import { EmailConfig, EMPTY_EMAIL_CONFIG } from "../types/emailConfig";
import { EmailConfigService } from "../services/emailConfigService";

interface EmailConfigModalProps {
  isOpen: boolean;
  onClose: () => void;
  onSave?: (config: EmailConfig) => void;
}

export const EmailConfigModal: React.FC<EmailConfigModalProps> = ({
  isOpen,
  onClose,
  onSave,
}) => {
  const [config, setConfig] = useState<EmailConfig>(EMPTY_EMAIL_CONFIG);
  const [isLoading, setIsLoading] = useState(false);
  const [isTesting, setIsTesting] = useState(false);
  const [toast, setToast] = useState<{
    message: string;
    type: "success" | "error" | "info";
  } | null>(null);

  // 加载配置
  useEffect(() => {
    if (isOpen) {
      loadConfig();
    }
  }, [isOpen]);

  const loadConfig = async () => {
    try {
      const loadedConfig = await EmailConfigService.getEmailConfig();
      setConfig(loadedConfig);
    } catch (error) {
      console.error("加载邮箱配置失败:", error);
      setToast({ message: "加载配置失败", type: "error" });
    }
  };

  const handleInputChange = (field: keyof EmailConfig, value: string) => {
    setConfig((prev) => ({ ...prev, [field]: value }));
  };

  const handleTest = async () => {
    // 验证配置
    const validation = EmailConfigService.validateEmailConfig(config);
    if (!validation.isValid) {
      setToast({
        message: `配置验证失败: ${validation.errors.join(", ")}`,
        type: "error",
      });
      return;
    }

    setIsTesting(true);
    try {
      const result = await EmailConfigService.testEmailConfig(config);
      setToast({
        message: result.message,
        type: result.success ? "success" : "error",
      });
    } catch (error) {
      setToast({ message: `测试失败: ${error}`, type: "error" });
    } finally {
      setIsTesting(false);
    }
  };

  const handleSave = async () => {
    // 验证配置
    const validation = EmailConfigService.validateEmailConfig(config);
    if (!validation.isValid) {
      setToast({
        message: `配置验证失败: ${validation.errors.join(", ")}`,
        type: "error",
      });
      return;
    }

    setIsLoading(true);
    try {
      const result = await EmailConfigService.saveEmailConfig(config);
      if (result.success) {
        setToast({ message: result.message, type: "success" });
        onSave?.(config);
        // 延迟关闭模态框，让用户看到成功消息
        setTimeout(() => {
          onClose();
        }, 1500);
      } else {
        setToast({ message: result.message, type: "error" });
      }
    } catch (error) {
      setToast({ message: `保存失败: ${error}`, type: "error" });
    } finally {
      setIsLoading(false);
    }
  };

  const handleReset = () => {
    setConfig(EMPTY_EMAIL_CONFIG);
    setToast({ message: "已清空配置", type: "info" });
  };

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black bg-opacity-50">
      <div className="panel-floating mx-4 max-h-[90vh] w-full max-w-2xl overflow-y-auto rounded-lg p-6">
        <div className="flex items-center justify-between mb-6">
          <h2 className="text-2xl font-bold text-slate-900 dark:text-slate-100">📧 邮箱配置</h2>
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
          {/* Worker 域名配置 */}
          <div>
            <label className="mb-2 block text-sm font-medium text-slate-700 dark:text-slate-300">
              Worker 域名 *
            </label>
            <input
              type="text"
              value={config.worker_domain}
              onChange={(e) =>
                handleInputChange("worker_domain", e.target.value)
              }
              placeholder="例如: apimail.xx.icu"
              className="field-input"
              disabled={isLoading}
            />
            <p className="text-muted mt-1 text-sm">
              用于API请求的Cloudflare Worker域名
            </p>
          </div>

          {/* 邮箱域名配置 */}
          <div>
            <label className="mb-2 block text-sm font-medium text-slate-700 dark:text-slate-300">
              邮箱域名 *
            </label>
            <input
              type="text"
              value={config.email_domain}
              onChange={(e) =>
                handleInputChange("email_domain", e.target.value)
              }
              placeholder="例如: xx.icu"
              className="field-input"
              disabled={isLoading}
            />
            <p className="text-muted mt-1 text-sm">
              用于生成临时邮箱地址的域名
            </p>
          </div>

          {/* 管理员密码配置 */}
          <div>
            <label className="mb-2 block text-sm font-medium text-slate-700 dark:text-slate-300">
              管理员密码 *
            </label>
            <input
              type="password"
              value={config.admin_password}
              onChange={(e) =>
                handleInputChange("admin_password", e.target.value)
              }
              placeholder="至少6位字符"
              className="field-input"
              disabled={isLoading}
            />
            <p className="text-muted mt-1 text-sm">
              用于访问邮箱服务的管理员密码 (X-Admin-Auth)
            </p>
          </div>

          {/* 访问密码配置 */}
          <div>
            <label className="mb-2 block text-sm font-medium text-slate-700 dark:text-slate-300">
              访问密码 *（如没有设置可随意填写）
            </label>
            <input
              type="password"
              value={config.access_password}
              onChange={(e) =>
                handleInputChange("access_password", e.target.value)
              }
              placeholder="至少6位字符"
              className="field-input"
              disabled={isLoading}
            />
            <p className="text-muted mt-1 text-sm">
              用于API注册访问的密码 (x-Custom-Auth)
            </p>
          </div>

          {/* 配置说明 */}
          <div className="status-info rounded-md p-4">
            <div className="flex">
              <div className="flex-shrink-0">
                <svg
                  className="w-5 h-5 text-blue-400"
                  fill="currentColor"
                  viewBox="0 0 20 20"
                >
                  <path
                    fillRule="evenodd"
                    d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-7-4a1 1 0 11-2 0 1 1 0 012 0zM9 9a1 1 0 000 2v3a1 1 0 001 1h1a1 1 0 100-2v-3a1 1 0 00-1-1H9z"
                    clipRule="evenodd"
                  />
                </svg>
              </div>
              <div className="ml-3">
                <h3 className="text-sm font-medium text-blue-800 dark:text-blue-100">配置说明</h3>
                <div className="mt-2 text-sm text-blue-700 dark:text-blue-200">
                  <ul className="space-y-1 list-disc list-inside">
                    <li>Worker域名：用于API请求的Cloudflare Worker服务域名</li>
                    <li>邮箱域名：用于生成临时邮箱地址的域名后缀</li>
                    <li>
                      管理员密码：访问邮箱服务时使用的认证密码 (X-Admin-Auth)
                    </li>
                    <li>
                      访问密码：API注册时使用的访问认证密码 (x-Custom-Auth)
                    </li>
                    <li>修改配置后建议先测试连接，确保服务可用</li>
                  </ul>
                </div>
              </div>
            </div>
          </div>

          {/* 操作按钮 */}
          <div className="flex justify-between space-x-3">
            <div className="flex space-x-3">
              <Button
                onClick={handleReset}
                variant="secondary"
                disabled={isLoading || isTesting}
              >
                清空配置
              </Button>
              <Button
                onClick={handleTest}
                variant="secondary"
                disabled={isLoading || isTesting}
              >
                {isTesting ? (
                  <>
                    <LoadingSpinner size="sm" />
                    <span className="ml-2">测试中...</span>
                  </>
                ) : (
                  "测试连接"
                )}
              </Button>
            </div>

            <div className="flex space-x-3">
              <Button
                onClick={onClose}
                variant="secondary"
                disabled={isLoading}
              >
                取消
              </Button>
              <Button onClick={handleSave} disabled={isLoading || isTesting}>
                {isLoading ? (
                  <>
                    <LoadingSpinner size="sm" />
                    <span className="ml-2">保存中...</span>
                  </>
                ) : (
                  "保存配置"
                )}
              </Button>
            </div>
          </div>
        </div>

        {/* Toast 消息 */}
        {toast && (
          <Toast
            message={toast.message}
            type={toast.type}
            onClose={() => setToast(null)}
          />
        )}
      </div>
    </div>
  );
};
