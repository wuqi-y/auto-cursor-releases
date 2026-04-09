import React, { useState, useEffect } from "react";
import { CursorService } from "../services/cursorService";
import { Button } from "../components/Button";
import { AuthCheckResult, TokenInfo } from "../types/auth";
import { AggregatedUsageDisplay } from "../components/AggregatedUsageDisplay";

export const AuthCheckPage: React.FC = () => {
  const [userToken, setUserToken] = useState<string>("");
  const [tokenInfo, setTokenInfo] = useState<TokenInfo | null>(null);
  const [authResult, setAuthResult] = useState<AuthCheckResult | null>(null);
  const [autoTokenLoading, setAutoTokenLoading] = useState<boolean>(false);
  const [checkingAuth, setCheckingAuth] = useState<boolean>(false);
  const [showDebug, setShowDebug] = useState<boolean>(false);

  useEffect(() => {
    // Auto-load token when component mounts
    getTokenAuto();
  }, []);

  const getTokenAuto = async () => {
    try {
      setAutoTokenLoading(true);
      const info = await CursorService.getTokenAuto();
      setTokenInfo(info);

      if (info.token) {
        setUserToken(info.token);
      }
    } catch (error) {
      console.error("自动获取 token 失败:", error);
    } finally {
      setAutoTokenLoading(false);
    }
  };

  const checkAuthorization = async () => {
    if (!userToken.trim()) {
      alert("请输入 token");
      return;
    }

    try {
      setCheckingAuth(true);
      const result = await CursorService.checkUserAuthorized(userToken.trim());
      setAuthResult(result);
    } catch (error) {
      console.error("检查授权失败:", error);
    } finally {
      setCheckingAuth(false);
    }
  };

  return (
    <div className="space-y-6">
      {/* Header */}
      <div>
        <h1 className="text-2xl font-bold text-slate-100">授权检查</h1>
        <p className="mt-1 text-sm text-slate-400">
          检查 Cursor 账户的授权状态和订阅信息
        </p>
      </div>

      {/* Token Input Section */}
      <div className="surface-primary rounded-2xl p-6 shadow">
        <h2 className="mb-4 text-lg font-medium text-slate-900 dark:text-slate-100">
          🔑 Token 输入
        </h2>

        {/* Auto Token Info */}
        {tokenInfo && (
          <div className="status-info mb-4 rounded-lg p-4">
            <div className="flex items-center justify-between mb-2">
              <h3 className="font-medium text-blue-800 dark:text-blue-100">自动检测到的 Token</h3>
              <Button
                variant="secondary"
                size="sm"
                onClick={getTokenAuto}
                loading={autoTokenLoading}
              >
                🔄 重新获取
              </Button>
            </div>
            <div className="space-y-2 text-sm">
              <p>
                <strong>来源:</strong> {tokenInfo.source}
              </p>
              <p>
                <strong>状态:</strong>
                <span
                  className={
                    tokenInfo.found ? "text-green-600" : "text-red-600"
                  }
                >
                  {tokenInfo.found ? " ✅ 已找到" : " ❌ 未找到"}
                </span>
              </p>
              <p>
                <strong>消息:</strong> {tokenInfo.message}
              </p>
              {tokenInfo.token && (
                <p>
                  <strong>Token 长度:</strong> {tokenInfo.token.length} 字符
                </p>
              )}
            </div>
          </div>
        )}

        {/* Manual Token Input */}
        <div className="space-y-4">
          <div>
            <label
              htmlFor="token"
              className="block mb-2 text-sm font-medium text-slate-700 dark:text-slate-300"
            >
              Token (手动输入或使用自动检测的)
            </label>
            <textarea
              id="token"
              value={userToken}
              onChange={(e) => setUserToken(e.target.value)}
              placeholder="请输入您的 Cursor token..."
              className="field-input h-32 rounded-xl px-3 py-2 font-mono text-sm"
            />
          </div>

          <Button
            variant="primary"
            onClick={checkAuthorization}
            loading={checkingAuth}
            disabled={!userToken.trim()}
            className="w-full"
          >
            🔍 检查授权状态
          </Button>
        </div>
      </div>

      {/* Auth Results */}
      {authResult && (
        <div className="surface-primary rounded-2xl p-6 shadow">
          <div className="flex items-center justify-between mb-4">
            <h2 className="text-lg font-medium text-slate-900 dark:text-slate-100">📊 检查结果</h2>
            <Button
              variant="secondary"
              size="sm"
              onClick={() => setShowDebug(!showDebug)}
            >
              {showDebug ? "隐藏" : "显示"}详细信息
            </Button>
          </div>

          {/* Basic Info */}
          <div className="grid grid-cols-1 gap-4 mb-6 md:grid-cols-2">
            <div className="surface-secondary rounded p-3">
              <p className="text-sm text-slate-600 dark:text-slate-300">授权状态</p>
              <p
                className={`font-medium ${
                  authResult.success ? "text-green-700" : "text-red-700"
                }`}
              >
                {authResult.success ? "✅ 已授权" : "❌ 未授权"}
              </p>
            </div>

            {authResult.user_info && (
              <>
                <div className="surface-secondary rounded p-3">
                  <p className="text-sm text-slate-600 dark:text-slate-300">Token 长度</p>
                  <p className="font-medium text-slate-800 dark:text-slate-100">
                    {authResult.user_info.token_length} 字符
                  </p>
                </div>

                <div className="surface-secondary rounded p-3">
                  <p className="text-sm text-slate-600 dark:text-slate-300">Token 格式</p>
                  <p className="font-medium text-slate-800 dark:text-slate-100">
                    {authResult.user_info.token_valid
                      ? "✅ JWT 格式"
                      : "❌ 非 JWT 格式"}
                  </p>
                </div>

                {authResult.user_info.api_status && (
                  <div className="surface-secondary rounded p-3">
                    <p className="text-sm text-slate-600 dark:text-slate-300">API 状态码</p>
                    <p className="font-medium text-slate-800 dark:text-slate-100">
                      {authResult.user_info.api_status}
                    </p>
                  </div>
                )}
              </>
            )}
          </div>

          {/* Account Info */}
          {authResult.user_info?.account_info && (
            <div className="mb-6">
              <h3 className="mb-3 text-lg font-medium text-slate-700 dark:text-slate-200">
                账户信息:
              </h3>
              <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
                {authResult.user_info.account_info.email && (
                  <div className="status-info rounded p-3">
                    <p className="text-sm text-slate-600 dark:text-slate-300">📧 邮箱</p>
                    <p className="font-medium text-slate-800 dark:text-slate-100">
                      {authResult.user_info.account_info.email}
                    </p>
                  </div>
                )}

                {authResult.user_info.account_info.username && (
                  <div className="status-info rounded p-3">
                    <p className="text-sm text-slate-600 dark:text-slate-300">👤 用户名</p>
                    <p className="font-medium text-slate-800 dark:text-slate-100">
                      {authResult.user_info.account_info.username}
                    </p>
                  </div>
                )}

                {authResult.user_info.account_info.subscription_status && (
                  <div className="status-success rounded p-3">
                    <p className="text-sm text-slate-600 dark:text-slate-300">📊 订阅状态</p>
                    <p className="font-medium text-green-700">
                      {authResult.user_info.account_info.subscription_status}
                    </p>
                  </div>
                )}

                {authResult.user_info.account_info.subscription_type && (
                  <div className="status-success rounded p-3">
                    <p className="text-sm text-slate-600 dark:text-slate-300">💳 订阅类型</p>
                    <p className="font-medium text-green-700">
                      {authResult.user_info.account_info.subscription_type}
                    </p>
                  </div>
                )}

                {authResult.user_info.account_info.trial_days_remaining !==
                  undefined && (
                  <div className="status-warning rounded p-3">
                    <p className="text-sm text-slate-600 dark:text-slate-300">⏰ 试用剩余天数</p>
                    <p className="font-medium text-yellow-700">
                      {authResult.user_info.account_info.trial_days_remaining}{" "}
                      天
                    </p>
                  </div>
                )}

                {authResult.user_info.account_info.usage_info && (
                  <div className="surface-secondary rounded p-3">
                    <p className="text-sm text-slate-600 dark:text-slate-300">📈 使用信息</p>
                    <p className="font-medium text-slate-800 dark:text-slate-100">
                      {authResult.user_info.account_info.usage_info}
                    </p>
                  </div>
                )}
              </div>

              {/* Aggregated Usage Data */}
              {authResult.user_info.account_info.aggregated_usage && (
                <div className="mt-6">
                  <AggregatedUsageDisplay
                    aggregatedUsage={
                      authResult.user_info.account_info.aggregated_usage
                    }
                    title="📊 聚合用量数据 (最近30天)"
                    variant="detailed"
                  />
                </div>
              )}
            </div>
          )}

          {/* Debug Info */}
          {showDebug && authResult.details && (
            <div>
              <h3 className="mb-3 text-lg font-medium text-slate-700 dark:text-slate-200">
                详细信息:
              </h3>
              <div className="space-y-2">
                {authResult.details.map((detail, index) => (
                  <div
                    key={index}
                    className="panel-code overflow-auto rounded p-3 text-sm"
                  >
                    {detail}
                  </div>
                ))}
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  );
};
