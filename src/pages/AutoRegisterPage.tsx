import React, { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Button } from "../components/Button";
import { LoadingSpinner } from "../components/LoadingSpinner";
import { Toast } from "../components/Toast";
import { BankCardConfigModal } from "../components/BankCardConfigModal";
import { EmailConfigModal } from "../components/EmailConfigModal";
import { AccountService } from "../services/accountService";
import { BankCardConfigService } from "../services/bankCardConfigService";
import { EmailConfigService } from "../services/emailConfigService";
import { CursorService } from "../services/cursorService";
import { useProxyStore } from "../stores/proxyStore";
import { useConfigStore } from "../stores/configStore";
import type { EmailType, RegistrationProvider } from "../stores/configStore";
import { BankCardConfig } from "../types/bankCardConfig";
import { EmailConfig } from "../types/emailConfig";
import { base64URLEncode, K, sha256 } from "../utils/cursorToken";
import { confirm } from "@tauri-apps/plugin-dialog";

interface RegistrationForm {
  email: string;
  firstName: string;
  lastName: string;
  password: string;
}

interface RegistrationResult {
  success: boolean | string;
  message: string;
  details?: string[];
  action?: string;
  status?: string;
  output_lines?: string[];
  raw_output?: string;
  error_output?: string;
  accountInfo?: {
    email: string;
    token: string;
    usage: string;
  };
}

interface BatchProgressEventPayload {
  provider?: string;
  mode?: string;
  index?: number;
  email?: string;
  success?: boolean;
  error?: string;
  completed?: number;
  total?: number;
  succeeded?: number;
  failed?: number;
}

interface CodexManualActionPayload {
  task_id?: string | null;
  email?: string | null;
  reason?: string;
  message?: string;
  status?: string;
  continue_file?: string | null;
  cancel_file?: string | null;
}

type CodexCdpStep =
  | {
      type: "click";
      selector: string;
      waitForLoad?: boolean;
    }
  | {
      type: "input";
      selector: string;
      value: string;
    };

interface CodexCdpOverrides {
  url?: string;
  steps?: CodexCdpStep[];
  waitAfterOpen?: number;
  waitAfterAction?: number;
  elementTimeout?: number;
  postOauthStep1Py?: string;
}

const DEFAULT_CODEX_CDP_OVERRIDES: CodexCdpOverrides = {
  url: "https://auth.openai.com/log-in/",
  steps: [
    { type: "click", selector: "css:a", waitForLoad: true },
    {
      type: "click",
      selector: "css:a[data-discover='true']",
      waitForLoad: true,
    },
    {
      type: "input",
      selector: "css:input[type='email']",
      value: "__REGISTER_EMAIL__",
    },
    { type: "click", selector: "css:button[type='submit']" },
    {
      type: "input",
      selector: "css:input[type='password']",
      value: "__ACCESS_PASSWORD__",
    },
    { type: "click", selector: "css:button[type='submit']" },
    { type: "input", selector: "@name=otp", value: "__AUTO__" },
    { type: "input", selector: "@name=name", value: "__RANDOM_EN_NAME__" },
    { type: "click", selector: "css:button[type='submit']" },
    { type: "click", selector: "css:button[type='submit']" },
    { type: "input", selector: "@name=age", value: "25" },
    { type: "click", selector: "@name=allCheckboxes" },
    { type: "click", selector: "css:button[type='submit']" },
    {
      type: "click",
      selector:
        "xpath://button[@type='submit' and (contains(normalize-space(.), 'Yes') or contains(normalize-space(.), '确定'))]",
    },
  ],
  elementTimeout: 20,
  waitAfterAction: 1.8,
  postOauthStep1Py: "openai_oauth_step1.py",
};

const DEFAULT_CODEX_CDP_OVERRIDES_JSON = JSON.stringify(
  DEFAULT_CODEX_CDP_OVERRIDES,
  null,
  2,
);
const DEFAULT_BATCH_SERIAL_DELAY_SECONDS = 10;

const isRegistrationSuccessResult = (
  result: RegistrationResult | null,
): boolean => {
  if (!result) return false;

  if (result.success === true || result.success === "completed") return true;
  if (result.message === "注册成功") return true;

  const outputText = [
    result.message || "",
    result.raw_output || "",
    ...(result.output_lines || []),
  ].join("\n");

  // 兜底：即使进程文案提示退出，只要已产出 token 文件/成功标记，仍视为成功。
  return (
    /token_[^/\s]+\.json/i.test(outputText) ||
    /OAuth Step2 成功/i.test(outputText) ||
    /已生成.*token/i.test(outputText)
  );
};

