import React, { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";

export interface WebServerConfigProps {
  isOpen: boolean;
  isSeamlessEnabled: boolean; // 无感换号是否启用
  onShowToast: (message: string, type: "success" | "error") => void;
}

export const WebServerConfig: React.FC<WebServerConfigProps> = ({
  isOpen,
  isSeamlessEnabled,
  onShowToast,
}) => {
  const [webServerPort, setWebServerPort] = useState<number>(34567);
  // const [webServerPortLoading, setWebServerPortLoading] = useState(false);

  // 自动轮换配置
  const [autoSwitchEnabled, setAutoSwitchEnabled] = useState<boolean>(false);
  const [costThreshold, setCostThreshold] = useState<number>(10);
  const [manualConfigEnabled, setManualConfigEnabled] =
    useState<boolean>(false);
  const [autoSwitchLoading, setAutoSwitchLoading] = useState(false);
  const [refreshCacheLoading, setRefreshCacheLoading] = useState(false);

  // 获取Web服务器端口
  const getWebServerPort = async () => {
    try {
      const port: number = await invoke("get_web_server_port");
      setWebServerPort(port);
    } catch (error) {
      console.error("获取Web服务器端口失败:", error);
      setWebServerPort(34567); // 默认端口
    }
  };

  // 设置Web服务器端口
  // const handleSetWebServerPort = async (newPort: number) => {
  //   if (newPort < 1 || newPort > 65535) {
  //     onShowToast("端口号必须在1-65535之间", "error");
  //     return;
  //   }

  //   try {
  //     setWebServerPortLoading(true);
  //     const result: string = await invoke("set_web_server_port", {
  //       port: newPort,
  //     });
  //     onShowToast(result, "success");
  //     setWebServerPort(newPort);
  //   } catch (error) {
  //     console.error("设置Web服务器端口失败:", error);
  //     onShowToast("设置端口失败", "error");
  //   } finally {
  //     setWebServerPortLoading(false);
  //   }
  // };

  // 获取自动轮换配置
  const getAutoSwitchConfig = async () => {
    try {
      const response = await fetch(
        `http://127.0.0.1:${webServerPort}/api/auto-switch/config`
      );
      const config = await response.json();

      if (config.autoSwitchEnabled !== undefined) {
        setAutoSwitchEnabled(config.autoSwitchEnabled);
      }
      if (config.costThreshold !== undefined) {
        setCostThreshold(config.costThreshold);
      }
      if (config.manualConfigEnabled !== undefined) {
        setManualConfigEnabled(config.manualConfigEnabled);
      }
    } catch (error) {
      console.error("获取自动轮换配置失败:", error);
      // 使用默认值
      setAutoSwitchEnabled(false);
      setCostThreshold(10);
      setManualConfigEnabled(false);
    }
  };

  // 保存自动轮换配置
  const saveAutoSwitchConfig = async () => {
    if (costThreshold <= 0) {
      onShowToast("费用阈值必须大于0", "error");
      return;
    }

    try {
      setAutoSwitchLoading(true);
      const response = await fetch(
        `http://127.0.0.1:${webServerPort}/api/auto-switch/config`,
        {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
          },
          body: JSON.stringify({
            autoSwitchEnabled,
            costThreshold,
            manualConfigEnabled,
          }),
        }
      );

      const result = await response.json();

      if (result.success) {
        onShowToast("自动轮换配置保存成功", "success");
      } else {
        onShowToast(result.error || "保存配置失败", "error");
      }
    } catch (error) {
      console.error("保存自动轮换配置失败:", error);
      onShowToast("保存配置失败", "error");
    } finally {
      setAutoSwitchLoading(false);
    }
  };

  // 刷新账户缓存
  const handleRefreshCache = async () => {
    try {
      setRefreshCacheLoading(true);
      const result: any = await invoke("refresh_eligible_accounts_cache");

      if (result.success) {
        onShowToast(result.message || "缓存刷新成功", "success");
      } else {
        onShowToast(result.message || "缓存刷新失败", "error");
      }
    } catch (error) {
      console.error("刷新缓存失败:", error);
      onShowToast("刷新缓存失败", "error");
    } finally {
      setRefreshCacheLoading(false);
    }
  };

  useEffect(() => {
    if (isOpen) {
      getWebServerPort();
      getAutoSwitchConfig();
    }
  }, [isOpen]);

  if (!isOpen) {
    return null;
  }

  return (
    <div className="mb-6 rounded-2xl border border-teal-200/70 bg-teal-50/90 p-4 dark:border-teal-500/25 dark:bg-teal-500/10">
      <h4 className="mb-3 font-medium text-teal-800 dark:text-teal-200 text-md">
        🔄 自动换号配置
      </h4>
      <div className="space-y-4">
        {/* <div>
          <label className="block mb-2 text-sm font-medium text-teal-700 dark:text-teal-300">
            服务端口
          </label>
          <div className="flex items-center space-x-3">
            <input
              type="number"
              value={webServerPort}
              onChange={(e) =>
                setWebServerPort(parseInt(e.target.value) || 34567)
              }
              min="1"
              max="65535"
              className="block w-32 px-3 py-2 text-sm border border-teal-300 dark:border-teal-500/30 rounded-md focus:outline-none focus:ring-2 focus:ring-teal-500 focus:border-teal-500"
              placeholder="34567"
            />
            <button
              type="button"
              onClick={() => handleSetWebServerPort(webServerPort)}
              disabled={webServerPortLoading}
              className={`inline-flex items-center px-3 py-2 text-sm font-medium text-white border border-transparent rounded-md focus:outline-none focus:ring-2 focus:ring-offset-2 ${
                webServerPortLoading
                  ? "bg-slate-400 cursor-not-allowed dark:bg-slate-700"
                  : "bg-teal-600 hover:bg-teal-700 focus:ring-teal-500"
              }`}
            >
              {webServerPortLoading ? "🔄 设置中..." : "💾 保存"}
            </button>
          </div>
          <p className="mt-1 text-xs text-teal-600">
            当前端口: {webServerPort}，修改后需要重启应用生效
          </p>
        </div> */}

        {/* 自动轮换账户配置 */}
        <div className="pt-4 border-t border-teal-300 dark:border-teal-500/30">
          <div className="mb-3">
            <h5 className="text-sm font-medium text-teal-800 dark:text-teal-200">
              🔄 自动轮换账户
            </h5>
            <p className="mt-1 text-xs text-teal-600">
              当检测到账户用量达到限制时，自动切换到可用账户
            </p>
          </div>

          <div className="space-y-3">
            {/* 开关 */}
            <div className="flex items-center justify-between">
              <label className="text-sm font-medium text-teal-700 dark:text-teal-300">
                启用自动轮换
              </label>
              <button
                type="button"
                onClick={() => setAutoSwitchEnabled(!autoSwitchEnabled)}
                disabled={!isSeamlessEnabled}
                className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors focus:outline-none focus:ring-2 focus:ring-teal-500 focus:ring-offset-2 ${
                  !isSeamlessEnabled
                    ? "field-toggle cursor-not-allowed"
                    : autoSwitchEnabled
                    ? "bg-teal-600"
                    : "field-toggle"
                }`}
                title={
                  !isSeamlessEnabled
                    ? "请先启用无感换号功能"
                    : autoSwitchEnabled
                    ? "点击关闭自动轮换"
                    : "点击开启自动轮换"
                }
              >
                <span
                  className={`field-toggle-thumb h-4 w-4 transform ${
                    autoSwitchEnabled ? "translate-x-6" : "translate-x-1"
                  }`}
                />
              </button>
            </div>

            {!isSeamlessEnabled && (
              <div className="status-warning rounded-md p-2 text-xs">
                ⚠️ 自动轮换功能需要先启用无感换号
              </div>
            )}

            {/* 费用阈值 */}
            <div>
              <label className="block mb-2 text-sm font-medium text-teal-700 dark:text-teal-300">
                费用阈值（美元）
              </label>
              <div>
                <input
                  type="number"
                  value={costThreshold}
                  onChange={(e) =>
                    setCostThreshold(parseFloat(e.target.value) || 10)
                  }
                  min="0.01"
                  step="0.01"
                  className="field-input block w-32 focus:ring-teal-500"
                  placeholder="10.00"
                />
              </div>
              <p className="mt-1 text-xs text-teal-600">
                当账户费用低于此阈值时，将被选为切换目标
              </p>
            </div>

            {/* 手动配置轮换开关 */}
            <div className="flex items-center justify-between">
              <div>
                <label className="text-sm font-medium text-teal-700 dark:text-teal-300">
                  开启手动配置轮换账户 ⭐ 推荐
                </label>
                <p className="mt-1 text-xs text-teal-600">
                  开启后优先自动轮换用户设置的账户，无需获取用量，无需填写work_os_session_token，自动切换速度非常快
                </p>
              </div>
              <button
                type="button"
                onClick={() => setManualConfigEnabled(!manualConfigEnabled)}
                disabled={!autoSwitchEnabled}
                className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors focus:outline-none focus:ring-2 focus:ring-teal-500 focus:ring-offset-2 ${
                  !autoSwitchEnabled
                    ? "field-toggle cursor-not-allowed"
                    : manualConfigEnabled
                    ? "bg-teal-600"
                    : "field-toggle"
                }`}
                title={
                  !autoSwitchEnabled
                    ? "请先启用自动轮换功能"
                    : manualConfigEnabled
                    ? "点击关闭手动配置模式"
                    : "点击开启手动配置模式"
                }
              >
                <span
                  className={`field-toggle-thumb h-4 w-4 transform ${
                    manualConfigEnabled ? "translate-x-6" : "translate-x-1"
                  }`}
                />
              </button>
            </div>

            {/* 手动配置推荐提示 */}
            {autoSwitchEnabled && !manualConfigEnabled && (
              <div className="status-warning rounded-md border p-3 text-xs">
                <p className="font-medium">💡 推荐开启手动配置模式：</p>
                <ul className="mt-1 ml-4 space-y-1 list-disc">
                  <li>⚡ 切换速度更快，无需等待用量查询</li>
                  <li>✅ 无需填写复杂的work_os_session_token</li>
                  <li>🎯 精确控制轮换顺序，按您的设置执行</li>
                </ul>
              </div>
            )}

            {/* 说明 */}
            {autoSwitchEnabled && (
              <div className="status-info rounded-md p-3 text-xs">
                <p className="font-medium">💡 工作原理：</p>
                <ul className="mt-1 ml-4 space-y-1 list-disc">
                  <li>检测到用量限制错误时自动触发</li>
                  {manualConfigEnabled ? (
                    <>
                      <li>
                        🎯 <strong>手动配置模式（推荐）</strong>
                        ：优先使用用户手动设置的账户
                      </li>
                      <li>
                        ⚡ 直接切换到第一个标记为"自动轮换"的账户，速度极快
                      </li>
                      <li>
                        ✅ 无需获取用量数据，无需填写work_os_session_token
                      </li>
                      <li>🔄 如果没有手动配置的账户，回退到自动模式</li>
                    </>
                  ) : (
                    <>
                      <li>
                        🤖 <strong>自动模式</strong>
                        ：优先选择试用版账户，其次是Pro账户
                      </li>
                      <li>选择费用低于阈值的第一个可用账户</li>
                      <li>⚠️ 账户需要填写work_os_session_token才能获取用量</li>
                      <li>⏳ 需要查询用量数据，切换速度较慢</li>
                    </>
                  )}
                  <li>自动执行机器码重置和账户切换</li>
                </ul>
              </div>
            )}

            {/* 操作按钮区域 */}
            {true && (
              <div className="flex items-start space-x-4">
                {/* 刷新缓存按钮 */}
                <div className="flex-1">
                  <button
                    type="button"
                    onClick={handleRefreshCache}
                    disabled={refreshCacheLoading}
                    className={`inline-flex items-center px-4 py-2 text-sm font-medium text-white border border-transparent rounded-md focus:outline-none focus:ring-2 focus:ring-offset-2 ${
                      refreshCacheLoading
                        ? "bg-slate-400 cursor-not-allowed dark:bg-slate-700"
                        : "bg-blue-600 hover:bg-blue-700 focus:ring-blue-500"
                    }`}
                  >
                    {refreshCacheLoading ? "🔄 刷新中..." : "🔄 刷新账户缓存"}
                  </button>
                  <p className="mt-2 text-xs text-teal-600">
                    手动刷新符合条件的账户缓存（应用启动时会自动刷新）
                  </p>
                </div>

                {/* 保存配置按钮 */}
                <div className="flex-1">
                  <button
                    type="button"
                    onClick={saveAutoSwitchConfig}
                    disabled={autoSwitchLoading}
                    className={`inline-flex items-center px-4 py-2 text-sm font-medium text-white border border-transparent rounded-md focus:outline-none focus:ring-2 focus:ring-offset-2 ${
                      autoSwitchLoading
                        ? "bg-slate-400 cursor-not-allowed dark:bg-slate-700"
                        : "bg-teal-600 hover:bg-teal-700 focus:ring-teal-500"
                    }`}
                  >
                    {autoSwitchLoading ? "🔄 保存中..." : "💾 保存配置"}
                  </button>
                  <p className="mt-2 text-xs text-teal-600">
                    保存所有自动轮换配置（启用状态、费用阈值、手动配置模式）
                  </p>
                </div>
              </div>
            )}
          </div>
        </div>

        {/* <div className="p-3 bg-teal-100 rounded-md">
          <h5 className="mb-2 text-sm font-medium text-teal-800 dark:text-teal-200">
            📋 API接口说明
          </h5>
          <div className="space-y-1 text-xs text-teal-700 dark:text-teal-300">
            <p>
              <strong>GET</strong> http://127.0.0.1:{webServerPort}
              /api/seamless-switch/config
            </p>
            <p className="ml-4">获取当前无感换号配置（token和切换状态）</p>
            <p>
              <strong>POST</strong> http://127.0.0.1:{webServerPort}
              /api/seamless-switch/token
            </p>
            <p className="ml-4">
              更新无感换号token，body:{" "}
              {JSON.stringify({ accessToken: "your_token" })}
            </p>
            <p>
              <strong>GET</strong> http://127.0.0.1:{webServerPort}
              /health
            </p>
            <p className="ml-4">健康检查接口</p>
          </div>
        </div> */}

        {/* <div className="p-3 border rounded-md border-amber-200 bg-amber-50">
          <h5 className="mb-2 text-sm font-medium text-amber-800">
            ⚠️ 使用说明
          </h5>
          <div className="space-y-1 text-xs text-amber-700">
            <p>• 启用无感换号后，切换账户时会自动更新Web配置文件</p>
            <p>• 外部应用可通过API接口获取当前token和切换状态</p>
            <p>• isSwitch=1表示需要切换，=0表示正常状态</p>
            <p>• 配置文件: seamless_switch_config.json</p>
          </div>
        </div> */}
      </div>
    </div>
  );
};