export const AutoRegisterPage: React.FC = () => {
  const [form, setForm] = useState<RegistrationForm>({
    email: "",
    firstName: "",
    lastName: "",
    password: "",
  });
  const [registrationProvider, setRegistrationProvider] =
    useState<RegistrationProvider>("cursor");
  const [emailType, setEmailType] = useState<EmailType>("custom");
  const [outlookMode, setOutlookMode] = useState<"default" | "token">(
    "default",
  );
  const [outlookEmail, setOutlookEmail] = useState("");

  // Tempmail配置相关状态
  const [tempmailEmail, setTempmailEmail] = useState("");
  const [tempmailPin, setTempmailPin] = useState("");

  /** 自建邮箱 API：与 Rust 中 GET 拉取邮件列表一致 */
  const [selfHostedMailUrl, setSelfHostedMailUrl] = useState("");
  const [selfHostedMailHeadersJson, setSelfHostedMailHeadersJson] = useState(
    '{\n  "Authorization": "Bearer ",\n  "Content-Type": "application/json"\n}',
  );
  const [selfHostedMailResponsePath, setSelfHostedMailResponsePath] =
    useState("results[0].raw");
  const [selfHostedMailClearEnabled, setSelfHostedMailClearEnabled] =
    useState(false);
  const [selfHostedMailClearUrl, setSelfHostedMailClearUrl] = useState("");
  const [selfHostedMailClearHeadersJson, setSelfHostedMailClearHeadersJson] =
    useState(
      '{\n  "Authorization": "Bearer ",\n  "Content-Type": "application/json"\n}',
    );
  const [selfHostedMailClearMethod, setSelfHostedMailClearMethod] =
    useState("DELETE");
  const [codexCdpOverridesJson, setCodexCdpOverridesJson] = useState(
    DEFAULT_CODEX_CDP_OVERRIDES_JSON,
  );

  const [useIncognito, setUseIncognito] = useState(true);
  const [enableBankCardBinding, setEnableBankCardBinding] = useState(true);
  const [skipPhoneVerification, setSkipPhoneVerification] = useState(false);
  const [isUsAccount, setIsUsAccount] = useState(false); // 美国账户选项

  // 代理配置相关状态 - 使用 Zustand store
  const {
    enabled: proxyEnabled,
    proxyType,
    httpProxy,
    socksProxy,
    vlessUrl,
    xrayHttpPort,
    xraySocksPort,
    noProxy,
    setEnabled: setProxyEnabled,
    setProxyType,
    setHttpProxy,
    setSocksProxy,
    setVlessUrl,
    setXrayHttpPort,
    setXraySocksPort,
    setNoProxy,
    resetToDefaults: resetProxyConfig,
    getProxyConfig,
  } = useProxyStore();

  // 缓存配置相关状态 - 使用 Config store
  const {
    getPinCode,
    setPinCode,
    getTempmailEmail,
    setTempmailEmail: setCachedTempmailEmail,
    getEmailType,
    setEmailType: setCachedEmailType,
    getUseIncognito,
    setUseIncognito: setCachedUseIncognito,
    getEnableBankCardBinding,
    setEnableBankCardBinding: setCachedEnableBankCardBinding,
    getUseParallelMode,
    setUseParallelMode: setCachedUseParallelMode,
    getBatchSerialDelaySeconds,
    setBatchSerialDelaySeconds: setCachedBatchSerialDelaySeconds,
    getSelfHostedMailUrl,
    setSelfHostedMailUrl: setCachedSelfHostedMailUrl,
    getSelfHostedMailHeadersJson,
    setSelfHostedMailHeadersJson: setCachedSelfHostedMailHeadersJson,
    getSelfHostedMailResponsePath,
    setSelfHostedMailResponsePath: setCachedSelfHostedMailResponsePath,
    getSelfHostedMailClearEnabled,
    setSelfHostedMailClearEnabled: setCachedSelfHostedMailClearEnabled,
    getSelfHostedMailClearUrl,
    setSelfHostedMailClearUrl: setCachedSelfHostedMailClearUrl,
    getSelfHostedMailClearHeadersJson,
    setSelfHostedMailClearHeadersJson: setCachedSelfHostedMailClearHeadersJson,
    getSelfHostedMailClearMethod,
    setSelfHostedMailClearMethod: setCachedSelfHostedMailClearMethod,
    getCodexSelfHostedMailUrl,
    setCodexSelfHostedMailUrl: setCachedCodexSelfHostedMailUrl,
    getCodexSelfHostedMailHeadersJson,
    setCodexSelfHostedMailHeadersJson: setCachedCodexSelfHostedMailHeadersJson,
    getCodexSelfHostedMailResponsePath,
    setCodexSelfHostedMailResponsePath:
      setCachedCodexSelfHostedMailResponsePath,
    getCodexSelfHostedMailClearEnabled,
    setCodexSelfHostedMailClearEnabled:
      setCachedCodexSelfHostedMailClearEnabled,
    getCodexSelfHostedMailClearUrl,
    setCodexSelfHostedMailClearUrl: setCachedCodexSelfHostedMailClearUrl,
    getCodexSelfHostedMailClearHeadersJson,
    setCodexSelfHostedMailClearHeadersJson:
      setCachedCodexSelfHostedMailClearHeadersJson,
    getCodexSelfHostedMailClearMethod,
    setCodexSelfHostedMailClearMethod: setCachedCodexSelfHostedMailClearMethod,
    getCodexCdpOverridesJson,
    setCodexCdpOverridesJson: setCachedCodexCdpOverridesJson,
    getRegistrationProvider,
    setRegistrationProvider: setCachedRegistrationProvider,
  } = useConfigStore();

  // 订阅配置相关状态
  const [subscriptionTier, setSubscriptionTier] = useState<
    "pro" | "pro_plus" | "ultra"
  >("pro");
  const [allowAutomaticPayment, setAllowAutomaticPayment] = useState(true);
  const [allowTrial, setAllowTrial] = useState(true);
  const [useApiForBindCard, setUseApiForBindCard] = useState<1 | 2>(1); // 1为使用接口，2为不使用接口（模拟点击）
  const [isLoading, setIsLoading] = useState(false);
  const [isTestingVlessProxy, setIsTestingVlessProxy] = useState(false);
  const [vlessRuntimeProxy, setVlessRuntimeProxy] = useState<{
    httpProxy: string;
    socksProxy: string;
    httpPort: number;
    socksPort: number;
  } | null>(null);
  const [vlessDownloadProgress, setVlessDownloadProgress] = useState<{
    stage: string;
    message: string;
    percent: number | null;
    receivedBytes: number;
    totalBytes: number | null;
  } | null>(null);
  const [toast, setToast] = useState<{
    message: string;
    type: "success" | "error" | "info";
  } | null>(null);
  const [registrationResult, setRegistrationResult] =
    useState<RegistrationResult | null>(null);
  const [useRandomInfo, setUseRandomInfo] = useState(true);
  const [showPassword, setShowPassword] = useState(false);
  const [showVerificationModal, setShowVerificationModal] = useState(false);
  const [verificationCode, setVerificationCode] = useState("");
  const [codexManualAction, setCodexManualAction] =
    useState<CodexManualActionPayload | null>(null);
  const [manualActionLoading, setManualActionLoading] = useState(false);
  const [currentTaskId, setCurrentTaskId] = useState<string | null>(null); // 当前需要验证码的任务ID
  const [currentTaskEmail, setCurrentTaskEmail] = useState<string | null>(null); // 当前需要验证码的任务邮箱
  const [realtimeOutput, setRealtimeOutput] = useState<string[]>([]);
  const [isRegistering, setIsRegistering] = useState(false);
  const isRegisteringRef = useRef(false);
  const realtimeOutputRef = useRef<string[]>([]);
  const batchCountRef = useRef(1);
  const isBatchRegisteringRef = useRef(false);
  const batchProgressEntriesRef = useRef<Map<number, string>>(new Map());
  const cancelledStopTaskIdsRef = useRef<Set<string>>(new Set());
  const registrationRunIdRef = useRef(0);
  const currentTaskIdRef = useRef<string | null>(null);
  const currentTaskEmailRef = useRef<string | null>(null);
  const [showBankCardConfig, setShowBankCardConfig] = useState(false);
  const [bankCardConfig, setBankCardConfig] = useState<BankCardConfig | null>(
    null,
  );
  const [showEmailConfig, setShowEmailConfig] = useState(false);
  const [emailConfig, setEmailConfig] = useState<EmailConfig | null>(null);

  // 浏览器路径配置相关状态
  const [showBrowserConfig, setShowBrowserConfig] = useState(false);
  const [customBrowserPath, setCustomBrowserPath] = useState<string>("");
  const [currentBrowserPath, setCurrentBrowserPath] = useState<string | null>(
    null,
  );

  // 批量注册相关状态
  const [batchCount, setBatchCount] = useState(1);
  const [batchSerialDelaySeconds, setBatchSerialDelaySeconds] = useState(
    DEFAULT_BATCH_SERIAL_DELAY_SECONDS,
  );
  const [batchEmails, setBatchEmails] = useState<string[]>([""]);
  const [useParallelMode, setUseParallelMode] = useState(true); // 并行模式开关，默认开启

  // 银行卡选择相关状态
  const [bankCardList, setBankCardList] = useState<BankCardConfig[]>([]);
  const [selectedCardIndex, setSelectedCardIndex] = useState<number>(0); // 单个注册：默认选中第一张
  const [selectedBatchCardIndices, setSelectedBatchCardIndices] = useState<
    number[]
  >([0]); // 批量注册：默认选中第一张

  // 同步ref和state
  useEffect(() => {
    isRegisteringRef.current = isRegistering;
  }, [isRegistering]);

  useEffect(() => {
    batchCountRef.current = batchCount;
  }, [batchCount]);

  useEffect(() => {
    currentTaskIdRef.current = currentTaskId;
  }, [currentTaskId]);

  useEffect(() => {
    currentTaskEmailRef.current = currentTaskEmail;
  }, [currentTaskEmail]);

  const showCodexManualStepPrompt = (
    payload: Partial<CodexManualActionPayload> = {},
  ) => {
    const fileTaskId =
      payload.continue_file || payload.cancel_file
        ? resolveCodexManualTaskId(payload as CodexManualActionPayload)
        : null;
    const taskId =
      payload.task_id !== undefined
        ? payload.task_id
        : fileTaskId || currentTaskIdRef.current;
    const email =
      payload.email !== undefined ? payload.email : currentTaskEmailRef.current;

    if (taskId) {
      setCurrentTaskId(taskId);
    }
    if (email) {
      setCurrentTaskEmail(email);
    }

    setCodexManualAction({
      task_id: taskId ?? null,
      email: email ?? null,
      reason: payload.reason || "codex_manual_confirm_registration_complete",
      message:
        payload.message ||
        "当前已经到 Step1 自动执行前的节点。如果自动注册没成功，请先在浏览器里手动完成注册，然后点击“手动确认注册完成并执行 Step1”；如果你确认这一步可以跳过，也可以直接继续。",
      status: payload.status || "waiting",
      continue_file: payload.continue_file ?? null,
      cancel_file: payload.cancel_file ?? null,
    });
  };

  const resolveCodexManualTaskId = (
    payload: CodexManualActionPayload | null,
  ): string | null => {
    if (payload?.task_id?.trim()) {
      return payload.task_id.trim();
    }

    const filePath = payload?.continue_file || payload?.cancel_file || "";
    const match = filePath.match(
      /(?:cdp_flow_continue_|cursor_registration_stop_)([^\\\/\s.]+)\.txt/i,
    );
    return match?.[1]?.trim() || null;
  };

  useEffect(() => {
    if (showVerificationModal) {
      // 弹窗提示
      confirm(
        "请手动输入验证码并请确认页面已经在输入验证码页面否则输入无效！",
        {
          title: "提示！",
          kind: "info",
        },
      );
    }
  }, [showVerificationModal]);

  // 根据webToken获取客户端assToken
  const getClientAccessToken = (workos_cursor_session_token: string) => {
    return new Promise(async (resolve, _reject) => {
      let verifier = base64URLEncode(K);
      let challenge = base64URLEncode(new Uint8Array(await sha256(verifier)));
      let uuid = crypto.randomUUID();
      // 轮询查token
      let interval = setInterval(() => {
        invoke("trigger_authorization_login_poll", {
          uuid,
          verifier,
        }).then((res: any) => {
          console.log(res, "res");
          if (res.success) {
            const data = JSON.parse(res.response_body);
            console.log(data, "data");
            resolve(data);
            setToast({ message: "token获取成功", type: "success" });
            clearInterval(interval);
          }
        });
      }, 1000);

      // 60秒后清除定时器
      setTimeout(() => {
        clearInterval(interval);
        resolve(null);
      }, 1000 * 20);

      // 触发授权登录-rust
      invoke("trigger_authorization_login", {
        uuid,
        challenge,
        workosCursorSessionToken: workos_cursor_session_token,
      });
    });
  };

  // 监听实时输出事件
  useEffect(() => {
    console.log("设置事件监听器...");
    const setupListeners = async () => {
      // 监听注册输出
      const unlistenOutput = await listen(
        "registration-output",
        async (event: any) => {
          console.log("收到实时输出事件:", event.payload);
          const data = event.payload;
          if (
            typeof data.line === "string" &&
            data.line.trim().startsWith("{")
          ) {
            try {
              const eventPayload = JSON.parse(data.line);
              if (
                eventPayload?.action === "wait_for_user" &&
                eventPayload?.reason ===
                  "codex_manual_confirm_registration_complete"
              ) {
                showCodexManualStepPrompt({
                  task_id: currentTaskIdRef.current,
                  email: currentTaskEmailRef.current,
                  reason: eventPayload.reason,
                  continue_file: eventPayload.continue_file,
                  cancel_file: eventPayload.cancel_file,
                  message:
                    eventPayload.message ||
                    "已到 Step1 自动执行前的节点。若你已手动完成注册，可点击“手动确认注册完成”继续下一步。",
                  status: eventPayload.status,
                });
                setToast({
                  message: "已到 Step1 前暂停点，可手动确认后继续下一步",
                  type: "info",
                });
              }
            } catch {
              // Ignore non-JSON log lines
            }
          }
          if (data.line.includes("wuqi666")) {
            const wuqi: any = JSON.parse(data.line);

            try {
              const acknowledgeResult = await invoke(
                "acknowledge_grace_period",
                {
                  workosCursorSessionToken: wuqi.wuqi666 || "",
                },
              );
              console.log("Acknowledge结果:", acknowledgeResult);
              setToast({ message: "已确认宽限期免责声明", type: "success" });
            } catch (error) {
              console.error("确认宽限期免责声明失败:", error);
              setToast({
                message: `确认宽限期免责声明失败: ${error}`,
                type: "error",
              });
            }
          }
          if (
            data.line.includes("workos_cursor_session_token") &&
            data.line.includes("token") &&
            data.line.includes("user_")
          ) {
            const resObj: any = JSON.parse(data.line);
            getClientAccessToken(resObj.workos_cursor_session_token).then(
              async (res: any) => {
                try {
                  const result = await AccountService.addAccount(
                    resObj.email,
                    res.accessToken,
                    res.refreshToken,
                    resObj.workos_cursor_session_token || undefined,
                  );
                  if (result.success) {
                    setToast({ message: "账户添加成功", type: "success" });
                  } else {
                    setToast({ message: result.message, type: "error" });
                  }
                } catch (error) {
                  console.error("Failed to add account:", error);
                  setToast({ message: "添加账户失败", type: "error" });
                }
                console.log(res.accessToken, "res.accessToken");
              },
            );
          }

          if (data.line.includes("程序将保持运行状态")) {
            // 提示用户手动输入绑卡地址，确认后将自动关闭浏览器进程进行下一个任务
            // 如果是批量注册（2个及以上），3秒后自动发送停止信号，不显示阻塞弹窗
            // 使用 ref 获取最新的 batchCount，避免读取到闭包中的旧值
            const isBatchRegistration = isBatchRegisteringRef.current;

            const sendCancelSignal = async (silent = false) => {
              try {
                await invoke("cancel_registration");
                if (!silent) {
                  setToast({
                    message: "已发送终止信号，浏览器进程将自动关闭",
                    type: "success",
                  });
                }
                console.log("✅ 已发送终止信号，等待进程退出");
              } catch (error) {
                console.error("发送取消信号失败:", error);
                if (!silent) {
                  setToast({
                    message: `发送取消信号失败: ${error}`,
                    type: "error",
                  });
                }
              }
            };

            try {
              if (isBatchRegistration) {
                // 批量注册模式：不直接取消全部；等待 Python 输出“停止信号文件...”后再只关闭当前窗口
                console.log(
                  "⏳ 批量注册：检测到保持运行状态，等待停止信号文件行再关闭当前窗口",
                );
              } else {
                // 单个注册模式：正常等待用户确认
                const confirmed = await confirm(
                  "程序将保持运行状态，确认后将自动关闭浏览器进程并进行下一个任务",
                  {
                    title: "程序将保持运行状态",
                    kind: "info",
                  },
                );

                if (confirmed) {
                  await sendCancelSignal();
                } else {
                  setToast({ message: "已取消", type: "info" });
                }
              }
            } catch (error) {
              console.error("弹窗确认失败:", error);
              setToast({ message: "弹窗确认失败，请重试", type: "error" });
              return;
            }
          }

          // 批量注册：只关闭当前等待的注册窗口
          if (
            isBatchRegisteringRef.current &&
            typeof data.line === "string" &&
            data.line.includes("停止信号文件:")
          ) {
            // 兼容不同命名拼写（例如 registrration 的轻微差异）
            // 直接从文件名中抓取：...stop_<taskId>.txt
            const match = data.line.match(/stop_([^\\\/\s]+)\.txt/);
            const taskId = match?.[1]?.trim();
            console.log("🔍 收到停止信号 - task_id:", taskId);

            if (taskId && !cancelledStopTaskIdsRef.current.has(taskId)) {
              cancelledStopTaskIdsRef.current.add(taskId);
              try {
                // 给 Python 先把旧 stop_file 清理完，避免刚写入就被当作旧文件删掉
                await new Promise((r) => setTimeout(r, 800));
                console.log("🔍 写入停止信号: task_id:", taskId);
                await invoke("cancel_registration_task", {
                  taskId: taskId,
                });
              } catch (e) {
                console.error("cancel_registration_task 失败:", e);
              }
            }
          }

          // 同时更新ref和state
          realtimeOutputRef.current = [...realtimeOutputRef.current, data.line];
          setRealtimeOutput((prev) => [...prev, data.line]);
          console.log("更新输出，当前行数:", realtimeOutputRef.current.length);

          console.log("触发状态更新");
        },
      );

      // 监听验证码请求
      const unlistenVerification = await listen(
        "verification-code-required",
        (event: any) => {
          // 只有在正在注册时才显示验证码弹窗
          if (isRegisteringRef.current) {
            const payload = event.payload;
            console.log("🔍 需要验证码:", payload);

            // 解析 task_id 和 email
            if (typeof payload === "object" && payload !== null) {
              const taskId = payload.task_id || null;
              const email = payload.email || null;
              console.log(
                `📝 保存任务信息 - Task ID: ${taskId}, Email: ${email}`,
              );
              setCurrentTaskId(taskId);
              setCurrentTaskEmail(email);
            }

            setShowVerificationModal(true);
            setToast({ message: "请输入验证码", type: "info" });
          }
        },
      );

      // 监听验证码获取超时
      const unlistenVerificationTimeout = await listen(
        "verification-code-timeout",
        (event: any) => {
          const payload = event.payload;
          console.log("🔍 验证码获取超时:", payload);

          // 解析 task_id 和 email
          if (typeof payload === "object" && payload !== null) {
            const taskId = payload.task_id || null;
            const email = payload.email || null;
            console.log(
              `📝 保存任务信息 - Task ID: ${taskId}, Email: ${email}`,
            );
            setCurrentTaskId(taskId);
            setCurrentTaskEmail(email);
          }

          setShowVerificationModal(true);
          setToast({
            message: `自动获取验证码超时，请手动输入验证码${
              payload.email ? ` (${payload.email})` : ""
            }`,
            type: "info",
          });
        },
      );

      // 监听自动获取的验证码
      const unlistenAutoCode = await listen(
        "verification-code-auto-filled",
        (event: any) => {
          const code = event.payload;
          console.log("🎯 收到自动获取的验证码:", code);
          setVerificationCode(code);
          setToast({ message: `自动获取验证码成功: ${code}`, type: "success" });
        },
      );

      // 监听验证码获取失败
      const unlistenCodeFailed = await listen(
        "verification-code-failed",
        (event: any) => {
          const error = event.payload;
          console.log("❌ 自动获取验证码失败:", error);
          setToast({ message: `自动获取验证码失败: ${error}`, type: "error" });
        },
      );

      // 监听需要手动输入验证码
      const unlistenManualInput = await listen(
        "verification-code-manual-input-required",
        (event: any) => {
          const payload = event.payload;
          console.log("🔍 需要手动输入验证码:", payload);

          // 解析 task_id 和 email
          if (typeof payload === "object" && payload !== null) {
            const taskId = payload.task_id || null;
            const email = payload.email || null;
            console.log(
              `📝 保存任务信息 - Task ID: ${taskId}, Email: ${email}`,
            );
            setCurrentTaskId(taskId);
            setCurrentTaskEmail(email);
          }

          setShowVerificationModal(true);
          setToast({
            message: `自动获取验证码失败，请手动输入验证码${
              payload.email ? ` (${payload.email})` : ""
            }`,
            type: "info",
          });
        },
      );

      console.log("事件监听器设置完成");

      const unlistenBatchProgress = await listen(
        "batch-registration-progress",
        (event: any) => {
          if (!isBatchRegisteringRef.current) {
            return;
          }
          updateBatchRegistrationProgress(
            (event.payload || {}) as BatchProgressEventPayload,
          );
        },
      );

      const unlistenCodexManualStep = await listen(
        "codex-manual-step-required",
        (event: any) => {
          if (!isRegisteringRef.current) {
            return;
          }
          const payload = (event.payload || {}) as CodexManualActionPayload;
          if (payload.reason !== "codex_manual_confirm_registration_complete") {
            return;
          }
          showCodexManualStepPrompt(payload);
          setToast({
            message:
              payload.message ||
              "已到 Step1 自动执行前的节点，可手动确认后继续下一步",
            type: "info",
          });
        },
      );

      return () => {
        unlistenOutput();
        unlistenVerification();
        unlistenAutoCode();
        unlistenCodeFailed();
        unlistenManualInput();
        unlistenVerificationTimeout();
        unlistenBatchProgress();
        unlistenCodexManualStep();
      };
    };

    let cleanup: (() => void) | undefined;

    setupListeners().then((cleanupFn) => {
      cleanup = cleanupFn;
    });

    return () => {
      console.log("清理事件监听器");
      if (cleanup) {
        cleanup();
      }
    };
  }, []); // 确保只运行一次

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    const setup = async () => {
      unlisten = await listen("vless-xray-download-progress", (event: any) => {
        const payload = event.payload || {};
        setVlessDownloadProgress({
          stage: payload.stage || "downloading",
          message: payload.message || "正在下载 xray...",
          percent:
            typeof payload.percent === "number" &&
            Number.isFinite(payload.percent)
              ? payload.percent
              : null,
          receivedBytes:
            typeof payload.receivedBytes === "number"
              ? payload.receivedBytes
              : 0,
          totalBytes:
            typeof payload.totalBytes === "number" ? payload.totalBytes : null,
        });
      });
    };
    setup();
    return () => {
      if (unlisten) {
        unlisten();
      }
    };
  }, []);

  // 加载银行卡列表
  useEffect(() => {
    const loadBankCardList = async () => {
      try {
        const configList = await BankCardConfigService.getBankCardConfigList();
        setBankCardList(configList.cards);
        // 默认选中第一张卡
        if (configList.cards.length > 0) {
          setSelectedCardIndex(0);
          setSelectedBatchCardIndices([0]);
        }
      } catch (error) {
        console.error("加载银行卡列表失败:", error);
      }
    };
    loadBankCardList();
  }, []);

  const generateRandomInfo = () => {
    const firstNames = [
      "Alex",
      "Jordan",
      "Taylor",
      "Casey",
      "Morgan",
      "Riley",
      "Avery",
      "Quinn",
    ];
    const lastNames = [
      "Smith",
      "Johnson",
      "Williams",
      "Brown",
      "Jones",
      "Garcia",
      "Miller",
      "Davis",
    ];

    const firstName = firstNames[Math.floor(Math.random() * firstNames.length)];
    const lastName = lastNames[Math.floor(Math.random() * lastNames.length)];

    // Generate random password
    const chars =
      "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!@#$%^&*";
    let password = "";
    for (let i = 0; i < 12; i++) {
      password += chars.charAt(Math.floor(Math.random() * chars.length));
    }

    setForm((prev) => ({
      ...prev, // 保留邮箱地址不变
      firstName,
      lastName,
      password,
    }));
  };

  const handleInputChange = (field: keyof RegistrationForm, value: string) => {
    setForm((prev) => ({ ...prev, [field]: value }));
  };

  // 清空 Tempmail 邮箱
  const clearTempmailInbox = async (
    email: string,
    pin?: string,
  ): Promise<boolean> => {
    try {
      console.log(`🗑️ 清空 Tempmail 邮箱: ${email}`);

      // 调用 Rust 后端命令
      const result = await invoke<{ success: boolean; message: string }>(
        "clear_tempmail_inbox",
        {
          email: email,
          pin: pin || null,
        },
      );

      if (result.success) {
        console.log("✅ Tempmail 邮箱清空成功");
        return true;
      } else {
        console.warn(`⚠️ ${result.message}`);
        return false;
      }
    } catch (error) {
      console.error("❌ 清空 Tempmail 邮箱失败:", error);
      return false;
    }
  };

  const parseCodexCdpOverrides = (): CodexCdpOverrides | null => {
    try {
      const parsed = JSON.parse(codexCdpOverridesJson) as unknown;
      if (
        typeof parsed !== "object" ||
        parsed === null ||
        Array.isArray(parsed)
      ) {
        throw new Error("顶层必须是 JSON 对象");
      }

      const overrides = parsed as CodexCdpOverrides;
      if (overrides.url !== undefined && typeof overrides.url !== "string") {
        throw new Error("url 必须是字符串");
      }
      if (
        overrides.waitAfterOpen !== undefined &&
        typeof overrides.waitAfterOpen !== "number"
      ) {
        throw new Error("waitAfterOpen 必须是数字");
      }
      if (
        overrides.waitAfterAction !== undefined &&
        typeof overrides.waitAfterAction !== "number"
      ) {
        throw new Error("waitAfterAction 必须是数字");
      }
      if (
        overrides.elementTimeout !== undefined &&
        typeof overrides.elementTimeout !== "number"
      ) {
        throw new Error("elementTimeout 必须是数字");
      }
      if (
        overrides.postOauthStep1Py !== undefined &&
        typeof overrides.postOauthStep1Py !== "string"
      ) {
        throw new Error("postOauthStep1Py 必须是字符串");
      }

      if (overrides.steps !== undefined) {
        if (!Array.isArray(overrides.steps)) {
          throw new Error("steps 必须是数组");
        }
        overrides.steps.forEach((step, index) => {
          if (!step || typeof step !== "object") {
            throw new Error(`steps[${index}] 必须是对象`);
          }
          if (step.type !== "click" && step.type !== "input") {
            throw new Error(`steps[${index}].type 仅支持 click 或 input`);
          }
          if (typeof step.selector !== "string" || !step.selector.trim()) {
            throw new Error(`steps[${index}].selector 不能为空字符串`);
          }
          if (
            "waitForLoad" in step &&
            step.waitForLoad !== undefined &&
            typeof step.waitForLoad !== "boolean"
          ) {
            throw new Error(`steps[${index}].waitForLoad 必须是布尔值`);
          }
          if (step.type === "input" && typeof step.value !== "string") {
            throw new Error(`steps[${index}].value 必须是字符串`);
          }
        });
      }

      return overrides;
    } catch (error) {
      setToast({
        message: `Codex CDP 配置 JSON 无效: ${error}`,
        type: "error",
      });
      return null;
    }
  };

  const beginRegistrationRun = () => {
    registrationRunIdRef.current += 1;
    return registrationRunIdRef.current;
  };

  const invalidateActiveRegistrationRun = () => {
    registrationRunIdRef.current += 1;
  };

  const resetRegistrationUiState = () => {
    setIsLoading(false);
    setIsRegistering(false);
    isBatchRegisteringRef.current = false;
    batchProgressEntriesRef.current.clear();
    cancelledStopTaskIdsRef.current.clear();
    setShowVerificationModal(false);
    setCodexManualAction(null);
    setManualActionLoading(false);
    setVerificationCode("");
    setCurrentTaskId(null);
    setCurrentTaskEmail(null);
  };

  const updateBatchRegistrationProgress = (
    payload: BatchProgressEventPayload,
  ) => {
    const total = payload.total ?? batchCountRef.current;
    const completed = payload.completed ?? 0;
    const succeeded = payload.succeeded ?? 0;
    const failed = payload.failed ?? 0;
    const index = payload.index ?? 0;
    const email = payload.email || `任务 ${index + 1}`;
    const detail = payload.success
      ? `✅[${index + 1}] ${email}: 成功`
      : `❌[${index + 1}] ${email}: ${payload.error || "失败"}`;

    batchProgressEntriesRef.current.set(index, detail);
    const details = Array.from(batchProgressEntriesRef.current.entries())
      .sort((a, b) => a[0] - b[0])
      .map(([, value]) => value);

    setRegistrationResult({
      success: failed === 0 || completed < total,
      message:
        completed >= total
          ? `批量注册完成：${succeeded}/${total} 成功`
          : `批量注册进行中：${completed}/${total} 已完成`,
      details,
    });
  };

  const handleCodexManualStepAction = async (
    action: "manual_confirm_complete" | "continue",
  ) => {
    const taskId = resolveCodexManualTaskId(codexManualAction);
    if (!taskId) {
      setToast({ message: "当前任务信息缺失，无法继续", type: "error" });
      return;
    }

    try {
      setManualActionLoading(true);
      await invoke("signal_registration_continue", {
        taskId,
        action,
      });
      setCodexManualAction(null);
      if (currentTaskIdRef.current === taskId) {
        setCurrentTaskId(null);
        setCurrentTaskEmail(null);
      }
      setToast({
        message:
          action === "manual_confirm_complete"
            ? "已发送手动确认完成信号，继续执行当前任务"
            : "已发送继续执行信号",
        type: "success",
      });
    } catch (error) {
      setToast({ message: `发送继续信号失败: ${error}`, type: "error" });
    } finally {
      setManualActionLoading(false);
    }
  };

  const handleCodexManualStepForceClose = async () => {
    const taskId = resolveCodexManualTaskId(codexManualAction);

    try {
      setManualActionLoading(true);
      if (taskId) {
        await invoke("cancel_registration_task", {
          taskId,
        });
      } else {
        await invoke("cancel_registration");
      }

      setCodexManualAction(null);
      if (taskId && currentTaskIdRef.current === taskId) {
        setCurrentTaskId(null);
        setCurrentTaskEmail(null);
      }
      setToast({
        message: taskId
          ? "已强制关闭当前任务，当前账号会按失败处理。"
          : "已停止当前注册流程。",
        type: "info",
      });
    } catch (error) {
      setToast({
        message: `强制关闭当前任务失败: ${error}`,
        type: "error",
      });
    } finally {
      setManualActionLoading(false);
    }
  };

  const handleVerificationCodeSubmit = async () => {
    if (!verificationCode || verificationCode.length !== 6) {
      setToast({ message: "请输入6位验证码", type: "error" });
      return;
    }

    try {
      // 提交验证码时传递 task_id（如果有的话）
      console.log(
        `📤 提交验证码 - Task ID: ${currentTaskId}, Email: ${currentTaskEmail}, Code: ${verificationCode}`,
      );
      await invoke("submit_verification_code", {
        code: verificationCode,
        taskId: currentTaskId, // 传递 task_id，可能为 null（单个注册模式）
      });
      setShowVerificationModal(false);
      setVerificationCode("");
      // 清除任务信息
      setCurrentTaskId(null);
      setCurrentTaskEmail(null);
      setToast({ message: "验证码已提交", type: "success" });
    } catch (error) {
      setToast({ message: `提交验证码失败: ${error}`, type: "error" });
    }
  };

  const handleCancelRegistration = async () => {
    invalidateActiveRegistrationRun();
    resetRegistrationUiState();
    try {
      await invoke("cancel_registration");
      setToast({ message: "注册已取消", type: "info" });
    } catch (error) {
      setToast({
        message: `注册状态已重置，但停止后端进程失败: ${error}`,
        type: "error",
      });
    }
  };

  // 银行卡选择切换函数（批量注册用，多选）
  const handleBatchCardSelection = (index: number) => {
    setSelectedBatchCardIndices((prev) => {
      if (prev.includes(index)) {
        // 如果已选中，则取消选中（但至少保留一个）
        if (prev.length > 1) {
          return prev.filter((i) => i !== index);
        }
        return prev;
      } else {
        // 如果未选中，则添加选中
        return [...prev, index].sort((a, b) => a - b);
      }
    });
  };

  // 银行卡选择函数（单个注册用，单选）
  const handleSingleCardSelection = (index: number) => {
    setSelectedCardIndex(index);
  };

  const isCodexProvider = registrationProvider === "codex";
  const activeProviderLabel = isCodexProvider ? "Codex" : "Cursor";
  const activeSelfHostedDescription = isCodexProvider
    ? "通过你配置的 HTTP API 拉取 Codex 验证邮件 JSON，从中取出邮件原文字符串，再解析 6 位验证码。请确保 API 返回的是该注册邮箱的最新一封邮件。"
    : "通过你配置的 HTTP API 拉取邮件 JSON，从中取出邮件原文字符串，再用与 Cloudflare 临时邮箱相同的规则解析 6 位验证码。请确保 API 返回的是最新一封邮件。";

  const switchRegistrationProvider = (provider: RegistrationProvider) => {
    setRegistrationProvider(provider);
    setCachedRegistrationProvider(provider);

    if (provider === "codex") {
      const codexEmailType: EmailType =
        emailType === "custom" || emailType === "self_hosted"
          ? emailType
          : "self_hosted";
      setEmailType(codexEmailType);
      const nextUrl =
        getCodexSelfHostedMailUrl() ?? getSelfHostedMailUrl() ?? "";
      const nextHdr =
        getCodexSelfHostedMailHeadersJson() ??
        getSelfHostedMailHeadersJson() ??
        '{\n  "Authorization": "Bearer ",\n  "Content-Type": "application/json"\n}';
      const nextPath =
        getCodexSelfHostedMailResponsePath() ??
        getSelfHostedMailResponsePath() ??
        "results[0].raw";
      const nextClearEnabled =
        getCodexSelfHostedMailClearEnabled() ??
        getSelfHostedMailClearEnabled() ??
        false;
      const nextClearUrl =
        getCodexSelfHostedMailClearUrl() ?? getSelfHostedMailClearUrl() ?? "";
      const nextClearHeaders =
        getCodexSelfHostedMailClearHeadersJson() ??
        getSelfHostedMailClearHeadersJson() ??
        '{\n  "Authorization": "Bearer ",\n  "Content-Type": "application/json"\n}';
      const nextClearMethod =
        getCodexSelfHostedMailClearMethod() ??
        getSelfHostedMailClearMethod() ??
        "DELETE";
      setSelfHostedMailUrl(nextUrl);
      setSelfHostedMailHeadersJson(nextHdr);
      setSelfHostedMailResponsePath(nextPath);
      setSelfHostedMailClearEnabled(nextClearEnabled);
      setSelfHostedMailClearUrl(nextClearUrl);
      setSelfHostedMailClearHeadersJson(nextClearHeaders);
      setSelfHostedMailClearMethod(nextClearMethod);
      return;
    }

    const cachedEmailType = getEmailType();
    setEmailType(cachedEmailType ?? "custom");
    const nextUrl = getSelfHostedMailUrl() ?? "";
    const nextHdr =
      getSelfHostedMailHeadersJson() ??
      '{\n  "Authorization": "Bearer ",\n  "Content-Type": "application/json"\n}';
    const nextPath = getSelfHostedMailResponsePath() ?? "results[0].raw";
    const nextClearEnabled = getSelfHostedMailClearEnabled() ?? false;
    const nextClearUrl = getSelfHostedMailClearUrl() ?? "";
    const nextClearHeaders =
      getSelfHostedMailClearHeadersJson() ??
      '{\n  "Authorization": "Bearer ",\n  "Content-Type": "application/json"\n}';
    const nextClearMethod = getSelfHostedMailClearMethod() ?? "DELETE";
    setSelfHostedMailUrl(nextUrl);
    setSelfHostedMailHeadersJson(nextHdr);
    setSelfHostedMailResponsePath(nextPath);
    setSelfHostedMailClearEnabled(nextClearEnabled);
    setSelfHostedMailClearUrl(nextClearUrl);
    setSelfHostedMailClearHeadersJson(nextClearHeaders);
    setSelfHostedMailClearMethod(nextClearMethod);
  };

  const persistSelfHostedConfig = () => {
    const url = selfHostedMailUrl.trim();
    const path = selfHostedMailResponsePath.trim();
    const clearUrl = selfHostedMailClearUrl.trim();
    const clearMethod =
      selfHostedMailClearMethod.trim().toUpperCase() || "DELETE";

    if (isCodexProvider) {
      setCachedCodexSelfHostedMailUrl(url);
      setCachedCodexSelfHostedMailHeadersJson(selfHostedMailHeadersJson);
      setCachedCodexSelfHostedMailResponsePath(path);
      setCachedCodexSelfHostedMailClearEnabled(selfHostedMailClearEnabled);
      setCachedCodexSelfHostedMailClearUrl(clearUrl);
      setCachedCodexSelfHostedMailClearHeadersJson(
        selfHostedMailClearHeadersJson,
      );
      setCachedCodexSelfHostedMailClearMethod(clearMethod);
      return;
    }

    setCachedSelfHostedMailUrl(url);
    setCachedSelfHostedMailHeadersJson(selfHostedMailHeadersJson);
    setCachedSelfHostedMailResponsePath(path);
    setCachedSelfHostedMailClearEnabled(selfHostedMailClearEnabled);
    setCachedSelfHostedMailClearUrl(clearUrl);
    setCachedSelfHostedMailClearHeadersJson(selfHostedMailClearHeadersJson);
    setCachedSelfHostedMailClearMethod(clearMethod);
  };

  const validateForm = (): boolean => {
    // 自定义邮箱需要验证邮箱地址
    if (
      isCodexProvider &&
      emailType !== "self_hosted" &&
      emailType !== "custom"
    ) {
      setToast({
        message: "Codex 当前仅支持自建邮箱 API 或手动验证码",
        type: "error",
      });
      return false;
    }

    if (emailType === "custom" && (!form.email || !form.email.includes("@"))) {
      setToast({ message: "请输入有效的邮箱地址", type: "error" });
      return false;
    }
    // Outlook邮箱需要验证邮箱地址
    if (emailType === "outlook" && outlookMode === "default") {
      if (!outlookEmail || !outlookEmail.includes("@")) {
        setToast({ message: "请输入有效的Outlook邮箱地址", type: "error" });
        return false;
      }
      if (!outlookEmail.toLowerCase().includes("outlook.com")) {
        setToast({ message: "请输入@outlook.com邮箱地址", type: "error" });
        return false;
      }
    }

    // Tempmail邮箱需要验证配置
    if (emailType === "tempmail") {
      if (!form.email || !form.email.includes("@")) {
        setToast({ message: "请输入有效的注册邮箱地址", type: "error" });
        return false;
      }
      if (!tempmailEmail || !tempmailEmail.includes("@")) {
        setToast({ message: "请输入有效的临时邮箱地址", type: "error" });
        return false;
      }
    }

    if (emailType === "self_hosted") {
      if (!form.email || !form.email.includes("@")) {
        setToast({ message: "请输入有效的注册邮箱地址", type: "error" });
        return false;
      }
      if (!selfHostedMailUrl.trim()) {
        setToast({ message: "请填写自建邮箱 API 的请求 URL", type: "error" });
        return false;
      }
      try {
        const h = JSON.parse(selfHostedMailHeadersJson) as unknown;
        if (typeof h !== "object" || h === null || Array.isArray(h)) {
          setToast({
            message: "Headers 须为 JSON 对象（键值对）",
            type: "error",
          });
          return false;
        }
      } catch {
        setToast({ message: "Headers 不是合法 JSON", type: "error" });
        return false;
      }
      if (!selfHostedMailResponsePath.trim()) {
        setToast({
          message: "请填写响应体路径，指向邮件原文字符串（如 results[0].raw）",
          type: "error",
        });
        return false;
      }
      if (selfHostedMailClearEnabled) {
        if (!selfHostedMailClearUrl.trim()) {
          setToast({ message: "请填写清空邮箱请求 URL", type: "error" });
          return false;
        }
        try {
          const clearHeaders = JSON.parse(
            selfHostedMailClearHeadersJson,
          ) as unknown;
          if (
            typeof clearHeaders !== "object" ||
            clearHeaders === null ||
            Array.isArray(clearHeaders)
          ) {
            setToast({
              message: "清空邮箱 Headers 须为 JSON 对象",
              type: "error",
            });
            return false;
          }
        } catch {
          setToast({
            message: "清空邮箱 Headers 不是合法 JSON",
            type: "error",
          });
          return false;
        }
        if (!selfHostedMailClearMethod.trim()) {
          setToast({ message: "请填写清空邮箱请求 Method", type: "error" });
          return false;
        }
      }
    }
    if (!form.firstName.trim()) {
      setToast({ message: "请输入名字", type: "error" });
      return false;
    }
    if (!form.lastName.trim()) {
      setToast({ message: "请输入姓氏", type: "error" });
      return false;
    }
    if (!form.password || form.password.length < 8) {
      setToast({ message: "密码长度至少8位", type: "error" });
      return false;
    }
    return true;
  };

  const buildRuntimeProxyConfig = async () => {
    const currentProxy = getProxyConfig() as any;
    if (!currentProxy.enabled || currentProxy.proxy_type !== "vless") {
      await invoke("stop_vless_proxy").catch(() => {});
      return currentProxy;
    }

    if (!currentProxy.vless_url?.trim()) {
      throw new Error("VLESS 模式下请先填写 vless 链接");
    }

    const normalizedHttpPort =
      Number.isInteger(Number(currentProxy.xray_http_port)) &&
      Number(currentProxy.xray_http_port) >= 1 &&
      Number(currentProxy.xray_http_port) <= 65535
        ? Number(currentProxy.xray_http_port)
        : 8991;
    const normalizedSocksPort =
      Number.isInteger(Number(currentProxy.xray_socks_port)) &&
      Number(currentProxy.xray_socks_port) >= 1 &&
      Number(currentProxy.xray_socks_port) <= 65535
        ? Number(currentProxy.xray_socks_port)
        : 1990;

    const runtime = await invoke<{
      httpProxy: string;
      socksProxy: string;
      httpPort: number;
      socksPort: number;
    }>("start_vless_proxy", {
      vlessUrl: currentProxy.vless_url.trim(),
      httpPort: normalizedHttpPort,
      socksPort: normalizedSocksPort,
    });

    return {
      ...currentProxy,
      proxy_type: "http",
      http_proxy: runtime.httpProxy,
      socks_proxy: runtime.socksProxy,
    };
  };

  const testAndStartVlessProxy = async () => {
    if (proxyType !== "vless") {
      setToast({ message: "请先选择 VLESS 代理类型", type: "error" });
      return;
    }
    if (!vlessUrl.trim()) {
      setToast({ message: "请填写 VLESS 标准链接", type: "error" });
      return;
    }

    setIsTestingVlessProxy(true);
    setVlessDownloadProgress({
      stage: "prepare",
      message: "准备启动 VLESS 代理...",
      percent: null,
      receivedBytes: 0,
      totalBytes: null,
    });
    try {
      const runtime = await invoke<{
        httpProxy: string;
        socksProxy: string;
        httpPort: number;
        socksPort: number;
      }>("start_vless_proxy", {
        vlessUrl: vlessUrl.trim(),
        httpPort:
          Number.isInteger(Number(xrayHttpPort)) &&
          xrayHttpPort >= 1 &&
          xrayHttpPort <= 65535
            ? xrayHttpPort
            : 8991,
        socksPort:
          Number.isInteger(Number(xraySocksPort)) &&
          xraySocksPort >= 1 &&
          xraySocksPort <= 65535
            ? xraySocksPort
            : 1990,
      });
      setVlessRuntimeProxy(runtime);
      setToast({
        message: `VLESS 启动成功：HTTP ${runtime.httpProxy} / SOCKS ${runtime.socksProxy}`,
        type: "success",
      });
      setVlessDownloadProgress((prev) =>
        prev
          ? {
              ...prev,
              stage: "completed",
              message: "VLESS 启动完成",
              percent: prev.percent ?? 100,
            }
          : null,
      );
    } catch (error) {
      setVlessRuntimeProxy(null);
      const errorText = String(error);
      if (errorText.includes("用户已取消 xray 下载")) {
        setToast({ message: "已取消 xray 下载", type: "info" });
      } else {
        setToast({ message: `VLESS 启动失败: ${error}`, type: "error" });
      }
    } finally {
      setIsTestingVlessProxy(false);
    }
  };

  const cancelVlessDownload = async () => {
    try {
      await invoke("cancel_vless_xray_download");
      setVlessDownloadProgress((prev) =>
        prev
          ? {
              ...prev,
              stage: "cancelled",
              message: "已取消下载，正在停止任务...",
            }
          : {
              stage: "cancelled",
              message: "已取消下载",
              percent: null,
              receivedBytes: 0,
              totalBytes: null,
            },
      );
      setToast({ message: "已取消 xray 下载", type: "info" });
    } catch (error) {
      setToast({ message: `取消下载失败: ${error}`, type: "error" });
    }
  };

  const copyText = async (text: string, label: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setToast({ message: `${label}已复制: ${text}`, type: "success" });
    } catch (error) {
      setToast({ message: `复制失败: ${error}`, type: "error" });
    }
  };

  const handleRegister = async () => {
    // 🔒 防止重复提交：检查是否已经在注册中
    if (isLoading || isRegistering) {
      console.warn("⚠️ 注册已在进行中，忽略重复点击");
      setToast({
        message: "注册正在进行中，请勿重复点击！",
        type: "info",
      });
      return;
    }

    // 单个注册模式
    isBatchRegisteringRef.current = false;

    if (!validateForm()) return;
    const runId = beginRegistrationRun();
    const codexCdpOverrides = isCodexProvider ? parseCodexCdpOverrides() : null;
    if (isCodexProvider && !codexCdpOverrides) {
      return;
    }

    // 如果使用 Tempmail，先清空邮箱
    if (emailType === "tempmail") {
      if (!tempmailEmail) {
        setToast({
          message: "请先配置 Tempmail 邮箱",
          type: "error",
        });
        return;
      }

      setToast({
        message: "🗑️ 正在清空 Tempmail 邮箱，避免干扰验证码获取...",
        type: "info",
      });

      const cleared = await clearTempmailInbox(
        tempmailEmail,
        tempmailPin || undefined,
      );
      if (runId !== registrationRunIdRef.current) {
        return;
      }
      if (cleared) {
        setToast({
          message: "✅ Tempmail 邮箱已清空，开始注册...",
          type: "success",
        });
      } else {
        setToast({
          message: "⚠️ Tempmail 邮箱清空失败，继续注册...",
          type: "info",
        });
      }
    }

    setIsLoading(true);
    setIsRegistering(true);
    batchProgressEntriesRef.current.clear();
    setRegistrationResult(null);
    cancelledStopTaskIdsRef.current.clear();
    realtimeOutputRef.current = []; // 清空ref
    setRealtimeOutput([]); // 清空之前的输出
    setToast({
      message: `开始注册${activeProviderLabel}账户...`,
      type: "info",
    });

    if (emailType === "self_hosted") {
      persistSelfHostedConfig();
    }

    try {
      // 如果启用了银行卡绑定，先检查并转换配置格式
      if (enableBankCardBinding) {
        setToast({ message: "检查银行卡配置格式...", type: "info" });
        try {
          const conversionResult = await invoke<string>(
            "check_and_convert_bank_card_config",
          );
          console.log("银行卡配置检查结果:", conversionResult);

          if (conversionResult.includes("转换")) {
            setToast({ message: "银行卡配置已转换为新格式", type: "success" });
          }
        } catch (error) {
          console.error("银行卡配置检查失败:", error);
          setToast({
            message: `银行卡配置检查失败: ${error}`,
            type: "error",
          });
          setIsLoading(false);
          setIsRegistering(false);
          return;
        }
      }
      let result: RegistrationResult;

      if (emailType === "cloudflare_temp") {
        // 使用Cloudflare临时邮箱注册
        result = await invoke<RegistrationResult>(
          "register_with_cloudflare_temp_email",
          {
            firstName: form.firstName,
            lastName: form.lastName,
            useIncognito: useIncognito,
            enableBankCardBinding: enableBankCardBinding,
            skipPhoneVerification: skipPhoneVerification,
            selectedCardIndex: enableBankCardBinding
              ? selectedCardIndex
              : undefined, // 传递选中的银行卡索引
            // 订阅配置参数整合到config中
            config: {
              subscriptionTier: subscriptionTier,
              allowAutomaticPayment: allowAutomaticPayment,
              allowTrial: allowTrial,
              useApiForBindCard: useApiForBindCard,
              btnIndex: isUsAccount ? 1 : 0, // 美国账户使用索引1，否则使用索引0
              cardIndex: enableBankCardBinding ? selectedCardIndex : 0, // 添加卡片索引
              // 代理配置
              proxy: await buildRuntimeProxyConfig(),
            },
          },
        );
      } else if (emailType === "tempmail") {
        // 使用Tempmail临时邮箱注册
        result = await CursorService.registerWithTempmail(
          form.email,
          form.firstName,
          form.lastName,
          tempmailEmail,
          tempmailPin,
          useIncognito,
          enableBankCardBinding,
          skipPhoneVerification,
          enableBankCardBinding ? selectedCardIndex : undefined,
          {
            subscriptionTier: subscriptionTier,
            allowAutomaticPayment: allowAutomaticPayment,
            allowTrial: allowTrial,
            useApiForBindCard: useApiForBindCard,
            btnIndex: isUsAccount ? 1 : 0,
            cardIndex: enableBankCardBinding ? selectedCardIndex : 0,
            proxy: await buildRuntimeProxyConfig(),
          },
        );
      } else if (
        (emailType === "self_hosted" && !isCodexProvider) ||
        (isCodexProvider &&
          (emailType === "self_hosted" || emailType === "custom"))
      ) {
        const sharedConfig = {
          subscriptionTier: subscriptionTier,
          allowAutomaticPayment: allowAutomaticPayment,
          allowTrial: allowTrial,
          useApiForBindCard: useApiForBindCard,
          btnIndex: isUsAccount ? 1 : 0,
          cardIndex: enableBankCardBinding ? selectedCardIndex : 0,
          proxy: await buildRuntimeProxyConfig(),
        };

        result = isCodexProvider
          ? await CursorService.registerCodexWithSelfHostedMailApi(
              form.email,
              form.firstName,
              form.lastName,
              emailType === "self_hosted" ? selfHostedMailUrl.trim() : "",
              emailType === "self_hosted" ? selfHostedMailHeadersJson : "{}",
              emailType === "self_hosted"
                ? selfHostedMailResponsePath.trim()
                : "results[0].raw",
              emailType === "self_hosted" ? selfHostedMailClearEnabled : false,
              emailType === "self_hosted" ? selfHostedMailClearUrl.trim() : "",
              emailType === "self_hosted"
                ? selfHostedMailClearHeadersJson
                : "{}",
              emailType === "self_hosted"
                ? selfHostedMailClearMethod.trim().toUpperCase() || "DELETE"
                : "DELETE",
              useIncognito,
              emailType === "custom",
              {
                proxy: sharedConfig.proxy,
                codexCdpOverrides,
              },
            )
          : await CursorService.registerWithSelfHostedMailApi(
              form.email,
              form.firstName,
              form.lastName,
              selfHostedMailUrl.trim(),
              selfHostedMailHeadersJson,
              selfHostedMailResponsePath.trim(),
              selfHostedMailClearEnabled,
              selfHostedMailClearUrl.trim(),
              selfHostedMailClearHeadersJson,
              selfHostedMailClearMethod.trim().toUpperCase() || "DELETE",
              useIncognito,
              enableBankCardBinding,
              skipPhoneVerification,
              enableBankCardBinding ? selectedCardIndex : undefined,
              sharedConfig,
            );
      } else if (emailType === "outlook" && outlookMode === "default") {
        // 使用Outlook邮箱注册
        result = await invoke<RegistrationResult>("register_with_outlook", {
          email: outlookEmail,
          firstName: form.firstName,
          lastName: form.lastName,
          useIncognito: useIncognito,
          enableBankCardBinding: enableBankCardBinding,
          skipPhoneVerification: skipPhoneVerification,
          selectedCardIndex: enableBankCardBinding
            ? selectedCardIndex
            : undefined, // 传递选中的银行卡索引
          // 订阅配置参数整合到config中
          config: {
            subscriptionTier: subscriptionTier,
            allowAutomaticPayment: allowAutomaticPayment,
            allowTrial: allowTrial,
            useApiForBindCard: useApiForBindCard,
            btnIndex: isUsAccount ? 1 : 0, // 美国账户使用索引1，否则使用索引0
            cardIndex: enableBankCardBinding ? selectedCardIndex : 0, // 添加卡片索引
            // 代理配置
            proxy: await buildRuntimeProxyConfig(),
          },
        });
      } else {
        // 使用自定义邮箱注册
        result = await invoke<RegistrationResult>("register_with_email", {
          email: form.email,
          firstName: form.firstName,
          lastName: form.lastName,
          useIncognito: useIncognito,
          enableBankCardBinding: enableBankCardBinding,
          skipPhoneVerification: skipPhoneVerification,
          selectedCardIndex: enableBankCardBinding
            ? selectedCardIndex
            : undefined, // 传递选中的银行卡索引
          // 订阅配置参数整合到config中
          config: {
            subscriptionTier: subscriptionTier,
            allowAutomaticPayment: allowAutomaticPayment,
            allowTrial: allowTrial,
            useApiForBindCard: useApiForBindCard,
            btnIndex: isUsAccount ? 1 : 0, // 美国账户使用索引1，否则使用索引0
            cardIndex: enableBankCardBinding ? selectedCardIndex : 0, // 添加卡片索引
            // 代理配置
            proxy: await buildRuntimeProxyConfig(),
          },
        });
      }

      if (runId !== registrationRunIdRef.current) {
        return;
      }
      setRegistrationResult(result);

      // 调试：打印收到的结果
      console.log("注册结果:", result);
      console.log("输出行数:", result.output_lines?.length || 0);

      // 检查输出中是否包含验证码请求
      const needsVerificationCode = result.message.includes("请输入验证码");

      if (needsVerificationCode && emailType === "custom") {
        // 只有自定义邮箱才需要手动输入验证码
        setShowVerificationModal(true);
        setToast({ message: "请输入验证码", type: "info" });
      } else if (needsVerificationCode && emailType === "outlook") {
        // Outlook邮箱会自动获取验证码
        setToast({ message: "正在从Outlook邮箱获取验证码...", type: "info" });
      } else if (needsVerificationCode && emailType === "tempmail") {
        // Tempmail邮箱会自动获取验证码
        setToast({ message: "正在从Tempmail邮箱获取验证码...", type: "info" });
      } else if (needsVerificationCode && emailType === "self_hosted") {
        setToast({
          message: "正在通过自建邮箱 API 获取验证码...",
          type: "info",
        });
      } else if (isRegistrationSuccessResult(result)) {
        // 注册成功，确保关闭验证码弹窗
        setShowVerificationModal(false);
        setToast({ message: "注册成功！", type: "success" });
      } else {
        // 注册失败，也关闭验证码弹窗
        setShowVerificationModal(false);
        setToast({ message: result.message || "注册失败", type: "error" });
      }
    } catch (error) {
      if (runId !== registrationRunIdRef.current) {
        return;
      }
      console.error("Registration error:", error);
      setToast({
        message: `注册失败: ${error}`,
        type: "error",
      });
    } finally {
      if (runId === registrationRunIdRef.current) {
        resetRegistrationUiState();
      }
    }
  };

  const handleGenerateRandom = () => {
    generateRandomInfo();
    setToast({ message: "已生成随机账户信息", type: "info" });
  };

  // 当批量数量变化时，更新邮箱数组
  useEffect(() => {
    if (
      emailType === "custom" ||
      emailType === "tempmail" ||
      emailType === "self_hosted"
    ) {
      const newEmails = Array(batchCount)
        .fill("")
        .map((_, i) => batchEmails[i] || "");
      setBatchEmails(newEmails);
    }
  }, [batchCount, emailType]);

  // 批量注册处理函数
  const handleBatchRegister = async () => {
    // 批量注册模式
    isBatchRegisteringRef.current = true;
    const runId = beginRegistrationRun();
    const codexCdpOverrides = isCodexProvider ? parseCodexCdpOverrides() : null;
    if (isCodexProvider && !codexCdpOverrides) {
      return;
    }

    // if (batchCount < 1 || batchCount > 3) {
    //   setToast({ message: "注册数量必须在1-3之间", type: "error" });
    //   return;
    // }

    // 验证邮箱是否都已填写
    if (
      emailType === "custom" ||
      emailType === "tempmail" ||
      emailType === "self_hosted"
    ) {
      const emptyEmails = batchEmails.filter(
        (email) => !email || !email.includes("@"),
      );
      if (emptyEmails.length > 0) {
        setToast({
          message: "请填写所有注册邮箱地址",
          type: "error",
        });
        return;
      }
    }

    if (emailType === "self_hosted") {
      if (!selfHostedMailUrl.trim()) {
        setToast({ message: "请填写自建邮箱 API 的请求 URL", type: "error" });
        return;
      }
      try {
        const h = JSON.parse(selfHostedMailHeadersJson) as unknown;
        if (typeof h !== "object" || h === null || Array.isArray(h)) {
          setToast({ message: "Headers 须为 JSON 对象", type: "error" });
          return;
        }
      } catch {
        setToast({ message: "Headers 不是合法 JSON", type: "error" });
        return;
      }
      if (!selfHostedMailResponsePath.trim()) {
        setToast({
          message: "请填写响应体路径（如 results[0].raw）",
          type: "error",
        });
        return;
      }
      if (selfHostedMailClearEnabled) {
        if (!selfHostedMailClearUrl.trim()) {
          setToast({ message: "请填写清空邮箱请求 URL", type: "error" });
          return;
        }
        try {
          const clearHeaders = JSON.parse(
            selfHostedMailClearHeadersJson,
          ) as unknown;
          if (
            typeof clearHeaders !== "object" ||
            clearHeaders === null ||
            Array.isArray(clearHeaders)
          ) {
            setToast({
              message: "清空邮箱 Headers 须为 JSON 对象",
              type: "error",
            });
            return;
          }
        } catch {
          setToast({
            message: "清空邮箱 Headers 不是合法 JSON",
            type: "error",
          });
          return;
        }
        if (!selfHostedMailClearMethod.trim()) {
          setToast({ message: "请填写清空邮箱请求 Method", type: "error" });
          return;
        }
      }
      persistSelfHostedConfig();
    }

    // 验证Tempmail配置
    if (emailType === "tempmail") {
      if (!tempmailEmail || !tempmailEmail.includes("@")) {
        setToast({
          message: "请输入有效的Tempmail邮箱地址（用于接收验证码）",
          type: "error",
        });
        return;
      }
    }

    // 验证银行卡配置
    if (!isCodexProvider && enableBankCardBinding) {
      // 检查选中的银行卡数量是否足够
      if (selectedBatchCardIndices.length < batchCount) {
        setToast({
          message: `选中的银行卡数量(${selectedBatchCardIndices.length})少于注册数量(${batchCount})，请选择足够的银行卡`,
          type: "error",
        });
        return;
      }
    }

    // 如果使用 Tempmail，先清空邮箱
    if (emailType === "tempmail") {
      if (!tempmailEmail) {
        setToast({
          message: "请先配置 Tempmail 邮箱",
          type: "error",
        });
        return;
      }

      setToast({
        message: "🗑️ 正在清空 Tempmail 邮箱，避免干扰验证码获取...",
        type: "info",
      });

      const cleared = await clearTempmailInbox(
        tempmailEmail,
        tempmailPin || undefined,
      );
      if (runId !== registrationRunIdRef.current) {
        return;
      }
      console.log(batchCount, "batchCount");

      if (cleared) {
        setToast({
          message: "✅ Tempmail 邮箱已清空，开始批量注册...",
          type: "success",
        });
      } else {
        setToast({
          message: "⚠️ Tempmail 邮箱清空失败，继续批量注册...",
          type: "info",
        });
      }
    }

    // 准备批量注册数据
    const emails: string[] = [];
    const firstNames: string[] = [];
    const lastNames: string[] = [];

    for (let i = 0; i < batchCount; i++) {
      if (emailType === "custom") {
        // 自定义邮箱：使用用户输入的邮箱列表
        emails.push(batchEmails[i] || "");
      } else if (emailType === "outlook") {
        // Outlook邮箱：使用配置的Outlook邮箱
        emails.push(outlookEmail || "");
      } else if (emailType === "tempmail") {
        // Tempmail邮箱：使用用户输入的注册邮箱列表
        emails.push(batchEmails[i] || "");
      } else if (emailType === "self_hosted") {
        emails.push(batchEmails[i] || "");
      } else {
        // Cloudflare临时邮箱：传空字符串，后端会自动生成
        emails.push("");
      }

      // 使用输入的姓名或随机生成
      if (useRandomInfo || !form.firstName || !form.lastName) {
        const randomInfo = generateBatchRandomInfo();
        firstNames.push(randomInfo.firstName);
        lastNames.push(randomInfo.lastName);
      } else {
        firstNames.push(form.firstName);
        lastNames.push(form.lastName);
      }
    }

    setIsLoading(true);
    setIsRegistering(true);
    setRegistrationResult(null);
    cancelledStopTaskIdsRef.current.clear();
    realtimeOutputRef.current = [];
    setRealtimeOutput([]);
    setRegistrationResult({
      success: true,
      message: `批量注册进行中：0/${batchCount} 已完成`,
      details: [],
    });
    setToast({
      message: `开始批量注册 ${batchCount} 个账户（${
        useParallelMode ? "并行" : "串行"
      }模式）...`,
      type: "info",
    });

    try {
      // 根据模式选择不同的命令
      const commandName = isCodexProvider
        ? useParallelMode
          ? "batch_register_codex_with_email_parallel"
          : "batch_register_codex_with_email"
        : useParallelMode
          ? "batch_register_with_email_parallel"
          : "batch_register_with_email";

      const result = await invoke<any>(commandName, {
        emails,
        firstNames,
        lastNames,
        ...(!useParallelMode
          ? {
              batchDelaySeconds: Math.max(
                0,
                Math.floor(batchSerialDelaySeconds),
              ),
            }
          : {}),
        emailType,
        outlookMode: emailType === "outlook" ? outlookMode : undefined,
        tempmailEmail: emailType === "tempmail" ? tempmailEmail : undefined,
        tempmailPin: emailType === "tempmail" ? tempmailPin : undefined,
        selfHostedMailUrl:
          emailType === "self_hosted" ? selfHostedMailUrl.trim() : undefined,
        selfHostedMailHeadersJson:
          emailType === "self_hosted" ? selfHostedMailHeadersJson : undefined,
        selfHostedMailResponsePath:
          emailType === "self_hosted"
            ? selfHostedMailResponsePath.trim()
            : undefined,
        selfHostedMailClearEnabled:
          emailType === "self_hosted" ? selfHostedMailClearEnabled : undefined,
        selfHostedMailClearUrl:
          emailType === "self_hosted"
            ? selfHostedMailClearUrl.trim()
            : undefined,
        selfHostedMailClearHeadersJson:
          emailType === "self_hosted"
            ? selfHostedMailClearHeadersJson
            : undefined,
        selfHostedMailClearMethod:
          emailType === "self_hosted"
            ? selfHostedMailClearMethod.trim().toUpperCase() || "DELETE"
            : undefined,
        useIncognito,
        enableBankCardBinding: isCodexProvider
          ? undefined
          : enableBankCardBinding,
        skipPhoneVerification: isCodexProvider
          ? undefined
          : skipPhoneVerification,
        selectedCardIndices:
          !isCodexProvider && enableBankCardBinding
            ? selectedBatchCardIndices.slice(0, batchCount)
            : undefined,
        config: isCodexProvider
          ? {
              proxy: await buildRuntimeProxyConfig(),
              codexCdpOverrides,
            }
          : {
              subscriptionTier: subscriptionTier,
              allowAutomaticPayment: allowAutomaticPayment,
              allowTrial: allowTrial,
              useApiForBindCard: useApiForBindCard,
              btnIndex: isUsAccount ? 1 : 0,
              proxy: await buildRuntimeProxyConfig(),
            },
        maxConcurrent: batchCount,
      });

      if (runId !== registrationRunIdRef.current) {
        return;
      }
      console.log("批量注册结果:", result);

      if (result.success) {
        setToast({
          message: `批量注册完成！成功: ${result.succeeded}, 失败: ${result.failed}`,
          type: result.failed > 0 ? "info" : "success",
        });

        // 显示详细结果
        setRegistrationResult({
          success: true,
          message: `批量注册完成：${result.succeeded}/${result.total} 成功`,
          details: [
            ...result.results.map(
              (r: any) => `✅ [${r.index + 1}] ${r.email}: 成功`,
            ),
            ...result.errors.map(
              (e: any) => `❌ [${e.index + 1}] ${e.email}: ${e.error}`,
            ),
          ],
        });
      } else {
        setToast({ message: result.message || "批量注册失败", type: "error" });
      }
    } catch (error) {
      if (runId !== registrationRunIdRef.current) {
        return;
      }
      console.error("批量注册错误:", error);
      setToast({ message: `批量注册失败: ${error}`, type: "error" });
    } finally {
      if (runId === registrationRunIdRef.current) {
        resetRegistrationUiState();
      }
    }
  };

  // 生成随机姓名
  const generateBatchRandomInfo = () => {
    const firstNames = [
      "Alex",
      "Jordan",
      "Taylor",
      "Casey",
      "Morgan",
      "Riley",
      "Avery",
      "Quinn",
      "Skyler",
      "Cameron",
    ];
    const lastNames = [
      "Smith",
      "Johnson",
      "Williams",
      "Brown",
      "Jones",
      "Garcia",
      "Miller",
      "Davis",
      "Rodriguez",
      "Martinez",
    ];

    return {
      firstName: firstNames[Math.floor(Math.random() * firstNames.length)],
      lastName: lastNames[Math.floor(Math.random() * lastNames.length)],
    };
  };

  // 加载银行卡配置
  const loadBankCardConfig = async () => {
    try {
      const config = await BankCardConfigService.getBankCardConfig();
      setBankCardConfig(config);
    } catch (error) {
      console.error("加载银行卡配置失败:", error);
    }
  };

  const handleBankCardConfigSave = async (config: BankCardConfig) => {
    setBankCardConfig(config);

    // 重新加载银行卡列表以更新选择列表
    try {
      const configList = await BankCardConfigService.getBankCardConfigList();
      setBankCardList(configList.cards);

      // 如果当前选中的索引超出了新列表的范围，重置为第一张卡
      if (selectedCardIndex >= configList.cards.length) {
        setSelectedCardIndex(0);
      }

      // 同样处理批量注册的选中索引
      const validBatchIndices = selectedBatchCardIndices.filter(
        (index) => index < configList.cards.length,
      );
      if (validBatchIndices.length === 0 && configList.cards.length > 0) {
        setSelectedBatchCardIndices([0]);
      } else if (validBatchIndices.length !== selectedBatchCardIndices.length) {
        setSelectedBatchCardIndices(validBatchIndices);
      }

      setToast({
        message: "银行卡配置已更新，选择列表已刷新",
        type: "success",
      });
    } catch (error) {
      console.error("重新加载银行卡列表失败:", error);
      setToast({
        message: "银行卡配置已更新，但选择列表刷新失败",
        type: "error",
      });
    }
  };

  // 加载邮箱配置
  const loadEmailConfig = async () => {
    try {
      const config = await EmailConfigService.getEmailConfig();
      setEmailConfig(config);
    } catch (error) {
      console.error("加载邮箱配置失败:", error);
    }
  };

  // 浏览器路径管理函数
  const loadBrowserPath = async () => {
    try {
      const path = await CursorService.getCustomBrowserPath();
      setCurrentBrowserPath(path);
      setCustomBrowserPath(path || "");
    } catch (error) {
      console.error("加载浏览器路径失败:", error);
    }
  };

  const handleSelectBrowserFile = async () => {
    try {
      const selectedPath = await CursorService.selectBrowserFile();
      if (selectedPath) {
        setCustomBrowserPath(selectedPath);
      }
    } catch (error) {
      console.error("选择浏览器文件失败:", error);
      setToast({
        message: "选择浏览器文件失败",
        type: "error",
      });
    }
  };

  const handleSetBrowserPath = async () => {
    if (!customBrowserPath.trim()) {
      setToast({
        message: "请输入浏览器路径",
        type: "error",
      });
      return;
    }

    try {
      const result =
        await CursorService.setCustomBrowserPath(customBrowserPath);
      setCurrentBrowserPath(customBrowserPath);
      setToast({
        message: "浏览器路径设置成功",
        type: "success",
      });
      console.log("浏览器路径验证结果:", result);
    } catch (error) {
      console.error("设置浏览器路径失败:", error);
      setToast({
        message: "设置浏览器路径失败",
        type: "error",
      });
    }
  };

  const handleClearBrowserPath = async () => {
    try {
      await CursorService.clearCustomBrowserPath();
      setCurrentBrowserPath(null);
      setCustomBrowserPath("");
      setToast({
        message: "浏览器路径已清除",
        type: "success",
      });
    } catch (error) {
      console.error("清除浏览器路径失败:", error);
      setToast({
        message: "清除浏览器路径失败",
        type: "error",
      });
    }
  };

  const handleEmailConfigSave = (config: EmailConfig) => {
    setEmailConfig(config);
    setToast({ message: "邮箱配置已更新", type: "success" });
  };

  // Initialize with random info on component mount
  useEffect(() => {
    if (useRandomInfo) {
      generateRandomInfo();
    }
    // 加载银行卡配置和邮箱配置
    loadBankCardConfig();
    loadEmailConfig();
    loadBrowserPath();
  }, [useRandomInfo]);

  // 从缓存中初始化配置
  useEffect(() => {
    const cachedProvider = getRegistrationProvider();
    if (cachedProvider) {
      setRegistrationProvider(cachedProvider);
    }

    const cachedEmail = getTempmailEmail();
    const cachedPin = getPinCode();

    if (cachedEmail) {
      setTempmailEmail(cachedEmail);
    }

    if (cachedPin) {
      setTempmailPin(cachedPin);
    }

    const cachedEmailType = getEmailType();
    if (cachedEmailType) {
      setEmailType(cachedEmailType);
    }

    const cachedUseIncognito = getUseIncognito();
    if (cachedUseIncognito !== null) {
      setUseIncognito(cachedUseIncognito);
    }

    const cachedEnableBankCardBinding = getEnableBankCardBinding();
    if (cachedEnableBankCardBinding !== null) {
      setEnableBankCardBinding(cachedEnableBankCardBinding);
    }

    const cachedUseParallelMode = getUseParallelMode();
    if (cachedUseParallelMode !== null) {
      setUseParallelMode(cachedUseParallelMode);
    }

    const cachedBatchSerialDelaySeconds = getBatchSerialDelaySeconds();
    if (cachedBatchSerialDelaySeconds !== null) {
      setBatchSerialDelaySeconds(
        Math.max(0, Math.floor(cachedBatchSerialDelaySeconds)),
      );
    }

    const cursorUrl = getSelfHostedMailUrl();
    const cursorHdr = getSelfHostedMailHeadersJson();
    const cursorPath = getSelfHostedMailResponsePath();
    const cursorClearEnabled = getSelfHostedMailClearEnabled();
    const cursorClearUrl = getSelfHostedMailClearUrl();
    const cursorClearHdr = getSelfHostedMailClearHeadersJson();
    const cursorClearMethod = getSelfHostedMailClearMethod();
    const codexUrl = getCodexSelfHostedMailUrl();
    const codexHdr = getCodexSelfHostedMailHeadersJson();
    const codexPath = getCodexSelfHostedMailResponsePath();
    const codexClearEnabled = getCodexSelfHostedMailClearEnabled();
    const codexClearUrl = getCodexSelfHostedMailClearUrl();
    const codexClearHdr = getCodexSelfHostedMailClearHeadersJson();
    const codexClearMethod = getCodexSelfHostedMailClearMethod();
    const codexCdpJson = getCodexCdpOverridesJson();

    const activeProvider = cachedProvider ?? "cursor";
    const activeUrl =
      activeProvider === "codex" ? (codexUrl ?? cursorUrl) : cursorUrl;
    const activeHdr =
      activeProvider === "codex" ? (codexHdr ?? cursorHdr) : cursorHdr;
    const activePath =
      activeProvider === "codex" ? (codexPath ?? cursorPath) : cursorPath;
    const activeClearEnabled =
      activeProvider === "codex"
        ? (codexClearEnabled ?? cursorClearEnabled)
        : cursorClearEnabled;
    const activeClearUrl =
      activeProvider === "codex"
        ? (codexClearUrl ?? cursorClearUrl)
        : cursorClearUrl;
    const activeClearHdr =
      activeProvider === "codex"
        ? (codexClearHdr ?? cursorClearHdr)
        : cursorClearHdr;
    const activeClearMethod =
      activeProvider === "codex"
        ? (codexClearMethod ?? cursorClearMethod)
        : cursorClearMethod;

    if (activeUrl) setSelfHostedMailUrl(activeUrl);
    if (activeHdr) setSelfHostedMailHeadersJson(activeHdr);
    if (activePath) setSelfHostedMailResponsePath(activePath);
    if (activeClearEnabled !== null)
      setSelfHostedMailClearEnabled(activeClearEnabled);
    if (activeClearUrl) setSelfHostedMailClearUrl(activeClearUrl);
    if (activeClearHdr) setSelfHostedMailClearHeadersJson(activeClearHdr);
    if (activeClearMethod) setSelfHostedMailClearMethod(activeClearMethod);
    if (codexCdpJson) setCodexCdpOverridesJson(codexCdpJson);
  }, [
    getRegistrationProvider,
    getTempmailEmail,
    getPinCode,
    getEmailType,
    getUseIncognito,
    getEnableBankCardBinding,
    getUseParallelMode,
    getSelfHostedMailUrl,
    getSelfHostedMailHeadersJson,
    getSelfHostedMailResponsePath,
    getSelfHostedMailClearEnabled,
    getSelfHostedMailClearUrl,
    getSelfHostedMailClearHeadersJson,
    getSelfHostedMailClearMethod,
    getCodexSelfHostedMailUrl,
    getCodexSelfHostedMailHeadersJson,
    getCodexSelfHostedMailResponsePath,
    getCodexSelfHostedMailClearEnabled,
    getCodexSelfHostedMailClearUrl,
    getCodexSelfHostedMailClearHeadersJson,
    getCodexSelfHostedMailClearMethod,
    getCodexCdpOverridesJson,
  ]);

  return (
    <div className="max-w-4xl mx-auto">
      <div className="shadow surface-primary rounded-2xl">
        <div className="px-4 py-5 sm:p-6">
          <div className="flex items-center justify-between mb-6">
            <div>
              <h3 className="text-lg font-medium leading-6 text-slate-900 dark:text-slate-100">
                📝 {activeProviderLabel} 自动注册
              </h3>
              <div className="flex gap-4 mt-3">
                <label className="flex items-center text-sm cursor-pointer text-slate-700 dark:text-slate-300">
                  <input
                    type="radio"
                    name="registration-provider"
                    checked={registrationProvider === "cursor"}
                    onChange={() => switchRegistrationProvider("cursor")}
                    className="w-4 h-4 text-blue-600 rounded border-slate-300 focus:ring-blue-500 dark:border-slate-600 dark:bg-slate-900"
                  />
                  <span className="ml-2">Cursor</span>
                </label>
                <label className="flex items-center text-sm cursor-pointer text-slate-700 dark:text-slate-300">
                  <input
                    type="radio"
                    name="registration-provider"
                    checked={registrationProvider === "codex"}
                    onChange={() => switchRegistrationProvider("codex")}
                    className="w-4 h-4 text-blue-600 rounded border-slate-300 focus:ring-blue-500 dark:border-slate-600 dark:bg-slate-900"
                  />
                  <span className="ml-2">Codex</span>
                </label>
              </div>
            </div>
            <div className="flex items-center space-x-2">
              <Button
                onClick={() => setShowBrowserConfig(true)}
                variant="secondary"
                className="flex items-center"
              >
                🌐 浏览器配置
              </Button>
              {!isCodexProvider && (
                <Button
                  onClick={() => setShowBankCardConfig(true)}
                  variant="secondary"
                  className="flex items-center"
                >
                  💳 银行卡配置
                </Button>
              )}
            </div>
          </div>

          <div className="space-y-6">
            {/* 使用随机信息选项 */}
            <div className="flex items-center">
              <input
                id="use-random"
                type="checkbox"
                checked={useRandomInfo}
                onChange={(e) => setUseRandomInfo(e.target.checked)}
                className="w-4 h-4 text-blue-600 rounded border-slate-300 focus:ring-blue-500 dark:border-slate-600 dark:bg-slate-900"
              />
              <label
                htmlFor="use-random"
                className="block ml-2 text-sm text-slate-900 dark:text-slate-100"
              >
                使用随机生成的账户信息
              </label>
            </div>

            {/* 表单 */}
            <div className="grid grid-cols-1 gap-6 sm:grid-cols-2">
              <div>
                <label
                  htmlFor="firstName"
                  className="block text-sm font-medium text-slate-700 dark:text-slate-300"
                >
                  名字
                </label>
                <input
                  type="text"
                  id="firstName"
                  value={form.firstName}
                  onChange={(e) =>
                    handleInputChange("firstName", e.target.value)
                  }
                  disabled={useRandomInfo}
                  className="block w-full mt-1 rounded-md shadow-sm border-slate-300 dark:border-slate-700 focus:ring-blue-500 focus:border-blue-500 sm:text-sm disabled:bg-slate-100 dark:disabled:bg-slate-800"
                  placeholder="请输入名字"
                />
              </div>

              <div>
                <label
                  htmlFor="lastName"
                  className="block text-sm font-medium text-slate-700 dark:text-slate-300"
                >
                  姓氏
                </label>
                <input
                  type="text"
                  id="lastName"
                  value={form.lastName}
                  onChange={(e) =>
                    handleInputChange("lastName", e.target.value)
                  }
                  disabled={useRandomInfo}
                  className="block w-full mt-1 rounded-md shadow-sm border-slate-300 dark:border-slate-700 focus:ring-blue-500 focus:border-blue-500 sm:text-sm disabled:bg-slate-100 dark:disabled:bg-slate-800"
                  placeholder="请输入姓氏"
                />
              </div>

              <div className="sm:col-span-2">
                <label className="block mb-3 text-sm font-medium text-slate-700 dark:text-slate-300">
                  邮箱类型
                </label>
                <div className="space-y-2">
                  {isCodexProvider && (
                    <div className="p-3 border rounded-md bg-slate-50 border-slate-200">
                      <p className="text-sm text-slate-700">
                        Codex 支持两种验证码模式：自建邮箱 API
                        自动获取，或自定义邮箱手动输入验证码。
                      </p>
                    </div>
                  )}
                  {!isCodexProvider && (
                    <>
                      <div className="flex items-center">
                        <input
                          id="email-custom"
                          name="email-type"
                          type="radio"
                          value="custom"
                          checked={emailType === "custom"}
                          onChange={(e) => {
                            const newType = e.target.value as EmailType;
                            setEmailType(newType);
                            setCachedEmailType(newType);
                          }}
                          className="w-4 h-4 text-blue-600 rounded border-slate-300 focus:ring-blue-500 dark:border-slate-600 dark:bg-slate-900"
                        />
                        <label
                          htmlFor="email-custom"
                          className="ml-2 text-sm text-slate-700 dark:text-slate-300"
                        >
                          自定义邮箱（手动输入验证码）
                        </label>
                      </div>
                      <div className="flex items-center">
                        <input
                          id="email-cloudflare"
                          name="email-type"
                          type="radio"
                          value="cloudflare_temp"
                          checked={emailType === "cloudflare_temp"}
                          onChange={(e) => {
                            const newType = e.target.value as EmailType;
                            setEmailType(newType);
                            setCachedEmailType(newType);
                          }}
                          className="w-4 h-4 text-blue-600 rounded border-slate-300 focus:ring-blue-500 dark:border-slate-600 dark:bg-slate-900"
                        />
                        <label
                          htmlFor="email-cloudflare"
                          className="ml-2 text-sm text-slate-700 dark:text-slate-300"
                        >
                          Cloudflare临时邮箱（自动获取验证码）
                        </label>
                      </div>
                      <div className="flex items-center">
                        <input
                          id="email-tempmail"
                          name="email-type"
                          type="radio"
                          value="tempmail"
                          checked={emailType === "tempmail"}
                          onChange={(e) => {
                            const newType = e.target.value as EmailType;
                            setEmailType(newType);
                            setCachedEmailType(newType);
                          }}
                          className="w-4 h-4 text-blue-600 rounded border-slate-300 focus:ring-blue-500 dark:border-slate-600 dark:bg-slate-900"
                        />
                        <label
                          htmlFor="email-tempmail"
                          className="ml-2 text-sm text-slate-700 dark:text-slate-300"
                        >
                          Tempmail临时邮箱（自动获取验证码）
                        </label>
                      </div>
                    </>
                  )}
                  {isCodexProvider && (
                    <div className="flex items-center">
                      <input
                        id="email-custom-codex"
                        name="email-type"
                        type="radio"
                        value="custom"
                        checked={emailType === "custom"}
                        onChange={(e) => {
                          const newType = e.target.value as EmailType;
                          setEmailType(newType);
                        }}
                        className="w-4 h-4 text-blue-600 rounded border-slate-300 focus:ring-blue-500 dark:border-slate-600 dark:bg-slate-900"
                      />
                      <label
                        htmlFor="email-custom-codex"
                        className="ml-2 text-sm text-slate-700 dark:text-slate-300"
                      >
                        自定义邮箱（手动输入验证码）
                      </label>
                    </div>
                  )}
                  <div className="flex items-center">
                    <input
                      id="email-self-hosted"
                      name="email-type"
                      type="radio"
                      value="self_hosted"
                      checked={emailType === "self_hosted"}
                      onChange={(e) => {
                        const newType = e.target.value as EmailType;
                        setEmailType(newType);
                        if (!isCodexProvider) {
                          setCachedEmailType(newType);
                        }
                      }}
                      className="w-4 h-4 text-blue-600 rounded border-slate-300 focus:ring-blue-500 dark:border-slate-600 dark:bg-slate-900"
                    />
                    <label
                      htmlFor="email-self-hosted"
                      className="ml-2 text-sm text-slate-700 dark:text-slate-300"
                    >
                      自建邮箱 API（自动获取验证码）
                    </label>
                  </div>
                  {/* <div className="flex items-center">
                    <input
                      id="email-outlook"
                      name="email-type"
                      type="radio"
                      value="outlook"
                      checked={emailType === "outlook"}
                      onChange={(e) =>
                        setEmailType(
                          e.target.value as EmailType
                        )
                      }
                      className="w-4 h-4 text-blue-600 rounded border-slate-300 focus:ring-blue-500 dark:border-slate-600 dark:bg-slate-900"
                    />
                    <label
                      htmlFor="email-outlook"
                      className="ml-2 text-sm text-slate-700 dark:text-slate-300"
                    >
                      Outlook邮箱（自动获取验证码）
                    </label>
                  </div> */}
                </div>
              </div>

              {(emailType === "custom" ||
                emailType === "tempmail" ||
                emailType === "self_hosted") && (
                <div className="sm:col-span-2">
                  <label
                    htmlFor="email"
                    className="block text-sm font-medium text-slate-700 dark:text-slate-300"
                  >
                    {emailType === "tempmail" || emailType === "self_hosted"
                      ? "注册邮箱地址"
                      : "邮箱地址"}
                  </label>
                  <input
                    type="email"
                    id="email"
                    value={form.email}
                    onChange={(e) => handleInputChange("email", e.target.value)}
                    className="mt-1 field-input sm:text-sm"
                    placeholder={
                      emailType === "tempmail" || emailType === "self_hosted"
                        ? "请输入用于注册的真实邮箱地址"
                        : "请输入邮箱地址"
                    }
                  />
                </div>
              )}

              {emailType === "cloudflare_temp" && (
                <div className="sm:col-span-2">
                  <div className="p-3 rounded-md status-info">
                    <p className="text-sm text-blue-700">
                      📧 将自动创建临时邮箱并获取验证码，无需手动输入
                    </p>
                  </div>
                </div>
              )}

              {emailType === "tempmail" && (
                <div className="space-y-4 sm:col-span-2">
                  <div className="p-3 rounded-md status-success">
                    <p className="text-sm text-green-700">
                      📧 配置Tempmail临时邮箱，将自动获取转发到临时邮箱的验证码
                    </p>
                    <p className="mt-2 text-xs text-green-600">
                      🗑️ 每次注册前会自动清空临时邮箱，避免旧邮件干扰验证码获取
                    </p>
                  </div>

                  <div>
                    <label
                      htmlFor="tempmail-email"
                      className="block text-sm font-medium text-slate-700 dark:text-slate-300"
                    >
                      临时邮箱地址 <span className="text-red-500">*</span>
                    </label>
                    <input
                      type="email"
                      id="tempmail-email"
                      value={tempmailEmail}
                      onChange={(e) => {
                        const value = e.target.value;
                        setTempmailEmail(value);
                        setCachedTempmailEmail(value);
                      }}
                      className="mt-1 field-input sm:text-sm"
                      placeholder="请输入tempmail邮箱地址，如：xxx@mailto.plus"
                    />
                  </div>

                  <div>
                    <label
                      htmlFor="tempmail-pin"
                      className="block text-sm font-medium text-slate-700 dark:text-slate-300"
                    >
                      PIN码{" "}
                      <span className="text-slate-400 dark:text-slate-500">
                        (可选)
                      </span>
                    </label>
                    <input
                      type="text"
                      id="tempmail-pin"
                      value={tempmailPin}
                      onChange={(e) => {
                        const value = e.target.value;
                        setTempmailPin(value);
                        setPinCode(value);
                      }}
                      className="mt-1 field-input sm:text-sm"
                      placeholder="请输入PIN码（如果有的话）"
                    />
                  </div>
                </div>
              )}

              {emailType === "self_hosted" && (
                <div className="space-y-4 sm:col-span-2">
                  <div className="p-3 rounded-md status-warning">
                    <p className="text-sm text-amber-900">
                      {activeSelfHostedDescription}
                    </p>
                    <p className="mt-2 text-xs text-amber-800">
                      响应路径示例：指向字符串字段，如{" "}
                      <code className="px-1 rounded bg-amber-100 dark:bg-amber-500/15 dark:text-amber-100">
                        results[0].raw
                      </code>{" "}
                      或{" "}
                      <code className="px-1 rounded bg-amber-100 dark:bg-amber-500/15 dark:text-amber-100">
                        results.0.raw
                      </code>
                      （与 JSON 结构一致即可）。
                    </p>
                  </div>

                  <div>
                    <label
                      htmlFor="self-hosted-url"
                      className="block text-sm font-medium text-slate-700 dark:text-slate-300"
                    >
                      请求 URL <span className="text-red-500">*</span>
                    </label>
                    <input
                      type="url"
                      id="self-hosted-url"
                      value={selfHostedMailUrl}
                      onChange={(e) => {
                        const v = e.target.value;
                        setSelfHostedMailUrl(v);
                        if (isCodexProvider) {
                          setCachedCodexSelfHostedMailUrl(v.trim());
                        } else {
                          setCachedSelfHostedMailUrl(v.trim());
                        }
                      }}
                      className="mt-1 font-mono text-sm field-input"
                      placeholder="https://example.com/api/mails?limit=1&offset=0"
                    />
                  </div>

                  <div>
                    <label
                      htmlFor="self-hosted-headers"
                      className="block text-sm font-medium text-slate-700 dark:text-slate-300"
                    >
                      请求 Headers（JSON 对象）
                      <span className="text-red-500">*</span>
                    </label>
                    <textarea
                      id="self-hosted-headers"
                      rows={5}
                      value={selfHostedMailHeadersJson}
                      onChange={(e) => {
                        const v = e.target.value;
                        setSelfHostedMailHeadersJson(v);
                        if (isCodexProvider) {
                          setCachedCodexSelfHostedMailHeadersJson(v);
                        } else {
                          setCachedSelfHostedMailHeadersJson(v);
                        }
                      }}
                      className="mt-1 font-mono text-xs field-input"
                      spellCheck={false}
                    />
                  </div>

                  <div>
                    <label
                      htmlFor="self-hosted-path"
                      className="block text-sm font-medium text-slate-700 dark:text-slate-300"
                    >
                      邮件原文字段路径 <span className="text-red-500">*</span>
                    </label>
                    <input
                      type="text"
                      id="self-hosted-path"
                      value={selfHostedMailResponsePath}
                      onChange={(e) => {
                        const v = e.target.value;
                        setSelfHostedMailResponsePath(v);
                        if (isCodexProvider) {
                          setCachedCodexSelfHostedMailResponsePath(v.trim());
                        } else {
                          setCachedSelfHostedMailResponsePath(v.trim());
                        }
                      }}
                      className="mt-1 font-mono text-sm field-input"
                      placeholder="results[0].raw"
                    />
                  </div>

                  <div className="p-3 space-y-4 border rounded-md surface-secondary border-subtle">
                    <div className="flex items-center">
                      <input
                        id="self-hosted-clear-enabled"
                        type="checkbox"
                        checked={selfHostedMailClearEnabled}
                        onChange={(e) => {
                          const checked = e.target.checked;
                          setSelfHostedMailClearEnabled(checked);
                          if (isCodexProvider) {
                            setCachedCodexSelfHostedMailClearEnabled(checked);
                          } else {
                            setCachedSelfHostedMailClearEnabled(checked);
                          }
                        }}
                        className="w-4 h-4 text-blue-600 rounded border-slate-300 focus:ring-blue-500 dark:border-slate-600 dark:bg-slate-900"
                      />
                      <label
                        htmlFor="self-hosted-clear-enabled"
                        className="block ml-2 text-sm font-medium text-slate-900 dark:text-slate-100"
                      >
                        获取验证码前先清空邮箱
                      </label>
                    </div>

                    {selfHostedMailClearEnabled && (
                      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
                        <div className="sm:col-span-2">
                          <label
                            htmlFor="self-hosted-clear-url"
                            className="block text-sm font-medium text-slate-700 dark:text-slate-300"
                          >
                            清空请求 URL <span className="text-red-500">*</span>
                          </label>
                          <input
                            type="url"
                            id="self-hosted-clear-url"
                            value={selfHostedMailClearUrl}
                            onChange={(e) => {
                              const v = e.target.value;
                              setSelfHostedMailClearUrl(v);
                              if (isCodexProvider) {
                                setCachedCodexSelfHostedMailClearUrl(v.trim());
                              } else {
                                setCachedSelfHostedMailClearUrl(v.trim());
                              }
                            }}
                            className="mt-1 font-mono text-sm field-input"
                            placeholder="https://example.com/api/mails/clear"
                          />
                        </div>

                        <div>
                          <label
                            htmlFor="self-hosted-clear-method"
                            className="block text-sm font-medium text-slate-700 dark:text-slate-300"
                          >
                            清空请求 Method{" "}
                            <span className="text-red-500">*</span>
                          </label>
                          <input
                            type="text"
                            id="self-hosted-clear-method"
                            value={selfHostedMailClearMethod}
                            onChange={(e) => {
                              const v = e.target.value.toUpperCase();
                              setSelfHostedMailClearMethod(v);
                              if (isCodexProvider) {
                                setCachedCodexSelfHostedMailClearMethod(v);
                              } else {
                                setCachedSelfHostedMailClearMethod(v);
                              }
                            }}
                            className="mt-1 font-mono text-sm field-input"
                            placeholder="DELETE"
                          />
                        </div>

                        <div className="sm:col-span-2">
                          <label
                            htmlFor="self-hosted-clear-headers"
                            className="block text-sm font-medium text-slate-700 dark:text-slate-300"
                          >
                            清空请求 Headers（JSON 对象）
                            <span className="text-red-500">*</span>
                          </label>
                          <textarea
                            id="self-hosted-clear-headers"
                            rows={5}
                            value={selfHostedMailClearHeadersJson}
                            onChange={(e) => {
                              const v = e.target.value;
                              setSelfHostedMailClearHeadersJson(v);
                              if (isCodexProvider) {
                                setCachedCodexSelfHostedMailClearHeadersJson(v);
                              } else {
                                setCachedSelfHostedMailClearHeadersJson(v);
                              }
                            }}
                            className="mt-1 font-mono text-xs field-input"
                            spellCheck={false}
                          />
                        </div>
                      </div>
                    )}
                  </div>
                </div>
              )}

              {isCodexProvider && (
                <div className="space-y-4 sm:col-span-2">
                  <div className="p-3 rounded-md status-info">
                    <p className="text-sm text-blue-800">
                      Codex CDP 参数可在这里直接覆盖。`incognito` 与
                      `custom-config-json` 仍由程序固定控制，这里只开放
                      URL、步骤、超时和附加脚本。
                    </p>
                    <p className="mt-2 text-xs text-blue-700">
                      支持的步骤格式：`click` / `input`。其中 `input.value`
                      可以继续使用
                      `__AUTO__`、`__RANDOM_EN_NAME__`、`__REGISTER_EMAIL__`、`__ACCESS_PASSWORD__`
                      这类占位符。点击步骤支持 `waitForLoad:
                      true`，用于点击后等待页面加载完成。
                    </p>
                  </div>

                  <div>
                    <div className="flex items-center justify-between">
                      <label
                        htmlFor="codex-cdp-overrides"
                        className="block text-sm font-medium text-slate-700 dark:text-slate-300"
                      >
                        Codex CDP 配置（JSON）
                      </label>
                      <Button
                        variant="secondary"
                        size="sm"
                        onClick={() => {
                          setCodexCdpOverridesJson(
                            DEFAULT_CODEX_CDP_OVERRIDES_JSON,
                          );
                          setCachedCodexCdpOverridesJson(
                            DEFAULT_CODEX_CDP_OVERRIDES_JSON,
                          );
                        }}
                      >
                        重置默认
                      </Button>
                    </div>
                    <textarea
                      id="codex-cdp-overrides"
                      rows={18}
                      value={codexCdpOverridesJson}
                      onChange={(e) => {
                        const value = e.target.value;
                        setCodexCdpOverridesJson(value);
                        setCachedCodexCdpOverridesJson(value);
                      }}
                      className="mt-1 font-mono text-xs field-input"
                      spellCheck={false}
                    />
                  </div>
                </div>
              )}

              {emailType === "outlook" && (
                <div className="space-y-4 sm:col-span-2">
                  {/* Outlook模式选择 */}
                  <div>
                    <label className="block mb-3 text-sm font-medium text-slate-700 dark:text-slate-300">
                      Outlook模式
                    </label>
                    <div className="space-y-2">
                      <div className="flex items-center">
                        <input
                          id="outlook-default"
                          name="outlook-mode"
                          type="radio"
                          value="default"
                          checked={outlookMode === "default"}
                          onChange={(e) =>
                            setOutlookMode(
                              e.target.value as "default" | "token",
                            )
                          }
                          className="w-4 h-4 text-blue-600 rounded border-slate-300 focus:ring-blue-500 dark:border-slate-600 dark:bg-slate-900"
                        />
                        <label
                          htmlFor="outlook-default"
                          className="ml-2 text-sm text-slate-700 dark:text-slate-300"
                        >
                          默认模式（只需输入邮箱）
                        </label>
                      </div>
                      <div className="flex items-center">
                        <input
                          id="outlook-token"
                          name="outlook-mode"
                          type="radio"
                          value="token"
                          checked={outlookMode === "token"}
                          onChange={(e) =>
                            setOutlookMode(
                              e.target.value as "default" | "token",
                            )
                          }
                          className="w-4 h-4 text-blue-600 rounded border-slate-300 focus:ring-blue-500 dark:border-slate-600 dark:bg-slate-900"
                          disabled
                        />
                        <label
                          htmlFor="outlook-token"
                          className="ml-2 text-sm text-slate-400 dark:text-slate-500"
                        >
                          令牌模式（TODO: 待实现）
                        </label>
                      </div>
                    </div>
                  </div>

                  {/* 默认模式配置 */}
                  {outlookMode === "default" && (
                    <div>
                      <label
                        htmlFor="outlook-email"
                        className="block text-sm font-medium text-slate-700 dark:text-slate-300"
                      >
                        Outlook邮箱地址
                      </label>
                      <input
                        type="email"
                        id="outlook-email"
                        value={outlookEmail}
                        onChange={(e) => setOutlookEmail(e.target.value)}
                        placeholder="example@outlook.com"
                        className="mt-1 field-input"
                      />
                      <p className="mt-1 text-sm text-muted">
                        请输入你的@outlook.com邮箱地址
                      </p>
                      <div className="p-3 mt-3 rounded-md status-success">
                        <p className="text-sm text-green-700">
                          📧 将自动获取该邮箱的验证码，无需手动输入
                        </p>
                      </div>
                    </div>
                  )}

                  {/* 令牌模式配置（预留） */}
                  {outlookMode === "token" && (
                    <div>
                      <label className="block text-sm font-medium text-slate-700 dark:text-slate-300">
                        令牌配置（格式：邮箱----密码----ID----令牌）
                      </label>
                      <textarea
                        rows={3}
                        placeholder="TODO: 令牌模式待实现"
                        className="mt-1 field-input"
                        disabled
                      />
                    </div>
                  )}
                </div>
              )}

              <div className="sm:col-span-2">
                <div className="flex items-center">
                  <input
                    id="use-incognito"
                    type="checkbox"
                    checked={useIncognito}
                    onChange={(e) => {
                      const value = e.target.checked;
                      setUseIncognito(value);
                      setCachedUseIncognito(value);
                    }}
                    className="w-4 h-4 text-blue-600 rounded border-slate-300 focus:ring-blue-500 dark:border-slate-600 dark:bg-slate-900"
                  />
                  <label
                    htmlFor="use-incognito"
                    className="ml-2 text-sm text-slate-700 dark:text-slate-300"
                  >
                    使用无痕模式（推荐）
                  </label>
                </div>
                <p className="mt-1 text-xs text-muted">
                  无痕模式可以避免浏览器缓存和历史记录影响注册过程
                </p>
              </div>

              {!isCodexProvider && (
                <>
                  <div className="sm:col-span-2">
                    <div className="flex items-center">
                      <input
                        id="enable-bank-card-binding"
                        type="checkbox"
                        checked={enableBankCardBinding}
                        onChange={(e) => {
                          const value = e.target.checked;
                          setEnableBankCardBinding(value);
                          setCachedEnableBankCardBinding(value);
                        }}
                        className="w-4 h-4 text-blue-600 rounded border-slate-300 focus:ring-blue-500 dark:border-slate-600 dark:bg-slate-900"
                      />
                      <label
                        htmlFor="enable-bank-card-binding"
                        className="ml-2 text-sm text-slate-700 dark:text-slate-300"
                      >
                        自动绑定银行卡（默认）
                      </label>
                    </div>
                    <p className="mt-1 text-xs text-muted">
                      勾选后将自动执行银行卡绑定流程，取消勾选则跳过银行卡绑定
                    </p>
                  </div>

                  {/* 订阅配置选项 */}
                  <div className="sm:col-span-2">
                    <div className="p-4 border border-purple-200 rounded-md bg-purple-50 dark:border-purple-500/25 dark:bg-purple-500/10">
                      <h4 className="mb-3 text-sm font-medium text-purple-800 dark:text-purple-100">
                        💎 订阅配置（绑卡相关）
                      </h4>

                      {/* 订阅层级选择 */}
                      <div className="mb-4">
                        <label className="block mb-2 text-sm font-medium text-slate-700 dark:text-slate-300">
                          订阅层级
                        </label>
                        <div className="space-y-2">
                          <div className="flex items-center">
                            <input
                              id="tier-pro"
                              name="subscription-tier"
                              type="radio"
                              value="pro"
                              checked={subscriptionTier === "pro"}
                              onChange={(e) =>
                                setSubscriptionTier(
                                  e.target.value as
                                    | "pro"
                                    | "pro_plus"
                                    | "ultra",
                                )
                              }
                              className="w-4 h-4 text-purple-600 rounded border-slate-300 focus:ring-purple-500 dark:border-slate-600 dark:bg-slate-900"
                            />
                            <label
                              htmlFor="tier-pro"
                              className="ml-2 text-sm text-slate-700 dark:text-slate-300"
                            >
                              Pro 试用版（默认）
                            </label>
                          </div>
                          <div className="flex items-center">
                            <input
                              id="tier-pro-plus"
                              name="subscription-tier"
                              type="radio"
                              value="pro_plus"
                              checked={subscriptionTier === "pro_plus"}
                              onChange={(e) =>
                                setSubscriptionTier(
                                  e.target.value as
                                    | "pro"
                                    | "pro_plus"
                                    | "ultra",
                                )
                              }
                              className="w-4 h-4 text-purple-600 rounded border-slate-300 focus:ring-purple-500 dark:border-slate-600 dark:bg-slate-900"
                            />
                            <label
                              htmlFor="tier-pro-plus"
                              className="ml-2 text-sm text-slate-700 dark:text-slate-300"
                            >
                              Pro Plus 试用版
                            </label>
                          </div>
                          <div className="flex items-center">
                            <input
                              id="tier-ultra"
                              name="subscription-tier"
                              type="radio"
                              value="ultra"
                              checked={subscriptionTier === "ultra"}
                              onChange={(e) =>
                                setSubscriptionTier(
                                  e.target.value as
                                    | "pro"
                                    | "pro_plus"
                                    | "ultra",
                                )
                              }
                              className="w-4 h-4 text-purple-600 rounded border-slate-300 focus:ring-purple-500 dark:border-slate-600 dark:bg-slate-900"
                            />
                            <label
                              htmlFor="tier-ultra"
                              className="ml-2 text-sm text-slate-700 dark:text-slate-300"
                            >
                              Ultra 版本
                            </label>
                          </div>
                        </div>
                      </div>

                      {/* 自动续费选项 */}
                      <div className="mb-4">
                        <div className="flex items-center">
                          <input
                            id="allow-automatic-payment"
                            type="checkbox"
                            checked={allowAutomaticPayment}
                            onChange={(e) =>
                              setAllowAutomaticPayment(e.target.checked)
                            }
                            className="w-4 h-4 text-purple-600 rounded border-slate-300 dark:border-slate-700 focus:ring-purple-500"
                          />
                          <label
                            htmlFor="allow-automatic-payment"
                            className="ml-2 text-sm text-slate-700 dark:text-slate-300"
                          >
                            允许自动续费
                          </label>
                        </div>
                        <p className="mt-1 text-xs text-muted">
                          勾选后将允许订阅到期时自动续费
                        </p>
                      </div>

                      {/* 试用选项 */}
                      <div className="mb-4">
                        <div className="flex items-center">
                          <input
                            id="allow-trial"
                            type="checkbox"
                            checked={allowTrial}
                            onChange={(e) => setAllowTrial(e.target.checked)}
                            className="w-4 h-4 text-purple-600 rounded border-slate-300 dark:border-slate-700 focus:ring-purple-500"
                          />
                          <label
                            htmlFor="allow-trial"
                            className="ml-2 text-sm text-slate-700 dark:text-slate-300"
                          >
                            开启试用
                          </label>
                        </div>
                        <p className="mt-1 text-xs text-muted">
                          勾选后将开启试用期，取消勾选则直接付费订阅
                        </p>
                      </div>

                      {/* 绑卡方式选择 */}
                      <div>
                        <label className="block mb-2 text-sm font-medium text-slate-700 dark:text-slate-300">
                          绑卡方式
                        </label>
                        <div className="space-y-2">
                          <div className="flex items-center">
                            <input
                              id="use-api-bind-card"
                              name="bind-card-method"
                              type="radio"
                              value={1}
                              checked={useApiForBindCard === 1}
                              onChange={() => setUseApiForBindCard(1)}
                              className="w-4 h-4 text-purple-600 rounded border-slate-300 focus:ring-purple-500 dark:border-slate-600 dark:bg-slate-900"
                            />
                            <label
                              htmlFor="use-api-bind-card"
                              className="ml-2 text-sm text-slate-700 dark:text-slate-300"
                            >
                              使用接口获取绑卡地址（推荐）
                            </label>
                          </div>
                          <div className="flex items-center">
                            <input
                              id="use-simulate-bind-card"
                              name="bind-card-method"
                              type="radio"
                              value={2}
                              checked={useApiForBindCard === 2}
                              onChange={() => setUseApiForBindCard(2)}
                              className="w-4 h-4 text-purple-600 rounded border-slate-300 focus:ring-purple-500 dark:border-slate-600 dark:bg-slate-900"
                            />
                            <label
                              htmlFor="use-simulate-bind-card"
                              className="ml-2 text-sm text-slate-700 dark:text-slate-300"
                            >
                              模拟页面点击获取绑卡地址
                            </label>
                          </div>
                        </div>
                        <p className="mt-1 text-xs text-muted">
                          选择获取绑卡地址的方式：接口方式更快更稳定，模拟点击方式更接近真实用户行为
                        </p>
                      </div>
                    </div>
                  </div>

                  <div className="sm:col-span-2">
                    <div className="flex items-center">
                      <input
                        id="is-us-account"
                        type="checkbox"
                        checked={isUsAccount}
                        onChange={(e) => setIsUsAccount(e.target.checked)}
                        className="w-4 h-4 text-blue-600 rounded border-slate-300 focus:ring-blue-500 dark:border-slate-600 dark:bg-slate-900"
                      />
                      <label
                        htmlFor="is-us-account"
                        className="ml-2 text-sm text-slate-700 dark:text-slate-300"
                      >
                        注册美国账户
                      </label>
                    </div>
                    <p className="mt-1 text-xs text-muted">
                      勾选后将选择美国地区的付款方式（按钮索引1），否则使用默认地区（按钮索引0）
                    </p>
                  </div>

                  <div className="sm:col-span-2">
                    <div className="flex items-center">
                      <input
                        id="skip-phone-verification"
                        type="checkbox"
                        checked={skipPhoneVerification}
                        onChange={(e) =>
                          setSkipPhoneVerification(e.target.checked)
                        }
                        className="w-4 h-4 text-blue-600 rounded border-slate-300 focus:ring-blue-500 dark:border-slate-600 dark:bg-slate-900"
                      />
                      <label
                        htmlFor="skip-phone-verification"
                        className="ml-2 text-sm text-slate-700 dark:text-slate-300"
                      >
                        跳过手机号验证（实验性功能）
                      </label>
                    </div>
                    <p className="mt-1 text-xs text-muted">
                      启用后将使用验证码登录方式跳过手机号验证，适用于无法接收短信的情况
                    </p>
                  </div>
                </>
              )}

              {/* 代理配置选项 */}
              <div className="sm:col-span-2">
                <div className="p-4 border border-orange-200 rounded-md bg-orange-50 dark:border-orange-500/25 dark:bg-orange-500/10">
                  <h4 className="mb-3 text-sm font-medium text-orange-800 dark:text-orange-100">
                    🌐 代理配置
                  </h4>
                  <p className="mb-3 text-xs text-orange-600">
                    💾 配置会自动保存到本地，下次打开时会自动恢复
                  </p>

                  {/* 启用代理选项 */}
                  <div className="mb-4">
                    <div className="flex items-center">
                      <input
                        id="enable-proxy"
                        type="checkbox"
                        checked={proxyEnabled}
                        onChange={(e) => setProxyEnabled(e.target.checked)}
                        className="w-4 h-4 text-orange-600 rounded border-slate-300 focus:ring-orange-500 dark:border-slate-600 dark:bg-slate-900"
                      />
                      <label
                        htmlFor="enable-proxy"
                        className="ml-2 text-sm text-slate-700 dark:text-slate-300"
                      >
                        启用代理
                      </label>
                    </div>
                    <p className="mt-1 text-xs text-muted">
                      勾选后将使用代理进行网络连接
                    </p>
                  </div>

                  {/* 代理配置详情 */}
                  {proxyEnabled && (
                    <div className="space-y-4">
                      {/* 代理类型选择 */}
                      <div>
                        <label className="block mb-2 text-sm font-medium text-slate-700 dark:text-slate-300">
                          代理类型
                        </label>
                        <div className="space-y-2">
                          <div className="flex items-center">
                            <input
                              id="proxy-http"
                              name="proxy-type"
                              type="radio"
                              value="http"
                              checked={proxyType === "http"}
                              onChange={(e) =>
                                setProxyType(
                                  e.target.value as "http" | "socks" | "vless",
                                )
                              }
                              className="w-4 h-4 text-orange-600 border-slate-300 dark:border-slate-700 focus:ring-orange-500"
                            />
                            <label
                              htmlFor="proxy-http"
                              className="ml-2 text-sm text-slate-700 dark:text-slate-300"
                            >
                              HTTP/HTTPS 代理
                            </label>
                          </div>
                          <div className="flex items-center">
                            <input
                              id="proxy-socks"
                              name="proxy-type"
                              type="radio"
                              value="socks"
                              checked={proxyType === "socks"}
                              onChange={(e) =>
                                setProxyType(
                                  e.target.value as "http" | "socks" | "vless",
                                )
                              }
                              className="w-4 h-4 text-orange-600 border-slate-300 dark:border-slate-700 focus:ring-orange-500"
                            />
                            <label
                              htmlFor="proxy-socks"
                              className="ml-2 text-sm text-slate-700 dark:text-slate-300"
                            >
                              SOCKS 代理
                            </label>
                          </div>
                          <div className="flex items-center">
                            <input
                              id="proxy-vless"
                              name="proxy-type"
                              type="radio"
                              value="vless"
                              checked={proxyType === "vless"}
                              onChange={(e) =>
                                setProxyType(
                                  e.target.value as "http" | "socks" | "vless",
                                )
                              }
                              className="w-4 h-4 text-orange-600 border-slate-300 dark:border-slate-700 focus:ring-orange-500"
                            />
                            <label
                              htmlFor="proxy-vless"
                              className="ml-2 text-sm text-slate-700 dark:text-slate-300"
                            >
                              VLESS（Xray 转 HTTP/SOCKS）
                            </label>
                          </div>
                        </div>
                      </div>

                      {/* HTTP 代理配置 */}
                      {proxyType === "http" && (
                        <div>
                          <label
                            htmlFor="http-proxy"
                            className="block text-sm font-medium text-slate-700 dark:text-slate-300"
                          >
                            HTTP 代理地址
                          </label>
                          <input
                            type="text"
                            id="http-proxy"
                            value={httpProxy}
                            onChange={(e) => setHttpProxy(e.target.value)}
                            placeholder="127.0.0.1:7890"
                            className="mt-1 field-input focus:ring-orange-500"
                          />
                          <p className="mt-1 text-xs text-muted">
                            格式：IP:端口 或 域名:端口
                          </p>
                        </div>
                      )}

                      {/* SOCKS 代理配置 */}
                      {proxyType === "socks" && (
                        <div>
                          <label
                            htmlFor="socks-proxy"
                            className="block text-sm font-medium text-slate-700 dark:text-slate-300"
                          >
                            SOCKS 代理地址
                          </label>
                          <input
                            type="text"
                            id="socks-proxy"
                            value={socksProxy}
                            onChange={(e) => setSocksProxy(e.target.value)}
                            placeholder="127.0.0.1:1080"
                            className="mt-1 field-input focus:ring-orange-500"
                          />
                          <p className="mt-1 text-xs text-muted">
                            格式：IP:端口 或 域名:端口
                          </p>
                        </div>
                      )}

                      {proxyType === "vless" && (
                        <div className="space-y-3">
                          <div>
                            <label
                              htmlFor="vless-url"
                              className="block text-sm font-medium text-slate-700 dark:text-slate-300"
                            >
                              VLESS 标准链接
                            </label>
                            <input
                              type="text"
                              id="vless-url"
                              value={vlessUrl}
                              onChange={(e) => setVlessUrl(e.target.value)}
                              placeholder="vless://uuid@host:port?security=reality&..."
                              className="mt-1 field-input focus:ring-orange-500"
                            />
                            <p className="mt-1 text-xs text-muted">
                              自动用 xray 在本地开启 HTTP/SOCKS
                              代理端口供浏览器使用
                            </p>
                          </div>

                          <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
                            <div>
                              <label
                                htmlFor="xray-http-port"
                                className="block text-sm font-medium text-slate-700 dark:text-slate-300"
                              >
                                本地 HTTP 端口
                              </label>
                              <input
                                type="number"
                                id="xray-http-port"
                                value={xrayHttpPort}
                                onChange={(e) =>
                                  setXrayHttpPort(
                                    Number.parseInt(e.target.value || "0", 10),
                                  )
                                }
                                className="mt-1 field-input focus:ring-orange-500"
                              />
                            </div>
                            <div>
                              <label
                                htmlFor="xray-socks-port"
                                className="block text-sm font-medium text-slate-700 dark:text-slate-300"
                              >
                                本地 SOCKS 端口
                              </label>
                              <input
                                type="number"
                                id="xray-socks-port"
                                value={xraySocksPort}
                                onChange={(e) =>
                                  setXraySocksPort(
                                    Number.parseInt(e.target.value || "0", 10),
                                  )
                                }
                                className="mt-1 field-input focus:ring-orange-500"
                              />
                            </div>
                          </div>
                          <p className="text-xs text-muted">
                            端口范围需为 1-65535，超出范围会自动回退到 HTTP 8991
                            / SOCKS 1990
                          </p>

                          <div className="flex flex-wrap gap-2">
                            <button
                              type="button"
                              onClick={testAndStartVlessProxy}
                              disabled={isTestingVlessProxy}
                              className="px-3 py-1 text-xs text-orange-700 bg-orange-100 border border-orange-300 rounded-md hover:bg-orange-200 disabled:cursor-not-allowed disabled:opacity-60 focus:outline-none focus:ring-2 focus:ring-orange-500 dark:border-orange-500/30 dark:bg-orange-500/15 dark:text-orange-200 dark:hover:bg-orange-500/25"
                            >
                              {isTestingVlessProxy
                                ? "⏳ 启动中..."
                                : "🚀 手动启动测试"}
                            </button>
                            {isTestingVlessProxy && (
                              <button
                                type="button"
                                onClick={cancelVlessDownload}
                                className="px-3 py-1 text-xs border rounded-md border-rose-300 bg-rose-100 text-rose-700 hover:bg-rose-200 focus:outline-none focus:ring-2 focus:ring-rose-500 dark:border-rose-500/30 dark:bg-rose-500/15 dark:text-rose-200 dark:hover:bg-rose-500/25"
                              >
                                🛑 取消下载
                              </button>
                            )}
                            <button
                              type="button"
                              onClick={() =>
                                copyText(
                                  vlessRuntimeProxy?.httpProxy ||
                                    `127.0.0.1:${xrayHttpPort}`,
                                  "HTTP 代理地址",
                                )
                              }
                              className="px-3 py-1 text-xs bg-white border rounded-md border-slate-300 text-slate-700 hover:bg-slate-100 focus:outline-none focus:ring-2 focus:ring-orange-500 dark:border-slate-600 dark:bg-slate-900 dark:text-slate-200 dark:hover:bg-slate-800"
                            >
                              复制 HTTP 地址
                            </button>
                            <button
                              type="button"
                              onClick={() =>
                                copyText(
                                  vlessRuntimeProxy?.socksProxy ||
                                    `127.0.0.1:${xraySocksPort}`,
                                  "SOCKS 代理地址",
                                )
                              }
                              className="px-3 py-1 text-xs bg-white border rounded-md border-slate-300 text-slate-700 hover:bg-slate-100 focus:outline-none focus:ring-2 focus:ring-orange-500 dark:border-slate-600 dark:bg-slate-900 dark:text-slate-200 dark:hover:bg-slate-800"
                            >
                              复制 SOCKS 地址
                            </button>
                          </div>

                          {vlessDownloadProgress && (
                            <div className="space-y-1">
                              <p className="text-xs text-slate-600 dark:text-slate-300">
                                {vlessDownloadProgress.message}
                                {typeof vlessDownloadProgress.percent ===
                                "number"
                                  ? ` (${vlessDownloadProgress.percent.toFixed(1)}%)`
                                  : ""}
                              </p>
                              {typeof vlessDownloadProgress.percent ===
                                "number" && (
                                <div className="w-full h-2 rounded bg-slate-200 dark:bg-slate-700">
                                  <div
                                    className="h-2 transition-all bg-orange-500 rounded"
                                    style={{
                                      width: `${Math.max(
                                        0,
                                        Math.min(
                                          100,
                                          vlessDownloadProgress.percent,
                                        ),
                                      )}%`,
                                    }}
                                  />
                                </div>
                              )}
                            </div>
                          )}

                          {vlessRuntimeProxy && (
                            <div className="p-2 text-xs border rounded-md border-emerald-200 bg-emerald-50 text-emerald-700 dark:border-emerald-500/30 dark:bg-emerald-500/10 dark:text-emerald-200">
                              当前可用：HTTP `{vlessRuntimeProxy.httpProxy}` /
                              SOCKS `{vlessRuntimeProxy.socksProxy}`
                            </div>
                          )}
                        </div>
                      )}

                      {/* 代理绕过列表 */}
                      <div>
                        <label
                          htmlFor="no-proxy"
                          className="block text-sm font-medium text-slate-700 dark:text-slate-300"
                        >
                          代理绕过列表
                        </label>
                        <input
                          type="text"
                          id="no-proxy"
                          value={noProxy}
                          onChange={(e) => setNoProxy(e.target.value)}
                          placeholder="localhost,127.0.0.1"
                          className="mt-1 field-input focus:ring-orange-500"
                        />
                        <p className="mt-1 text-xs text-muted">
                          不使用代理的地址列表，用逗号分隔
                        </p>
                      </div>

                      {/* 重置代理配置按钮 */}
                      <div className="flex justify-end">
                        <button
                          type="button"
                          onClick={async () => {
                            try {
                              const confirmed = await confirm(
                                "确定要重置代理配置吗？这将清除所有自定义设置并恢复为默认值。",
                                {
                                  title: "重置代理配置",
                                  kind: "warning",
                                  cancelLabel: "取消",
                                  okLabel: "确定",
                                },
                              );
                              if (confirmed) {
                                resetProxyConfig();
                                setToast({
                                  message: "代理配置已重置为默认值",
                                  type: "success",
                                });
                              }
                            } catch (error) {
                              console.error("重置配置确认弹窗失败:", error);
                              setToast({
                                message: "重置配置失败，请重试",
                                type: "error",
                              });
                            }
                          }}
                          className="px-3 py-1 text-xs text-orange-700 bg-orange-100 border border-orange-300 rounded-md hover:bg-orange-200 focus:outline-none focus:ring-2 focus:ring-orange-500 dark:border-orange-500/30 dark:bg-orange-500/15 dark:text-orange-200 dark:hover:bg-orange-500/25"
                        >
                          🔄 重置配置
                        </button>
                      </div>
                    </div>
                  )}
                </div>
              </div>

              <div className="sm:col-span-2">
                <label
                  htmlFor="password"
                  className="block text-sm font-medium text-slate-700 dark:text-slate-300"
                >
                  密码
                </label>
                <div className="relative mt-1">
                  <input
                    type={showPassword ? "text" : "password"}
                    id="password"
                    value={form.password}
                    onChange={(e) =>
                      handleInputChange("password", e.target.value)
                    }
                    disabled={useRandomInfo}
                    className="pr-10 field-input sm:text-sm disabled:bg-slate-100 dark:disabled:bg-slate-800"
                    placeholder="请输入密码（至少8位）"
                  />
                  <button
                    type="button"
                    className="absolute inset-y-0 right-0 flex items-center pr-3"
                    onClick={() => setShowPassword(!showPassword)}
                  >
                    {showPassword ? (
                      <svg
                        className="w-5 h-5 text-slate-400 dark:text-slate-500"
                        fill="none"
                        stroke="currentColor"
                        viewBox="0 0 24 24"
                      >
                        <path
                          strokeLinecap="round"
                          strokeLinejoin="round"
                          strokeWidth={2}
                          d="M13.875 18.825A10.05 10.05 0 0112 19c-4.478 0-8.268-2.943-9.543-7a9.97 9.97 0 011.563-3.029m5.858.908a3 3 0 114.243 4.243M9.878 9.878l4.242 4.242M9.878 9.878L3 3m6.878 6.878L21 21"
                        />
                      </svg>
                    ) : (
                      <svg
                        className="w-5 h-5 text-slate-400 dark:text-slate-500"
                        fill="none"
                        stroke="currentColor"
                        viewBox="0 0 24 24"
                      >
                        <path
                          strokeLinecap="round"
                          strokeLinejoin="round"
                          strokeWidth={2}
                          d="M15 12a3 3 0 11-6 0 3 3 0 016 0z"
                        />
                        <path
                          strokeLinecap="round"
                          strokeLinejoin="round"
                          strokeWidth={2}
                          d="M2.458 12C3.732 7.943 7.523 5 12 5c4.478 0 8.268 2.943 9.542 7-1.274 4.057-5.064 7-9.542 7-4.477 0-8.268-2.943-9.542-7z"
                        />
                      </svg>
                    )}
                  </button>
                </div>
              </div>
            </div>

            {/* 邮箱配置状态 */}
            {!isCodexProvider && emailConfig && (
              <div className="p-4 rounded-md status-success">
                <div className="flex items-center justify-between">
                  <div>
                    <h5 className="text-sm font-medium text-green-800">
                      📧 邮箱配置状态
                    </h5>
                    <p className="mt-1 text-sm text-green-700">
                      Worker域名: {emailConfig.worker_domain || "未配置"} |
                      邮箱域名: {emailConfig.email_domain || "未配置"} |
                      管理员密码:{" "}
                      {emailConfig.admin_password ? "已配置" : "未配置"} |
                      访问密码:{" "}
                      {emailConfig.access_password ? "已配置" : "未配置"}
                    </p>
                  </div>
                  <Button
                    onClick={() => setShowEmailConfig(true)}
                    variant="secondary"
                    size="sm"
                  >
                    编辑
                  </Button>
                </div>
              </div>
            )}

            {/* 银行卡配置状态 */}
            {!isCodexProvider && bankCardConfig && (
              <div className="p-4 rounded-md status-info">
                <div className="flex items-center justify-between">
                  <div>
                    <h5 className="text-sm font-medium text-blue-800">
                      💳 银行卡配置状态
                    </h5>
                    <p className="mt-1 text-sm text-blue-700">
                      卡号:{" "}
                      {bankCardConfig.cardNumber
                        ? `${bankCardConfig.cardNumber.slice(
                            0,
                            4,
                          )}****${bankCardConfig.cardNumber.slice(-4)}`
                        : "未配置"}{" "}
                      | 持卡人: {bankCardConfig.billingName || "未配置"} | 地址:{" "}
                      {bankCardConfig.billingAdministrativeArea || "未配置"}
                    </p>
                  </div>
                  <Button
                    onClick={() => setShowBankCardConfig(true)}
                    variant="secondary"
                    size="sm"
                  >
                    编辑
                  </Button>
                </div>
              </div>
            )}

            {/* 银行卡选择（单个注册用） */}
            {!isCodexProvider &&
              enableBankCardBinding &&
              bankCardList.length > 0 && (
                <div className="p-4 rounded-md status-info">
                  <div className="flex items-center justify-between mb-3">
                    <h5 className="text-sm font-medium text-blue-800">
                      💳 选择银行卡（单个注册）
                    </h5>
                    <div className="text-xs text-blue-700">
                      已选：卡片 {selectedCardIndex + 1}
                    </div>
                  </div>
                  <div className="flex gap-2 overflow-x-auto">
                    {bankCardList.map((card, index) => (
                      <div
                        key={index}
                        className={`relative flex-shrink-0 p-3 border-2 rounded-md cursor-pointer transition-all ${
                          selectedCardIndex === index
                            ? "border-blue-500 bg-blue-50 dark:border-blue-500/40 dark:bg-blue-500/12"
                            : "surface-elevated border-slate-300 dark:border-slate-700 hover:border-slate-400 dark:hover:border-slate-500"
                        }`}
                        onClick={() => handleSingleCardSelection(index)}
                      >
                        <div className="text-sm font-medium">
                          卡片 {index + 1}
                        </div>
                        <div className="mt-1 text-xs text-slate-600 dark:text-slate-300">
                          {card.cardNumber
                            ? `****${card.cardNumber.slice(-4)}`
                            : "未设置"}
                        </div>
                        {selectedCardIndex === index && (
                          <div className="absolute text-blue-600 top-1 right-1">
                            ✓
                          </div>
                        )}
                      </div>
                    ))}
                  </div>
                  <p className="mt-2 text-xs text-slate-600 dark:text-slate-300">
                    💡 点击卡片选择，单个注册将使用选中的银行卡
                  </p>
                </div>
              )}

            {/* 操作按钮 */}
            <div className="flex space-x-4">
              {useRandomInfo && (
                <Button
                  onClick={handleGenerateRandom}
                  variant="secondary"
                  disabled={isLoading}
                >
                  🎲 重新生成随机信息
                </Button>
              )}

              <Button
                onClick={handleRegister}
                disabled={isLoading}
                className="flex items-center"
              >
                {isLoading ? (
                  <>
                    <LoadingSpinner size="sm" />
                    注册中...
                  </>
                ) : (
                  `🚀 开始注册 ${activeProviderLabel}`
                )}
              </Button>
              {isRegistering && (
                <Button
                  onClick={handleCancelRegistration}
                  variant="secondary"
                  className="flex items-center text-red-700 border-red-300 hover:bg-red-50 dark:border-red-500/30 dark:text-red-200 dark:hover:bg-red-500/15"
                >
                  ⛔ 停止注册
                </Button>
              )}
            </div>

            {/* 批量注册 */}
            <div className="p-4 mt-6 border-t-2 border-blue-200">
              <h4 className="mb-3 text-sm font-medium text-slate-700 dark:text-slate-300">
                📦 批量注册（实验性功能）
              </h4>
              <div className="space-y-4">
                <div className="flex items-center gap-4">
                  <div className="flex-1">
                    <label className="block mb-1 text-sm text-subtle">
                      注册数量
                    </label>
                    <input
                      type="number"
                      min="1"
                      max="30"
                      value={batchCount}
                      onChange={(e) => {
                        const value = parseInt(e.target.value) || 1;
                        setBatchCount(Math.min(value, 30)); // 限制最大值为30
                      }}
                      className="field-input"
                      placeholder="输入注册数量 (1-3)"
                      disabled={isLoading}
                    />
                    <p className="mt-1 text-xs text-muted">
                      ⚠️ 需要配置相同数量的
                      {!isCodexProvider && "银行卡和"}
                      {emailType === "custom" ||
                      emailType === "tempmail" ||
                      emailType === "self_hosted"
                        ? "邮箱"
                        : "注册资源"}
                    </p>

                    {/* 并行模式开关 */}
                    <div className="flex items-center gap-2 mt-2">
                      <input
                        type="checkbox"
                        id="useParallelMode"
                        checked={useParallelMode}
                        onChange={(e) => {
                          const value = e.target.checked;
                          setUseParallelMode(value);
                          setCachedUseParallelMode(value);
                        }}
                        className="w-4 h-4 text-blue-600 rounded border-slate-300 focus:ring-blue-500 dark:border-slate-600 dark:bg-slate-900"
                        disabled={isLoading}
                      />
                      <label
                        htmlFor="useParallelMode"
                        className="text-sm cursor-pointer text-slate-700 dark:text-slate-300"
                      >
                        🚀 并行模式（多窗口同时注册，速度更快）
                      </label>
                    </div>
                    <p className="mt-1 text-xs text-muted">
                      💡 串行模式：一个接一个注册，更稳定
                      <br />
                      💡 并行模式：每个账户独立窗口，同时注册，速度更快
                    </p>
                    {!useParallelMode && (
                      <div className="mt-3">
                        <label className="block mb-1 text-sm text-subtle">
                          串行间隔秒数
                        </label>
                        <input
                          type="number"
                          min="0"
                          max="600"
                          step="1"
                          value={batchSerialDelaySeconds}
                          onChange={(e) => {
                            const rawValue = parseInt(e.target.value, 10);
                            const nextValue = Number.isNaN(rawValue)
                              ? DEFAULT_BATCH_SERIAL_DELAY_SECONDS
                              : Math.max(0, Math.min(rawValue, 600));
                            setBatchSerialDelaySeconds(nextValue);
                            setCachedBatchSerialDelaySeconds(nextValue);
                          }}
                          className="field-input"
                          placeholder="输入下一个任务前等待的秒数"
                          disabled={isLoading}
                        />
                        <p className="mt-1 text-xs text-muted">
                          第 1 个任务立即开始，第 2
                          个起每次都会等待这里设置的秒数后再打开执行。
                        </p>
                      </div>
                    )}
                  </div>
                  <div className="flex-shrink-0 pt-6">
                    <div className="flex items-center gap-2">
                      <Button
                        onClick={handleBatchRegister}
                        disabled={isLoading || batchCount < 1}
                        className="flex items-center"
                      >
                        {isLoading ? (
                          <>
                            <LoadingSpinner size="sm" />
                            批量注册中...
                          </>
                        ) : (
                          `🚀 批量注册 (${batchCount})`
                        )}
                      </Button>
                      {isRegistering && (
                        <Button
                          onClick={handleCancelRegistration}
                          variant="secondary"
                          className="flex items-center text-red-700 border-red-300 hover:bg-red-50 dark:border-red-500/30 dark:text-red-200 dark:hover:bg-red-500/15"
                        >
                          ⛔ 停止注册
                        </Button>
                      )}
                    </div>
                  </div>
                </div>

                {/* 需要输入邮箱列表时显示 */}
                {(emailType === "custom" ||
                  emailType === "tempmail" ||
                  emailType === "self_hosted") && (
                  <div className="space-y-2">
                    <label className="block text-sm font-medium text-slate-700 dark:text-slate-300">
                      📧 注册邮箱列表
                      {emailType === "tempmail" && (
                        <span className="ml-2 text-xs text-blue-600">
                          (验证码通过上面配置的Tempmail邮箱接收，批量注册前会自动清空临时邮箱)
                        </span>
                      )}
                      {emailType === "self_hosted" && (
                        <span className="ml-2 text-xs text-amber-700">
                          （验证码通过上方自建邮箱 API 拉取，请保证 API
                          能返回对应邮箱的最新邮件）
                        </span>
                      )}
                    </label>
                    <div className="grid grid-cols-1 gap-2 p-3 overflow-y-auto rounded-md surface-secondary max-h-60">
                      {Array.from({ length: batchCount }).map((_, index) => (
                        <div key={index} className="flex items-center gap-2">
                          <span className="flex-shrink-0 w-8 text-xs font-medium text-muted">
                            #{index + 1}
                          </span>
                          <input
                            type="email"
                            value={batchEmails[index] || ""}
                            onChange={(e) => {
                              const newEmails = [...batchEmails];
                              newEmails[index] = e.target.value;
                              setBatchEmails(newEmails);
                            }}
                            className="flex-1 field-input"
                            placeholder={`请输入第 ${index + 1} 个真实邮箱地址`}
                            disabled={isLoading}
                          />
                        </div>
                      ))}
                    </div>
                  </div>
                )}

                {/* 邮箱类型提示（需单独说明的类型） */}
                {(emailType === "cloudflare_temp" ||
                  emailType === "outlook") && (
                  <div className="p-3 rounded-md status-info">
                    <p className="text-sm text-blue-700">
                      {emailType === "cloudflare_temp"
                        ? "💡 将自动为每个账号生成独立的临时邮箱"
                        : "💡 将使用配置的 Outlook 邮箱进行批量注册"}
                    </p>
                  </div>
                )}

                {/* 银行卡选择（批量注册用） */}
                {!isCodexProvider &&
                  enableBankCardBinding &&
                  bankCardList.length > 0 && (
                    <div className="p-4 rounded-md status-success">
                      <div className="flex items-center justify-between mb-3">
                        <h5 className="text-sm font-medium text-green-800">
                          💳 选择银行卡（批量注册）
                        </h5>
                        <div className="text-xs text-green-700">
                          已选 {selectedBatchCardIndices.length}/
                          {bankCardList.length} 张
                        </div>
                      </div>
                      <div className="flex gap-2 overflow-x-auto">
                        {bankCardList.map((card, index) => (
                          <div
                            key={index}
                            className={`relative flex-shrink-0 p-3 border-2 rounded-md cursor-pointer transition-all ${
                              selectedBatchCardIndices.includes(index)
                                ? "border-green-500 bg-green-50 dark:border-green-500/40 dark:bg-green-500/12"
                                : "surface-elevated border-slate-300 dark:border-slate-700 hover:border-slate-400 dark:hover:border-slate-500"
                            }`}
                            onClick={() => handleBatchCardSelection(index)}
                          >
                            <div className="text-sm font-medium">
                              卡片 {index + 1}
                            </div>
                            <div className="mt-1 text-xs text-slate-600 dark:text-slate-300">
                              {card.cardNumber
                                ? `****${card.cardNumber.slice(-4)}`
                                : "未设置"}
                            </div>
                            {selectedBatchCardIndices.includes(index) && (
                              <div className="absolute text-green-600 top-1 right-1">
                                ✓
                              </div>
                            )}
                          </div>
                        ))}
                      </div>
                      <p className="mt-2 text-xs text-slate-600 dark:text-slate-300">
                        💡
                        点击卡片选择/取消选择，批量注册将按顺序使用选中的银行卡
                      </p>
                    </div>
                  )}
              </div>
            </div>

            {/* 注册结果 */}
            {codexManualAction && !showVerificationModal && (
              <div className="p-4 border rounded-md shadow-sm border-amber-300 bg-amber-50/95 dark:border-amber-500/40 dark:bg-amber-500/10">
                <div className="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
                  <div className="space-y-2">
                    <h4 className="text-sm font-semibold text-amber-900 dark:text-amber-100">
                      Codex 已暂停在 Step1 自动执行前
                    </h4>
                    <p className="text-sm text-amber-800 dark:text-amber-100/90">
                      {codexManualAction.message ||
                        "如果自动注册没有成功，请先在当前浏览器里手动完成注册，然后点击“手动确认注册完成并执行 Step1”。"}
                    </p>
                    <div className="flex flex-wrap gap-3 text-xs text-amber-900/80 dark:text-amber-100/80">
                      {codexManualAction.email && (
                        <span className="px-3 py-1 font-mono rounded-full bg-white/70 dark:bg-slate-900/40">
                          邮箱: {codexManualAction.email}
                        </span>
                      )}
                      {codexManualAction.task_id && (
                        <span className="px-3 py-1 font-mono rounded-full bg-white/70 dark:bg-slate-900/40">
                          Task ID: {codexManualAction.task_id}
                        </span>
                      )}
                    </div>
                  </div>
                  <div className="flex flex-wrap gap-3">
                    <button
                      type="button"
                      onClick={() =>
                        handleCodexManualStepAction("manual_confirm_complete")
                      }
                      disabled={manualActionLoading}
                      className="px-4 py-2 text-sm font-medium text-white border rounded-md border-emerald-500 bg-emerald-600 hover:bg-emerald-700 disabled:cursor-not-allowed disabled:opacity-50"
                    >
                      手动触发 Step1
                    </button>
                  </div>
                </div>
              </div>
            )}

            {registrationResult && (
              <div
                className={`rounded-md p-4 ${
                  isRegistrationSuccessResult(registrationResult)
                    ? "status-success"
                    : "status-error"
                }`}
              >
                <h4
                  className={`text-sm font-medium ${
                    isRegistrationSuccessResult(registrationResult)
                      ? "text-green-800"
                      : "text-red-800"
                  }`}
                >
                  {isRegistrationSuccessResult(registrationResult)
                    ? "✅ 注册成功"
                    : "❌ 注册失败"}
                </h4>
                <p
                  className={`mt-1 text-sm ${
                    isRegistrationSuccessResult(registrationResult)
                      ? "text-green-700"
                      : "text-red-700"
                  }`}
                >
                  {registrationResult.message}
                </p>
                {registrationResult.accountInfo && (
                  <div className="p-3 mt-3 border rounded surface-elevated">
                    <h5 className="mb-2 text-sm font-medium text-slate-900 dark:text-slate-100">
                      账户信息：
                    </h5>
                    <div className="space-y-1 text-sm text-slate-700 dark:text-slate-300">
                      <div>
                        <strong>邮箱：</strong>{" "}
                        {registrationResult.accountInfo.email}
                      </div>
                      <div>
                        <strong>Token：</strong>{" "}
                        <span className="font-mono text-xs break-all">
                          {registrationResult.accountInfo.token}
                        </span>
                      </div>
                      <div>
                        <strong>使用限制：</strong>{" "}
                        {registrationResult.accountInfo.usage}
                      </div>
                    </div>
                  </div>
                )}
                {registrationResult.details &&
                  registrationResult.details.length > 0 && (
                    <div className="mt-3">
                      <h5 className="mb-1 text-sm font-medium text-slate-900 dark:text-slate-100">
                        详细信息：
                      </h5>
                      <ul className="space-y-1 text-sm list-disc list-inside text-slate-700 dark:text-slate-300">
                        {registrationResult.details.map((detail, index) => (
                          <li key={index}>{detail}</li>
                        ))}
                      </ul>
                    </div>
                  )}
              </div>
            )}
            {/* 显示实时Python脚本输出 */}
            {(isRegistering || realtimeOutput.length > 0) && (
              <div className="mt-3">
                <h5 className="mb-2 text-sm font-medium text-slate-900 dark:text-slate-100">
                  脚本执行日志：
                  {isRegistering && (
                    <span className="ml-2 text-xs text-blue-600">
                      (实时更新中...)
                    </span>
                  )}
                </h5>
                <div className="p-3 overflow-y-auto rounded-md panel-code max-h-64">
                  <div className="space-y-1 font-mono text-xs text-green-400">
                    {Array.from(new Set(realtimeOutput)).map((line, index) => (
                      <div key={index} className="whitespace-pre-wrap">
                        {line}
                      </div>
                    ))}
                    {isRegistering && realtimeOutput.length === 0 && (
                      <div className="text-yellow-400">等待脚本输出...</div>
                    )}
                  </div>
                </div>
              </div>
            )}
            {/* 显示错误输出 */}
            {/* {registrationResult.error_output && (
                  <div className="mt-3">
                    <h5 className="mb-2 text-sm font-medium text-red-700">
                      错误信息：
                    </h5>
                    <div className="p-3 overflow-y-auto border border-red-200 rounded-md bg-red-50 max-h-32">
                      <pre className="text-xs text-red-700 whitespace-pre-wrap">
                        {registrationResult.error_output}
                      </pre>
                    </div>
                  </div>
                )} */}
          </div>
        </div>
      </div>

      {/* Toast 通知 */}
      {toast && (
        <Toast
          message={toast.message}
          type={toast.type}
          onClose={() => setToast(null)}
        />
      )}

      {/* 验证码输入弹窗 */}
      {showVerificationModal && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black bg-opacity-50">
          <div className="max-w-md p-6 mx-4 rounded-lg panel-floating w-96">
            <h3 className="mb-4 text-lg font-medium text-slate-900 dark:text-slate-100">
              输入验证码
            </h3>
            {currentTaskEmail && (
              <div className="px-3 py-2 mb-3 rounded-md status-info">
                <p className="text-sm text-blue-800">
                  📧 任务邮箱:{" "}
                  <span className="font-mono font-medium">
                    {currentTaskEmail}
                  </span>
                </p>
                {currentTaskId && (
                  <p className="mt-1 text-xs text-blue-600">
                    🆔 任务ID:{" "}
                    <span className="font-mono">{currentTaskId}</span>
                  </p>
                )}
              </div>
            )}
            <p className="mb-4 text-sm text-subtle">
              请检查您的邮箱并输入6位验证码(请确认页面已经在输入验证码页面否则输入无效！)
            </p>
            <input
              type="text"
              value={verificationCode}
              onChange={(e) => {
                const value = e.target.value.replace(/\D/g, "").slice(0, 6);
                setVerificationCode(value);
              }}
              placeholder="请输入6位验证码"
              className="w-full text-lg tracking-widest text-center field-input"
              maxLength={6}
              autoFocus
            />
            <div className="flex justify-end mt-6 space-x-3">
              <button
                type="button"
                onClick={handleCancelRegistration}
                className="px-4 py-2 text-sm font-medium border rounded-md surface-secondary border-subtle text-slate-700 hover:bg-slate-200/80 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-gray-500 dark:text-slate-300 dark:hover:bg-slate-700/70"
              >
                取消注册
              </button>
              <button
                type="button"
                onClick={handleVerificationCodeSubmit}
                disabled={verificationCode.length !== 6}
                className="px-4 py-2 text-sm font-medium text-white bg-blue-600 border border-transparent rounded-md hover:bg-blue-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-blue-500 disabled:opacity-50 disabled:cursor-not-allowed"
              >
                提交
              </button>
            </div>
          </div>
        </div>
      )}

      {/* 邮箱配置模态框 */}
      {false && codexManualAction && !showVerificationModal && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black bg-opacity-50">
          <div className="panel-floating mx-4 w-[30rem] max-w-xl rounded-lg p-6">
            <h3 className="mb-4 text-lg font-medium text-slate-900 dark:text-slate-100">
              Codex 手动确认步骤
            </h3>
            <div className="px-3 py-2 mb-4 rounded-md status-info">
              {codexManualAction?.email && (
                <p className="text-sm text-blue-800">
                  📧 任务邮箱:{" "}
                  <span className="font-mono font-medium">
                    {codexManualAction?.email}
                  </span>
                </p>
              )}
              {codexManualAction?.task_id && (
                <p className="mt-1 text-xs text-blue-600">
                  🆔 任务ID:{" "}
                  <span className="font-mono">
                    {codexManualAction?.task_id}
                  </span>
                </p>
              )}
            </div>
            <p className="mb-4 text-sm text-subtle">
              {codexManualAction?.message ||
                "当前任务需要手动确认后再继续执行。"}
            </p>
            <div className="flex flex-wrap justify-end gap-3">
              {/* <button
                type="button"
                onClick={() => handleCodexManualStepAction("continue")}
                disabled={manualActionLoading}
                className="px-4 py-2 text-sm font-medium border rounded-md surface-secondary border-subtle text-slate-700 hover:bg-slate-200/80 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-gray-500 disabled:cursor-not-allowed disabled:opacity-50 dark:text-slate-300 dark:hover:bg-slate-700/70"
              >
                继续执行
              </button> */}
              <button
                type="button"
                onClick={handleCodexManualStepForceClose}
                disabled={manualActionLoading}
                className="px-4 py-2 text-sm font-medium text-white bg-red-600 border border-red-500 rounded-md hover:bg-red-700 focus:outline-none focus:ring-2 focus:ring-red-500 focus:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50"
              >
                强制关闭当前任务
              </button>
              <button
                type="button"
                onClick={() =>
                  handleCodexManualStepAction("manual_confirm_complete")
                }
                disabled={manualActionLoading}
                className="px-4 py-2 text-sm font-medium text-white border rounded-md border-emerald-500 bg-emerald-600 hover:bg-emerald-700 focus:outline-none focus:ring-2 focus:ring-emerald-500 focus:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50"
              >
                手动确认注册完成
              </button>
            </div>
          </div>
        </div>
      )}

      <EmailConfigModal
        isOpen={showEmailConfig}
        onClose={() => setShowEmailConfig(false)}
        onSave={handleEmailConfigSave}
      />

      {/* 银行卡配置模态框 */}
      {/* 浏览器配置模态框 */}
      {showBrowserConfig && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black bg-opacity-50">
          <div className="w-full max-w-2xl p-6 mx-4 rounded-lg panel-floating">
            <div className="flex items-center justify-between mb-4">
              <h2 className="text-xl font-bold">浏览器路径配置</h2>
              <button
                onClick={() => setShowBrowserConfig(false)}
                className="text-slate-500 hover:text-slate-700 dark:text-slate-400 dark:hover:text-slate-200"
              >
                ✕
              </button>
            </div>

            <div className="space-y-6">
              {/* 说明文字 */}
              <div className="p-4 rounded-lg status-info">
                <h3 className="mb-2 font-medium text-blue-800">
                  🔍 浏览器配置说明
                </h3>
                <p className="text-sm text-blue-700">
                  如果默认浏览器路径无法找到或需要使用特定浏览器，可以手动指定浏览器可执行文件路径。
                  <br />
                  例如:{" "}
                  <code className="px-1 text-blue-700 bg-blue-100 rounded dark:bg-blue-500/15 dark:text-blue-100">
                    C:\Program Files\Google\Chrome\Application\chrome.exe
                  </code>
                </p>
              </div>

              {/* 当前状态 */}
              <div className="p-4 rounded-lg surface-secondary">
                <h3 className="mb-2 font-medium text-slate-800 dark:text-slate-100">
                  📍 当前状态
                </h3>
                <div className="text-sm text-slate-600 dark:text-slate-300">
                  {currentBrowserPath ? (
                    <div>
                      <span className="font-medium">
                        已设置自定义浏览器路径:
                      </span>
                      <br />
                      <span className="px-1 font-mono text-xs rounded bg-slate-200 dark:bg-slate-800">
                        {currentBrowserPath}
                      </span>
                    </div>
                  ) : (
                    <span>未设置自定义浏览器路径，使用系统默认</span>
                  )}
                </div>
              </div>

              {/* 路径输入 */}
              <div className="space-y-3">
                <h3 className="font-medium text-slate-800 dark:text-slate-100">
                  📝 设置浏览器路径
                </h3>
                <div className="space-y-3">
                  <input
                    type="text"
                    value={customBrowserPath}
                    onChange={(e) => setCustomBrowserPath(e.target.value)}
                    placeholder="请输入浏览器可执行文件完整路径"
                    className="field-input"
                  />

                  <div className="flex flex-wrap gap-2">
                    <Button
                      variant="primary"
                      onClick={handleSetBrowserPath}
                      className="text-sm"
                    >
                      💾 保存路径
                    </Button>

                    <Button
                      variant="secondary"
                      onClick={handleSelectBrowserFile}
                      className="text-sm"
                    >
                      📁 选择文件
                    </Button>

                    <Button
                      variant="danger"
                      onClick={handleClearBrowserPath}
                      className="text-sm"
                    >
                      🗑️ 清除路径
                    </Button>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      )}

      <BankCardConfigModal
        isOpen={showBankCardConfig}
        onClose={() => setShowBankCardConfig(false)}
        onSave={handleBankCardConfigSave}
      />
    </div>
  );
};
