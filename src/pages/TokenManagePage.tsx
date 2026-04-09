import React, { useState, useEffect, useRef } from "react";
import { AccountService } from "../services/accountService";
import { CursorService } from "../services/cursorService";
import type { AccountInfo } from "../types/account";
import { LoadingSpinner } from "../components/LoadingSpinner";
import { Toast } from "../components/Toast";
import { ConfirmDialog } from "../components/ConfirmDialog";
import { UsageDisplay } from "../components/UsageDisplay";
import { AccountUsageModal } from "../components/AccountUsageModal";
import { UsageProgressModal } from "../components/UsageProgressModal";
import { UsageProgressDisplay } from "../components/UsageProgressDisplay";
import { AccountList } from "../components/AccountList";
import { WebServerConfig } from "../components/WebServerConfig";
import { useAccountStore } from "../stores/accountStore";
import { open } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  getClientAccessToken,
  fetchActualCurrentToken,
} from "../utils/tokenUtils";
import { scrollToTop, addTemporaryHighlight } from "../utils/domUtils";
import { flashWindow, showAutoLoginWindow } from "../utils/windowUtils";
import { getRemainingDays, isValidEmail } from "../utils/accountUtils";
import { formatDate } from "../utils/dateUtils";

export const TokenManagePage: React.FC = () => {
  // 使用store管理账户数据
  const {
    accountData,
    loading,
    setAccountData,
    setLoading,
    isCacheValid,
    switchCurrentAccount,
    removeAccountByEmail,
  } = useAccountStore();

  // 使用configStore管理Key验证状态
  // 分组相关状态 - 现在由AccountList组件内部管理
  const [showScrollToTop, setShowScrollToTop] = useState(false);

  // 订阅信息缓存相关状态
  const [subscriptionCache, setSubscriptionCache] = useState<{
    data: Map<string, any>; // 以邮箱为key的缓存数据
    timestamp: number; // 缓存时间戳
    isValid: boolean; // 缓存是否有效
  }>({
    data: new Map(),
    timestamp: 0,
    isValid: false,
  });

  const SUBSCRIPTION_CACHE_DURATION = 5 * 60 * 1000; // 5分钟缓存有效期
  const [reLoginAccount, setReLoginAccount] = useState<AccountInfo | null>(
    null
  ); // 正在重新登录的账户
  const [showReLoginModal, setShowReLoginModal] = useState(false); // 显示重新登录模态框
  const [_isUpdatingAccount, setIsUpdatingAccount] = useState(false); // 标记是否为更新账户模式
  const updateAccountOldTokenRef = useRef<string>(""); // 存储要更新账户的旧token，用于查找和更新
  const addAccountFormRef = useRef<HTMLDivElement>(null); // 添加账户表单的引用

  // 表单相关状态
  const [showAddForm, setShowAddForm] = useState(false);
  const [showQuickSwitchForm, setShowQuickSwitchForm] = useState(false);
  const [showEditForm, setShowEditForm] = useState(false);
  const [addAccountType, setAddAccountType] = useState<
    "token" | "email" | "verification_code"
  >("token"); // 新增：添加账户类型选择
  const [newEmail, setNewEmail] = useState("");
  const [newToken, setNewToken] = useState("");
  const [newPassword, setNewPassword] = useState(""); // 新增：密码字段
  const [newRefreshToken, setNewRefreshToken] = useState("");
  const [newWorkosSessionToken, setNewWorkosSessionToken] = useState("");
  const [autoLoginLoading, setAutoLoginLoading] = useState(false); // 新增：自动登录loading状态
  const [showLoginWindow, setShowLoginWindow] = useState(false); // 新增：是否显示登录窗口
  const [fetchingAccessToken, setFetchingAccessToken] = useState(false); // 获取AccessToken加载状态
  const [_autoLoginTimeout, setAutoLoginTimeout] = useState(false); // 新增：30秒超时状态
  const [showCancelLoginButton, setShowCancelLoginButton] = useState(false); // 新增：是否显示取消登录按钮
  const [openMenuEmail, setOpenMenuEmail] = useState<string | null>(null); // 新增：控制哪个账户的操作菜单打开
  const [usageModalOpen, setUsageModalOpen] = useState(false); // 用量modal状态
  const [usageProgressModalOpen, setUsageProgressModalOpen] = useState(false); // 使用进度modal状态
  const [selectedAccount, setSelectedAccount] = useState<AccountInfo | null>(
    null
  ); // 选中的账户
  const currentEmailRef = useRef<string>(""); // 用于在事件监听器中访问当前邮箱
  const autoLoginTimerRef = useRef<number | null>(null); // 新增：超时计时器引用
  const lastProcessedTokenRef = useRef<string>(""); // 防止重复处理同一个token
  const [editingAccount, setEditingAccount] = useState<AccountInfo | null>(
    null
  );
  const [editToken, setEditToken] = useState("");
  const [editRefreshToken, setEditRefreshToken] = useState("");
  const [editWorkosSessionToken, setEditWorkosSessionToken] = useState("");
  const [quickSwitchEmail, setQuickSwitchEmail] = useState("");
  const [quickSwitchToken, setQuickSwitchToken] = useState("");
  const [quickSwitchAuthType, setQuickSwitchAuthType] = useState("Auth_0");
  const [showManualVerificationModal, setShowManualVerificationModal] =
    useState(false);
  const [manualVerificationCode, setManualVerificationCode] = useState("");
  const [manualVerificationTaskId, setManualVerificationTaskId] = useState<
    string | null
  >(null);
  const [manualVerificationEmail, setManualVerificationEmail] = useState<
    string | null
  >(null);
  const [toast, setToast] = useState<{
    message: string;
    type: "success" | "error";
  } | null>(null);

  // 无感换号相关状态
  const [seamlessSwitchEnabled, setSeamlessSwitchEnabled] = useState<
    boolean | null
  >(null); // null表示未检查，true/false表示启用/禁用状态
  const [seamlessSwitchLoading, setSeamlessSwitchLoading] = useState(false);
  const [seamlessSwitchFullStatus, setSeamlessSwitchFullStatus] = useState<{
    workbench_modified: boolean;
    extension_host_modified: boolean;
    fully_enabled: boolean;
    need_reset_warning: boolean;
  } | null>(null);

  // Web服务器相关状态
  const [showWebServerConfig, setShowWebServerConfig] = useState(false);

  // 测试API相关状态

  // 订阅配置相关状态
  const [subscriptionTier, setSubscriptionTier] = useState<
    "pro" | "pro_plus" | "ultra"
  >("pro");
  const [allowAutomaticPayment, setAllowAutomaticPayment] = useState(true);
  const [allowTrial, setAllowTrial] = useState(true);
  const [confirmDialog, setConfirmDialog] = useState<{
    show: boolean;
    title: string;
    message: string;
    onConfirm: (checkboxValue?: boolean, autoCloseValue?: boolean) => void;
    checkboxLabel?: string;
    checkboxDefaultChecked?: boolean;
    checkboxDisabled?: boolean;
    autoCloseCheckboxLabel?: string;
    autoCloseCheckboxDefaultChecked?: boolean;
    autoCloseCheckboxDisabled?: boolean;
  }>({ show: false, title: "", message: "", onConfirm: () => {} });

  // 这些功能现在由AccountList组件内部处理
  // - 订阅信息缓存管理
  // - 刷新账户数据
  // - 获取当前token

  // 检查无感换号状态
  const checkSeamlessSwitchStatus = async () => {
    try {
      const result: any = await invoke("get_seamless_switch_full_status");
      if (result.success) {
        setSeamlessSwitchEnabled(result.workbench_modified);
        setSeamlessSwitchFullStatus({
          workbench_modified: result.workbench_modified,
          extension_host_modified: result.extension_host_modified,
          fully_enabled: result.fully_enabled,
          need_reset_warning: result.need_reset_warning,
        });
      } else {
        console.error("检查无感换号状态失败:", result.message);
        setSeamlessSwitchEnabled(false);
        setSeamlessSwitchFullStatus(null);
      }
    } catch (error) {
      console.error("检查无感换号状态失败:", error);
      setSeamlessSwitchEnabled(false);
      setSeamlessSwitchFullStatus(null);
    }
  };

  // 启用无感换号
  const handleEnableSeamlessSwitch = async () => {
    try {
      setSeamlessSwitchLoading(true);
      setToast({
        message: "正在启用无感换号功能...",
        type: "success",
      });

      const result: any = await invoke("enable_seamless_switch");
      if (result.success) {
        setToast({
          message: result.message,
          type: "success",
        });
        setSeamlessSwitchEnabled(true);
        await checkSeamlessSwitchStatus();
      } else {
        setToast({
          message: result.message,
          type: "error",
        });
      }
    } catch (error) {
      console.error("启用无感换号失败:", error);
      setToast({
        message: "启用无感换号功能失败",
        type: "error",
      });
    } finally {
      setSeamlessSwitchLoading(false);
    }
  };

  // 禁用无感换号（带确认对话框）
  const handleDisableSeamlessSwitch = () => {
    setConfirmDialog({
      show: true,
      title: "关闭无感换号",
      message:
        "确定要关闭无感换号功能吗？这将恢复原始的workbench文件并重启Cursor。",
      onConfirm: async () => {
        try {
          setSeamlessSwitchLoading(true);
          setToast({
            message: "正在关闭无感换号功能...",
            type: "success",
          });

          const result: any = await invoke("disable_seamless_switch");
          if (result.success) {
            setToast({
              message: result.message,
              type: "success",
            });
            setSeamlessSwitchEnabled(false);
            // 重新检查完整状态
            await checkSeamlessSwitchStatus();
          } else {
            setToast({
              message: result.message,
              type: "error",
            });
          }
        } catch (error) {
          console.error("关闭无感换号失败:", error);
          setToast({
            message: "关闭无感换号功能失败",
            type: "error",
          });
        } finally {
          setSeamlessSwitchLoading(false);
        }
        setConfirmDialog({ ...confirmDialog, show: false });
      },
    });
  };

  useEffect(() => {
    loadAccounts();

    // 检查无感换号状态
    checkSeamlessSwitchStatus();

    // 设置取消订阅事件监听器
    let cleanupListeners: (() => void) | null = null;

    const setupListeners = async () => {
      const { listen } = await import("@tauri-apps/api/event");

      const successUnlisten = await listen(
        "cancel-subscription-success",
        async () => {
          console.log("Cancel subscription success event received");
          // setCancelSubscriptionLoading (removed - handled by AccountList)(null);
          setToast({
            message: "取消订阅页面已打开，请继续完成操作",
            type: "success",
          });
          // 取消订阅操作可能会改变账户状态，刷新列表
          await loadAccounts(true);
        }
      );

      const failedUnlisten = await listen("cancel-subscription-failed", () => {
        console.log("Cancel subscription failed event received");
        // setCancelSubscriptionLoading (removed - handled by AccountList)(null);
        setToast({
          message: "未找到取消订阅按钮，请手动操作",
          type: "error",
        });
      });

      // 手动绑卡事件监听器
      const bindCardSuccessUnlisten = await listen(
        "manual-bind-card-success",
        () => {
          console.log("Manual bind card success event received");
          // setManualBindCardLoading (removed - handled by AccountList)(null);
          setToast({
            message: "手动绑卡页面已打开，请继续完成操作",
            type: "success",
          });
        }
      );

      const bindCardFailedUnlisten = await listen(
        "manual-bind-card-failed",
        () => {
          console.log("Manual bind card failed event received");
          // setManualBindCardLoading (removed - handled by AccountList)(null);
          setTimeout(() => {
            setToast({
              message: "未找到开始试用按钮，可能已经绑卡！",
              type: "error",
            });
          }, 1000);
        }
      );

      // 自动登录事件监听器
      const autoLoginSuccessUnlisten = await listen(
        "auto-login-success",
        async (event: any) => {
          console.log("Auto login success event received", event.payload);

          // 清除超时计时器
          if (autoLoginTimerRef.current) {
            window.clearTimeout(autoLoginTimerRef.current);
          }

          const webToken = event.payload?.token;
          if (webToken) {
            // 防止重复处理同一个token
            if (lastProcessedTokenRef.current === webToken) {
              console.log("⚠️ 检测到重复的token，忽略此次处理");
              return;
            }
            lastProcessedTokenRef.current = webToken;
            // 显示获取AccessToken的提示
            setToast({
              message: "WebToken获取成功！正在获取AccessToken...",
              type: "success",
            });

            try {
              // 获取AccessToken
              const accessTokenData = await getClientAccessToken(webToken);
              console.log("AccessToken data:", accessTokenData);

              if (accessTokenData && (accessTokenData as any).accessToken) {
                const accessToken = (accessTokenData as any).accessToken;
                const refreshToken =
                  (accessTokenData as any).refreshToken || accessToken;

                // 显示保存账户的提示
                setToast({
                  message: "AccessToken获取成功！正在保存账户信息...",
                  type: "success",
                });

                // 自动保存账户 - 使用ref中的邮箱
                const currentEmail = currentEmailRef.current; // 从ref获取当前邮箱
                console.log(currentEmail, "currentEmail");

                // 检查邮箱是否为空
                if (!currentEmail || currentEmail.trim() === "") {
                  console.error("❌ 自动登录：邮箱为空，无法添加账户");
                  setToast({
                    message: "邮箱为空，无法添加账户，请重新尝试",
                    type: "error",
                  });
                  setAutoLoginLoading(false);
                  setAutoLoginTimeout(false);
                  setShowCancelLoginButton(false);
                  return;
                }

                // 检查是否为更新账户模式 - 使用旧token来查找账户
                const oldToken = updateAccountOldTokenRef.current;
                const accountsData =
                  useAccountStore.getState().accountData?.accounts || [];
                const existingAccount = oldToken
                  ? accountsData.find((acc) => acc.token === oldToken)
                  : null;
                const isUpdateMode = !!existingAccount;
                console.log(
                  "isUpdateMode:",
                  isUpdateMode,
                  "oldToken:",
                  oldToken,
                  "existingAccount:",
                  existingAccount
                );

                const result =
                  isUpdateMode && existingAccount
                    ? await AccountService.editAccount(
                        existingAccount.email,
                        accessToken,
                        refreshToken,
                        webToken
                      )
                    : await AccountService.addAccount(
                        currentEmail,
                        accessToken,
                        refreshToken,
                        webToken
                      );

                if (result.success) {
                  // 使用自定义Toast替代系统对话框，避免macOS新系统的兼容性问题
                  showSuccessToastWithFlash(
                    isUpdateMode
                      ? `账户更新成功！${
                          existingAccount?.email || currentEmail
                        } 所有Token已自动获取并更新`
                      : `账户添加成功！${currentEmail} 所有Token已自动获取并保存`
                  );

                  // 清空表单并关闭
                  setNewEmail("");
                  setNewPassword("");
                  setNewToken("");
                  setNewRefreshToken("");
                  setNewWorkosSessionToken("");
                  currentEmailRef.current = ""; // 也清空ref
                  updateAccountOldTokenRef.current = ""; // 也清空旧token ref
                  lastProcessedTokenRef.current = ""; // 清空已处理token ref
                  setShowAddForm(false);
                  setAutoLoginLoading(false);
                  setAutoLoginTimeout(false);
                  setShowCancelLoginButton(false);
                  setShowLoginWindow(false);
                  setIsUpdatingAccount(false);
                  setReLoginAccount(null);

                  // 刷新账户列表
                  await loadAccounts(true);
                  await fetchActualCurrentToken();
                } else {
                  setToast({
                    message: isUpdateMode
                      ? `更新账户失败: ${result.message}`
                      : `保存账户失败: ${result.message}`,
                    type: "error",
                  });
                  lastProcessedTokenRef.current = ""; // 失败时也清空
                  setAutoLoginLoading(false);
                  setAutoLoginTimeout(false);
                  setShowCancelLoginButton(false);
                }
              } else {
                // 如果获取AccessToken失败，至少保存WebToken
                setNewWorkosSessionToken(webToken);
                setToast({
                  message: "获取AccessToken失败，但WebToken已填充，请手动添加",
                  type: "error",
                });
                lastProcessedTokenRef.current = ""; // 失败时也清空
                setAutoLoginLoading(false);
                setAutoLoginTimeout(false);
                setShowCancelLoginButton(false);
              }
            } catch (error) {
              console.error("获取AccessToken失败:", error);
              // 如果获取AccessToken失败，至少保存WebToken
              setNewWorkosSessionToken(webToken);
              setToast({
                message: "获取AccessToken失败，但WebToken已填充，请手动添加",
                type: "error",
              });
              lastProcessedTokenRef.current = ""; // 失败时也清空
              setAutoLoginLoading(false);
              setAutoLoginTimeout(false);
              setShowCancelLoginButton(false);
            }
          } else {
            lastProcessedTokenRef.current = ""; // 没有token时也清空
            setAutoLoginLoading(false);
            setAutoLoginTimeout(false);
            setShowCancelLoginButton(false);
          }
        }
      );

      const autoLoginFailedUnlisten = await listen(
        "auto-login-failed",
        (event: any) => {
          console.log("Auto login failed event received", event.payload);

          // 清除超时计时器
          if (autoLoginTimerRef.current) {
            window.clearTimeout(autoLoginTimerRef.current);
          }

          setAutoLoginLoading(false);
          setAutoLoginTimeout(false);
          setShowCancelLoginButton(false);
          setToast({
            message: `自动登录失败: ${event.payload?.error || "未知错误"}`,
            type: "error",
          });
        }
      );

      // 验证码登录事件监听器
      const verificationLoginSuccessUnlisten = await listen(
        "verification-login-cookie-found",
        async (event: any) => {
          console.log(
            "Verification login success event received",
            event.payload
          );

          // 清除超时计时器
          if (autoLoginTimerRef.current) {
            window.clearTimeout(autoLoginTimerRef.current);
          }

          const webToken = event.payload?.WorkosCursorSessionToken;
          if (webToken) {
            // 防止重复处理同一个token
            if (lastProcessedTokenRef.current === webToken) {
              console.log("⚠️ 检测到重复的token，忽略此次处理");
              return;
            }
            lastProcessedTokenRef.current = webToken;
            // 显示获取AccessToken的提示
            setToast({
              message:
                "验证码登录成功！WebToken获取成功！正在获取AccessToken...",
              type: "success",
            });

            try {
              // 获取AccessToken
              const accessTokenData = await getClientAccessToken(webToken);
              console.log("AccessToken data:", accessTokenData);

              if (accessTokenData && (accessTokenData as any).accessToken) {
                const accessToken = (accessTokenData as any).accessToken;
                const refreshToken =
                  (accessTokenData as any).refreshToken || accessToken;

                // 显示保存账户的提示
                setToast({
                  message: "AccessToken获取成功！正在保存账户信息...",
                  type: "success",
                });

                // 自动保存账户 - 使用ref中的邮箱
                const currentEmail = currentEmailRef.current; // 从ref获取当前邮箱
                console.log(currentEmail, "currentEmail");

                // 检查邮箱是否为空
                if (!currentEmail || currentEmail.trim() === "") {
                  console.error("❌ 验证码登录：邮箱为空，无法添加账户");
                  setToast({
                    message: "邮箱为空，无法添加账户，请重新尝试",
                    type: "error",
                  });
                  setAutoLoginLoading(false);
                  setAutoLoginTimeout(false);
                  setShowCancelLoginButton(false);
                  return;
                }

                // 检查是否为更新账户模式 - 使用旧token来查找账户
                const oldToken = updateAccountOldTokenRef.current;
                const accountsData =
                  useAccountStore.getState().accountData?.accounts || [];
                const existingAccount = oldToken
                  ? accountsData.find((acc) => acc.token === oldToken)
                  : null;
                const isUpdateMode = !!existingAccount;
                console.log(
                  "isUpdateMode:",
                  isUpdateMode,
                  "oldToken:",
                  oldToken,
                  "existingAccount:",
                  existingAccount
                );

                const result =
                  isUpdateMode && existingAccount
                    ? await AccountService.editAccount(
                        existingAccount.email,
                        accessToken,
                        refreshToken,
                        webToken
                      )
                    : await AccountService.addAccount(
                        currentEmail,
                        accessToken,
                        refreshToken,
                        webToken
                      );

                if (result.success) {
                  // 使用自定义Toast替代系统对话框，避免macOS新系统的兼容性问题
                  showSuccessToastWithFlash(
                    isUpdateMode
                      ? `账户更新成功！${
                          existingAccount?.email || currentEmail
                        } 所有Token已自动获取并更新`
                      : `账户添加成功！${currentEmail} 所有Token已自动获取并保存`
                  );

                  // 清空表单并关闭
                  setNewEmail("");
                  setNewPassword("");
                  setNewToken("");
                  setNewRefreshToken("");
                  setNewWorkosSessionToken("");
                  currentEmailRef.current = ""; // 也清空ref
                  updateAccountOldTokenRef.current = ""; // 也清空旧token ref
                  lastProcessedTokenRef.current = ""; // 清空已处理token ref
                  setShowAddForm(false);
                  setAutoLoginLoading(false);
                  setAutoLoginTimeout(false);
                  setShowCancelLoginButton(false);
                  setShowLoginWindow(false);
                  setIsUpdatingAccount(false);
                  setReLoginAccount(null);

                  // 刷新账户列表
                  await loadAccounts(true);
                  await fetchActualCurrentToken();
                } else {
                  setToast({
                    message: isUpdateMode
                      ? `更新账户失败: ${result.message}`
                      : `保存账户失败: ${result.message}`,
                    type: "error",
                  });
                  lastProcessedTokenRef.current = ""; // 失败时也清空
                  setAutoLoginLoading(false);
                  setAutoLoginTimeout(false);
                  setShowCancelLoginButton(false);
                }
              } else {
                // 如果获取AccessToken失败，至少保存WebToken
                setNewWorkosSessionToken(webToken);
                setToast({
                  message: "获取AccessToken失败，但WebToken已填充，请手动添加",
                  type: "error",
                });
                lastProcessedTokenRef.current = ""; // 失败时也清空
                setAutoLoginLoading(false);
                setAutoLoginTimeout(false);
                setShowCancelLoginButton(false);
              }
            } catch (error) {
              console.error("获取AccessToken失败:", error);
              // 如果获取AccessToken失败，至少保存WebToken
              setNewWorkosSessionToken(webToken);
              setToast({
                message: "获取AccessToken失败，但WebToken已填充，请手动添加",
                type: "error",
              });
              lastProcessedTokenRef.current = ""; // 失败时也清空
              setAutoLoginLoading(false);
              setAutoLoginTimeout(false);
              setShowCancelLoginButton(false);
            }
          } else {
            lastProcessedTokenRef.current = ""; // 没有token时也清空
            setAutoLoginLoading(false);
            setAutoLoginTimeout(false);
            setShowCancelLoginButton(false);
          }
        }
      );

      const manualVerificationInputRequiredUnlisten = await listen(
        "verification-code-manual-input-required",
        (event: any) => {
          const payload = event.payload ?? {};
          const taskId = payload.task_id || null;
          const email = payload.email || currentEmailRef.current || null;

          setManualVerificationTaskId(taskId);
          setManualVerificationEmail(email);
          setManualVerificationCode("");
          setShowManualVerificationModal(true);
          setToast({
            message: `自动获取验证码失败，请手动输入验证码${
              email ? `（${email}）` : ""
            }`,
            type: "error",
          });
        }
      );

      cleanupListeners = () => {
        successUnlisten();
        failedUnlisten();
        bindCardSuccessUnlisten();
        bindCardFailedUnlisten();
        autoLoginSuccessUnlisten();
        autoLoginFailedUnlisten();
        verificationLoginSuccessUnlisten();
        manualVerificationInputRequiredUnlisten();
      };
    };

    setupListeners();

    return () => {
      if (cleanupListeners) {
        cleanupListeners();
      }
    };
  }, []);

  // 监听点击外部关闭菜单
  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      const target = event.target as Element;
      if (openMenuEmail && !target.closest(".dropdown-menu")) {
        setOpenMenuEmail(null);
      }
    };

    document.addEventListener("mousedown", handleClickOutside);
    return () => {
      document.removeEventListener("mousedown", handleClickOutside);
    };
  }, [openMenuEmail]);

  // 监听滚动显示/隐藏置顶按钮
  useEffect(() => {
    const handleScroll = () => {
      const scrollTop =
        window.pageYOffset || document.documentElement.scrollTop;
      setShowScrollToTop(scrollTop > 300); // 滚动超过300px时显示按钮
    };

    window.addEventListener("scroll", handleScroll);
    return () => {
      window.removeEventListener("scroll", handleScroll);
    };
  }, []);

  // 处理获取AccessToken按钮点击
  const handleFetchAccessToken = async () => {
    if (!newWorkosSessionToken.trim()) {
      setToast({
        message: "请先输入 WorkOS Session Token",
        type: "error",
      });
      return;
    }

    setFetchingAccessToken(true);
    try {
      const result: any = await getClientAccessToken(
        newWorkosSessionToken.trim()
      );
      if (result && result.accessToken) {
        // 回显 AccessToken 和 RefreshToken
        setNewToken(result.accessToken);
        if (result.refreshToken) {
          setNewRefreshToken(result.refreshToken);
        }
        setToast({
          message: "AccessToken 获取成功！",
          type: "success",
        });
      } else {
        setToast({
          message:
            "获取 AccessToken 失败，请检查 WorkOS Session Token 是否正确",
          type: "error",
        });
      }
    } catch (error) {
      console.error("获取 AccessToken 失败:", error);
      setToast({
        message: "获取 AccessToken 时发生错误",
        type: "error",
      });
    } finally {
      setFetchingAccessToken(false);
    }
  };

  const handleSubmitManualVerificationCode = async () => {
    const code = manualVerificationCode.trim();
    if (!/^\d{6}$/.test(code)) {
      setToast({ message: "请输入6位验证码", type: "error" });
      return;
    }

    try {
      await invoke("submit_verification_code", {
        code,
        taskId: manualVerificationTaskId,
      });
      setShowManualVerificationModal(false);
      setManualVerificationCode("");
      setToast({ message: "验证码已提交，请稍候...", type: "success" });
    } catch (error) {
      setToast({
        message: `提交验证码失败: ${
          error instanceof Error ? error.message : "未知错误"
        }`,
        type: "error",
      });
    }
  };

  // 处理更新账户AccessToken
  const handleUpdateAccessToken = async (account: AccountInfo) => {
    if (!account.workos_cursor_session_token) {
      setToast({
        message: "该账户没有WorkOS Session Token，无法更新AccessToken",
        type: "error",
      });
      return;
    }

    // setUpdateAccessTokenLoading (removed - handled by AccountList)(account.email);
    try {
      setToast({
        message: "正在更新AccessToken，请稍候...",
        type: "success",
      });

      const result: any = await getClientAccessToken(
        account.workos_cursor_session_token
      );

      if (result && result.accessToken) {
        // 更新账户的token和refresh token
        const updateResult = await AccountService.editAccount(
          account.email,
          result.accessToken,
          result.refreshToken || account.refresh_token,
          account.workos_cursor_session_token
        );

        if (updateResult.success) {
          setToast({
            message: "AccessToken 更新成功！",
            type: "success",
          });

          // 只更新当前账户的数据，而不是刷新整个列表
          const currentData = useAccountStore.getState().accountData;
          if (currentData?.accounts) {
            const updatedAccounts = currentData.accounts.map((acc) =>
              acc.email === account.email
                ? {
                    ...acc,
                    token: result.accessToken,
                    refresh_token: result.refreshToken || acc.refresh_token,
                    // 重置授权状态，让系统重新获取订阅信息
                    auth_status: undefined,
                    subscription_type: undefined,
                    subscription_status: undefined,
                    trial_days_remaining: undefined,
                    auth_error: undefined,
                  }
                : acc
            );

            setAccountData({
              ...currentData,
              accounts: updatedAccounts,
            });

            // 异步获取更新后账户的详细信息
            setTimeout(async () => {
              try {
                const authResult = await CursorService.getUserInfo(
                  result.accessToken
                );
                const currentData = useAccountStore.getState().accountData;
                if (currentData?.accounts) {
                  const finalUpdatedAccounts = currentData.accounts.map((acc) =>
                    acc.email === account.email
                      ? {
                          ...acc,
                          ...(authResult.success &&
                          authResult.user_info?.account_info
                            ? {
                                subscription_type:
                                  authResult.user_info.account_info
                                    .subscription_type,
                                subscription_status:
                                  authResult.user_info.account_info
                                    .subscription_status,
                                trial_days_remaining:
                                  authResult.user_info.account_info
                                    .trial_days_remaining,
                                auth_status: "authorized" as const,
                              }
                            : {
                                subscription_type: "获取失败",
                                subscription_status: "error",
                                auth_status: "error" as const,
                                auth_error:
                                  authResult.message || "获取账户信息失败",
                              }),
                        }
                      : acc
                  );

                  setAccountData({
                    ...currentData,
                    accounts: finalUpdatedAccounts,
                  });
                }
              } catch (error) {
                console.error("获取更新后的账户信息失败:", error);
              }
            }, 100);
          }

          // 刷新账户列表以确保数据同步
          await loadAccounts(true);
          await fetchActualCurrentToken();
        } else {
          setToast({
            message: `更新失败: ${updateResult.message}`,
            type: "error",
          });
        }
      } else {
        setToast({
          message:
            "获取新的AccessToken失败，请检查WorkOS Session Token是否有效",
          type: "error",
        });
      }
    } catch (error) {
      console.error("更新AccessToken失败:", error);
      setToast({
        message: "更新AccessToken时发生错误",
        type: "error",
      });
    } finally {
      // setUpdateAccessTokenLoading (removed - handled by AccountList)(null);
    }
  };

  // 处理重新登录账户
  const handleReLoginAccount = (account: AccountInfo) => {
    setReLoginAccount(account);
    updateAccountOldTokenRef.current = account.token; // 保存旧token用于后续查找和更新
    setShowReLoginModal(true);
  };

  // 关闭重新登录模态框
  const handleCloseReLoginModal = () => {
    setReLoginAccount(null);
    setShowReLoginModal(false);
    setIsUpdatingAccount(false);
    updateAccountOldTokenRef.current = ""; // 清空旧token
  };

  // 滚动到添加账户表单
  const scrollToAddAccountForm = () => {
    setTimeout(() => {
      if (addAccountFormRef.current) {
        addAccountFormRef.current.scrollIntoView({
          behavior: "smooth",
          block: "center",
        });
        // 添加一个轻微的高亮效果
        addTemporaryHighlight(addAccountFormRef.current);
      }
    }, 100); // 等待表单显示后再滚动
  };

  // 显示成功Toast并闪烁提示
  const showSuccessToastWithFlash = (message: string) => {
    setToast({
      message,
      type: "success",
    });
    flashWindow();
  };

  // 缓存相关函数
  const getSubscriptionFromCache = (email: string) => {
    const now = Date.now();
    const cacheIsValid =
      subscriptionCache.isValid &&
      now - subscriptionCache.timestamp < SUBSCRIPTION_CACHE_DURATION;

    if (cacheIsValid && subscriptionCache.data.has(email)) {
      return subscriptionCache.data.get(email);
    }
    return null;
  };

  const updateSubscriptionCache = (email: string, data: any) => {
    const newCacheData = new Map(subscriptionCache.data);
    newCacheData.set(email, data);

    setSubscriptionCache({
      data: newCacheData,
      timestamp: Date.now(),
      isValid: true,
    });
  };

  // 批量操作相关函数 - 现在由AccountList组件内部处理
  // 这些函数已移至AccountList组件

  const loadAccounts = async (forceRefresh: boolean = false) => {
    try {
      // 如果不是强制刷新且缓存有效，则不重新加载
      if (!forceRefresh && isCacheValid() && accountData) {
        console.log("使用缓存的账户数据");
        return;
      }

      // 只在首次加载（无数据）时显示 loading，刷新时保持数据显示
      const isFirstLoad =
        !accountData ||
        !accountData.accounts ||
        accountData.accounts.length === 0;
      if (isFirstLoad) {
        setLoading(true);
      }

      const result = await AccountService.getAccountList();

      // 先显示基本账户列表
      if (result.success && result.accounts) {
        setAccountData(result);
        setLoading(false); // 立即取消loading状态，显示账户列表

        // 获取每个账户的详细信息（订阅类型、试用天数等） - 现在由AccountList组件管理
        if (true) {
          // 始终获取，由AccountList组件决定是否显示
          result.accounts.forEach(async (account, index) => {
            try {
              // 首先检查缓存
              const cachedData = getSubscriptionFromCache(account.email);
              let authResult;

              if (cachedData && !forceRefresh) {
                // 使用缓存数据
                console.log(`使用缓存的订阅信息: ${account.email}`);
                authResult = cachedData;
              } else {
                // 从API获取订阅信息
                console.log(`从API获取订阅信息: ${account.email}`);
                authResult = await CursorService.getUserInfo(account.token);

                // 将成功的结果缓存
                if (authResult.success) {
                  updateSubscriptionCache(account.email, authResult);
                }
              }

              // 更新单个账户的信息
              const currentData = useAccountStore.getState().accountData;
              if (!currentData?.accounts) return;

              const updatedAccounts = [...currentData.accounts];

              if (authResult.success && authResult.user_info?.account_info) {
                const subscriptionType =
                  authResult.user_info?.account_info?.subscription_type;

                // 成功获取账户信息
                updatedAccounts[index] = {
                  ...updatedAccounts[index],
                  subscription_type: subscriptionType,
                  subscription_status:
                    authResult.user_info?.account_info?.subscription_status,
                  trial_days_remaining:
                    authResult.user_info?.account_info?.trial_days_remaining,
                  auth_status: "authorized", // 标记为已授权
                };

                // 立即更新账户列表，显示订阅类型
                setAccountData({
                  ...currentData,
                  accounts: [...updatedAccounts],
                });

                // 判断是否需要获取使用进度（仅对试用账户、Pro、Pro Plus、Ultra获取）
                const shouldFetchUsage =
                  subscriptionType &&
                  (subscriptionType.toLowerCase().includes("free") ||
                    subscriptionType.toLowerCase().includes("trial") ||
                    subscriptionType.toLowerCase().includes("pro") ||
                    subscriptionType.toLowerCase().includes("ultra"));

                if (shouldFetchUsage) {
                  // 异步获取使用进度，不阻塞其他账户的处理
                  (async () => {
                    try {
                      console.log(`🔄 获取 ${account.email} 的使用进度...`);
                      const usageResult =
                        await CursorService.getCurrentPeriodUsage(
                          account.token
                        );

                      if (
                        usageResult.success &&
                        usageResult.parsed_data?.parsed
                      ) {
                        const parsed = usageResult.parsed_data.parsed;
                        const spendLimit = parsed.spendLimitUsage;
                        const progressPercentage =
                          parsed.usageProgressPercentage || 0;

                        if (spendLimit) {
                          let individualUsed = spendLimit.individualUsed || 0;
                          let individualLimit = spendLimit.individualLimit || 0;

                          // 修正限额：如果限额不合理，根据进度计算
                          if (
                            individualLimit <= individualUsed &&
                            progressPercentage > 0
                          ) {
                            individualLimit = Math.round(
                              individualUsed / (progressPercentage / 100)
                            );
                          }

                          // 再次获取最新的账户数据
                          const latestData =
                            useAccountStore.getState().accountData;
                          if (!latestData?.accounts) return;

                          const latestAccounts = [...latestData.accounts];
                          latestAccounts[index] = {
                            ...latestAccounts[index],
                            usage_progress: {
                              percentage: progressPercentage,
                              individualUsed,
                              individualLimit,
                              individualUsedDollars: individualUsed / 100,
                              individualLimitDollars: individualLimit / 100,
                              message:
                                parsed.usageProgressMessage ||
                                parsed.displayMessage,
                            },
                          };

                          setAccountData({
                            ...latestData,
                            accounts: latestAccounts,
                          });

                          console.log(
                            `✅ ${account.email} 使用进度: ${progressPercentage}%`
                          );
                        }
                      }
                    } catch (error) {
                      console.error(
                        `❌ 获取 ${account.email} 使用进度失败:`,
                        error
                      );
                    }
                  })();
                }
              } else {
                // 处理获取信息失败的情况
                const isUnauthorized =
                  !authResult.success &&
                  (authResult.user_info?.api_status === 401 ||
                    authResult.user_info?.is_authorized === false);

                updatedAccounts[index] = {
                  ...updatedAccounts[index],
                  subscription_type: isUnauthorized ? "未授权" : "获取失败",
                  subscription_status: isUnauthorized
                    ? "unauthorized"
                    : "error",
                  trial_days_remaining: undefined,
                  auth_status: isUnauthorized ? "unauthorized" : "error",
                  auth_error: authResult.message || "获取账户信息失败",
                };
              }

              setAccountData({
                ...currentData,
                accounts: updatedAccounts,
              });
            } catch (error) {
              console.error(`Failed to get info for ${account.email}:`, error);

              // 处理异常情况
              const currentData = useAccountStore.getState().accountData;
              if (!currentData?.accounts) return;

              const updatedAccounts = [...currentData.accounts];
              updatedAccounts[index] = {
                ...updatedAccounts[index],
                subscription_type: "网络错误",
                subscription_status: "network_error",
                trial_days_remaining: undefined,
                auth_status: "network_error",
                auth_error:
                  error instanceof Error ? error.message : "网络请求失败",
              };

              setAccountData({
                ...currentData,
                accounts: updatedAccounts,
              });
            }
          });
        } else {
          // 如果没有勾选获取订阅状态，直接设置为未获取状态
          const updatedAccounts = result.accounts.map((account) => ({
            ...account,
            subscription_type: "未获取",
            subscription_status: "not_fetched",
            trial_days_remaining: undefined,
            auth_status: "not_fetched" as const,
          }));

          setAccountData({
            ...result,
            accounts: updatedAccounts,
          });
        }
      } else {
        setAccountData(result);
      }
    } catch (error) {
      console.error("Failed to load accounts:", error);
      setToast({ message: "加载账户列表失败", type: "error" });
    } finally {
      setLoading(false);
    }
  };

  const handleAddAccount = async () => {
    if (!newEmail) {
      setToast({ message: "请填写邮箱地址", type: "error" });
      return;
    }

    if (!isValidEmail(newEmail)) {
      setToast({ message: "请输入有效的邮箱地址", type: "error" });
      return;
    }

    // 根据添加类型进行不同的验证
    if (addAccountType === "token") {
      if (!newToken) {
        setToast({ message: "请填写Token", type: "error" });
        return;
      }
    } else if (addAccountType === "email") {
      if (!newPassword) {
        setToast({ message: "请填写密码", type: "error" });
        return;
      }
      // 执行自动登录获取token
      await handleAutoLogin();
      return; // 自动登录完成后会自动填充token，用户可以再次点击添加
    } else if (addAccountType === "verification_code") {
      // 执行验证码登录获取token（会打开窗口让用户手动输入验证码）
      await handleVerificationCodeLogin();
      return; // 验证码登录完成后会自动填充token并保存账户
    }

    try {
      // 检查是否是更新现有账户
      const existingAccount = accountData?.accounts?.find(
        (acc) => acc.email === newEmail
      );

      if (existingAccount) {
        // 更新现有账户
        const result = await AccountService.editAccount(
          newEmail,
          newToken,
          newRefreshToken || undefined,
          newWorkosSessionToken || undefined
        );

        if (result.success) {
          setToast({ message: "账户更新成功", type: "success" });

          // 只更新当前账户的数据，而不是刷新整个列表
          const currentData = useAccountStore.getState().accountData;
          if (currentData?.accounts) {
            const updatedAccounts = currentData.accounts.map((acc) =>
              acc.email === newEmail
                ? {
                    ...acc,
                    token: newToken,
                    refresh_token: newRefreshToken || acc.refresh_token,
                    workos_cursor_session_token:
                      newWorkosSessionToken || acc.workos_cursor_session_token,
                    // 重置授权状态，让系统重新获取订阅信息
                    auth_status: undefined,
                    subscription_type: undefined,
                    subscription_status: undefined,
                    trial_days_remaining: undefined,
                    auth_error: undefined,
                  }
                : acc
            );

            setAccountData({
              ...currentData,
              accounts: updatedAccounts,
            });

            // 异步获取更新后账户的详细信息
            setTimeout(async () => {
              try {
                const authResult = await CursorService.getUserInfo(newToken);
                const currentData = useAccountStore.getState().accountData;
                if (currentData?.accounts) {
                  const finalUpdatedAccounts = currentData.accounts.map((acc) =>
                    acc.email === newEmail
                      ? {
                          ...acc,
                          ...(authResult.success &&
                          authResult.user_info?.account_info
                            ? {
                                subscription_type:
                                  authResult.user_info.account_info
                                    .subscription_type,
                                subscription_status:
                                  authResult.user_info.account_info
                                    .subscription_status,
                                trial_days_remaining:
                                  authResult.user_info.account_info
                                    .trial_days_remaining,
                                auth_status: "authorized" as const,
                              }
                            : {
                                subscription_type: "获取失败",
                                subscription_status: "error",
                                auth_status: "error" as const,
                                auth_error:
                                  authResult.message || "获取账户信息失败",
                              }),
                        }
                      : acc
                  );

                  setAccountData({
                    ...currentData,
                    accounts: finalUpdatedAccounts,
                  });
                }
              } catch (error) {
                console.error("获取更新后的账户信息失败:", error);
              }
            }, 100);
          }
        } else {
          setToast({ message: result.message, type: "error" });
        }
      } else {
        // 添加新账户
        const result = await AccountService.addAccount(
          newEmail,
          newToken,
          newRefreshToken || undefined,
          newWorkosSessionToken || undefined
        );

        if (result.success) {
          setToast({ message: "账户添加成功", type: "success" });
          await loadAccounts(true);
        } else {
          setToast({ message: result.message, type: "error" });
        }
      }

      // 清空表单并关闭
      setNewEmail("");
      setNewToken("");
      setNewPassword("");
      setNewRefreshToken("");
      setNewWorkosSessionToken("");
      setShowAddForm(false);
    } catch (error) {
      console.error("Failed to add account:", error);
      setToast({ message: "添加账户失败", type: "error" });
    }
  };


  const handleAutoLogin = async () => {
    if (!newEmail || !newPassword) {
      setToast({ message: "请填写邮箱和密码", type: "error" });
      return;
    }

    try {
      setAutoLoginLoading(true);
      setAutoLoginTimeout(false);
      setShowCancelLoginButton(false);
      setToast({
        message: "正在后台执行自动登录，请稍候...",
        type: "success",
      });

      // 启动30秒超时计时器
      if (autoLoginTimerRef.current) {
        window.clearTimeout(autoLoginTimerRef.current);
      }

      autoLoginTimerRef.current = window.setTimeout(() => {
        console.log("自动登录30秒超时");
        setAutoLoginTimeout(true);
        setShowCancelLoginButton(true);
        setToast({
          message:
            "自动登录超时（30秒），如需要可以点击取消登录或者显示窗口查看是否遇到了验证码或者人机验证",
          type: "error",
        });
        flashWindow();
      }, 30000); // 30秒

      // 调用Rust后端的自动登录函数
      const result = await invoke("auto_login_and_get_cookie", {
        email: newEmail,
        password: newPassword,
        showWindow: showLoginWindow,
      });

      console.log("Auto login result:", result);
    } catch (error) {
      console.error("Failed to start auto login:", error);
      // 清除计时器
      if (autoLoginTimerRef.current) {
        window.clearTimeout(autoLoginTimerRef.current);
      }
      setAutoLoginLoading(false);
      setAutoLoginTimeout(false);
      setShowCancelLoginButton(false);
      setToast({
        message: "启动自动登录失败",
        type: "error",
      });
    }
  };

  // 新增：验证码登录函数
  const handleVerificationCodeLogin = async () => {
    if (!newEmail) {
      setToast({ message: "请填写邮箱", type: "error" });
      return;
    }

    try {
      setAutoLoginLoading(true);
      setAutoLoginTimeout(false);
      setShowCancelLoginButton(false);
      setToast({
        message: "正在打开登录窗口，请在窗口中输入邮箱收到的验证码...",
        type: "success",
      });

      // 启动60秒超时计时器（给用户更多时间输入验证码）
      if (autoLoginTimerRef.current) {
        window.clearTimeout(autoLoginTimerRef.current);
      }

      autoLoginTimerRef.current = window.setTimeout(() => {
        console.log("验证码登录60秒超时");
        setAutoLoginTimeout(true);
        setShowCancelLoginButton(true);
        setToast({
          message:
            "验证码登录超时（60秒），请检查邮箱并输入验证码，如需要可以点击取消登录或者显示窗口查看登录状态",
          type: "error",
        });
        flashWindow();
      }, 60000); // 60秒

      // 调用Rust后端的验证码登录函数（验证码传空字符串，由JS脚本处理）
      const result = await CursorService.verificationCodeLogin(
        newEmail,
        "", // 验证码为空，由用户在窗口中手动输入或脚本自动获取
        true // 验证码登录必须显示窗口
      );

      console.log("Verification code login result:", result);
    } catch (error) {
      console.error("Failed to start verification code login:", error);
      // 清除计时器
      if (autoLoginTimerRef.current) {
        window.clearTimeout(autoLoginTimerRef.current);
      }
      setAutoLoginLoading(false);
      setAutoLoginTimeout(false);
      setShowCancelLoginButton(false);
      setToast({
        message: "启动验证码登录失败",
        type: "error",
      });
    }
  };

  // 新增：取消自动登录函数
  const handleCancelAutoLogin = async () => {
    setConfirmDialog({
      show: true,
      title: "取消自动登录",
      message: "确定要取消当前的自动登录操作吗？",
      onConfirm: async () => {
        try {
          // 清除计时器
          if (autoLoginTimerRef.current) {
            window.clearTimeout(autoLoginTimerRef.current);
          }

          // 调用后端取消自动登录
          await invoke("auto_login_failed", { error: "用户手动取消" });

          // 重置状态
          setAutoLoginLoading(false);
          setAutoLoginTimeout(false);
          setShowCancelLoginButton(false);

          // 默认勾选显示窗口选项
          setShowLoginWindow(true);

          setToast({
            message: "已取消自动登录，下次将显示登录窗口",
            type: "success",
          });
        } catch (error) {
          console.error("Failed to cancel auto login:", error);
          setToast({
            message: "取消登录失败",
            type: "error",
          });
        }
        setConfirmDialog({ ...confirmDialog, show: false });
      },
    });
  };

  // 新增：显示自动登录窗口函数
  const handleShowAutoLoginWindow = async () => {
    try {
      await showAutoLoginWindow();
      setToast({
        message: "自动登录窗口已显示",
        type: "success",
      });
    } catch (error) {
      console.error("Failed to show auto login window:", error);
      setToast({
        message:
          error instanceof Error
            ? error.message
            : "显示窗口失败，可能窗口已关闭",
        type: "error",
      });
    }
  };

  // 新增：查看Cursor主页函数
  const handleViewDashboard = async (account: AccountInfo) => {
    if (!account.workos_cursor_session_token) {
      setToast({
        message: "该账户没有WorkOS Session Token，无法查看主页",
        type: "error",
      });
      return;
    }

    try {
      const result = await invoke("open_cursor_dashboard", {
        workosCursorSessionToken: account.workos_cursor_session_token,
      });
      console.log("Dashboard result:", result);
      setToast({
        message: "Cursor主页已打开",
        type: "success",
      });
    } catch (error) {
      console.error("Failed to open dashboard:", error);
      setToast({
        message: "打开主页失败",
        type: "error",
      });
    }
  };

  // 查看用量函数
  const handleViewUsage = (account: AccountInfo) => {
    if (!account.token) {
      setToast({
        message: "该账户没有Token，无法查看用量",
        type: "error",
      });
      return;
    }

    setSelectedAccount(account);
    setUsageModalOpen(true);
  };

  const handleSwitchAccount = async (email: string) => {
    // 检查是否启用了无感换号
    const isSeamlessEnabled = seamlessSwitchEnabled === true;

    if (isSeamlessEnabled) {
      // 无感换号模式：支持重置机器码，但禁用自动重启
      setConfirmDialog({
        show: true,
        title: "无感切换账户",
        message: `确定要切换到账户 ${email} 吗？\n\n✨ 当前使用无感换号模式：\n• 无需重启Cursor，切换后立即生效\n• 自动更新配置文件供外部应用调用\n• 支持重置机器码，确保切换成功`,
        checkboxLabel:
          "同时重置机器码（推荐，确保账户切换成功，⚠️如果频繁切换账户请取消勾选，同一个账户，频繁不同ID会封号！⚠️）",
        checkboxDefaultChecked: true,
        checkboxDisabled: false, // 无感换号模式下启用重置机器码选项
        autoCloseCheckboxLabel: "自动重启Cursor（无感换号模式下无需重启）",
        autoCloseCheckboxDefaultChecked: true, // 默认不勾选
        autoCloseCheckboxDisabled: true, // 禁用自动重启选项
        onConfirm: async (shouldReset?: boolean) => {
          try {
            const shouldResetMachineId = shouldReset ?? true;
            console.log(
              "无感换号模式 - shouldResetMachineId:",
              shouldResetMachineId
            );

            if (shouldResetMachineId) {
              // 第一步：执行完全重置
              console.log("🔄 开始执行完全重置...");
              setToast({ message: "正在执行完全重置...", type: "success" });

              const resetResult = await CursorService.completeResetMachineIds();
              if (!resetResult.success) {
                setToast({
                  message: `重置失败: ${resetResult.message}`,
                  type: "error",
                });
                setConfirmDialog({ ...confirmDialog, show: false });
                return;
              }

              console.log("✅ 完全重置成功，开始无感切换...");
              setToast({
                message: "重置成功，正在进行无感切换...",
                type: "success",
              });
            } else {
              console.log("⏭️ 跳过重置机器码，直接进行无感切换...");
              setToast({ message: "正在进行无感切换...", type: "success" });
            }

            // 第二步：无感换号模式切换（auto_restart强制为false，传递reset_machine_id参数）
            const result = await AccountService.switchAccount(
              email,
              false,
              shouldResetMachineId
            );

            if (result.success) {
              const message = shouldResetMachineId
                ? "无感切换成功！机器码已重置，配置已更新，Cursor将自动检测新账户"
                : "无感切换成功！配置已更新，Cursor将自动检测新账户";
              setToast({
                message,
                type: "success",
              });
              // 延迟一下，等待Cursor配置文件完全写入
              await new Promise((resolve) => setTimeout(resolve, 300));
              // 精准更新：只更新当前账户标记
              switchCurrentAccount(email);
              await fetchActualCurrentToken();
            } else {
              setToast({ message: result.message, type: "error" });
            }
          } catch (error) {
            console.error("Failed to seamless switch account:", error);
            setToast({ message: "无感切换失败", type: "error" });
          }
          setConfirmDialog({ ...confirmDialog, show: false });
        },
      });
    } else {
      // 传统模式：完整的弹窗
      setConfirmDialog({
        show: true,
        title: "切换账户",
        message: `确定要切换到账户 ${email} 吗？`,
        checkboxLabel: "同时重置机器码（推荐，确保账户切换成功）",
        checkboxDefaultChecked: true,
        autoCloseCheckboxLabel: "自动重启Cursor",
        autoCloseCheckboxDefaultChecked: true,
        onConfirm: async (shouldReset?: boolean, shouldAutoClose?: boolean) => {
          try {
            const shouldResetMachineId = shouldReset ?? true;
            const shouldAutoRestartCursor = shouldAutoClose ?? true;
            console.log("shouldResetMachineId:", shouldResetMachineId);
            console.log("shouldAutoRestartCursor:", shouldAutoRestartCursor);
            if (shouldResetMachineId) {
              // 第一步：执行完全重置
              console.log("🔄 开始执行完全重置...");
              setToast({ message: "正在执行完全重置...", type: "success" });

              const resetResult = await CursorService.completeResetMachineIds();
              if (!resetResult.success) {
                setToast({
                  message: `重置失败: ${resetResult.message}`,
                  type: "error",
                });
                setConfirmDialog({ ...confirmDialog, show: false });
                return;
              }

              console.log("✅ 完全重置成功，开始切换账户...");
              setToast({
                message: "重置成功，正在切换账户...",
                type: "success",
              });
            } else {
              console.log("⏭️ 跳过重置机器码，直接切换账户...");
              setToast({ message: "正在切换账户...", type: "success" });
            }

            // 第二步：切换账户（传统模式下也传递reset_machine_id参数）
            const result = await AccountService.switchAccount(
              email,
              shouldAutoRestartCursor,
              shouldResetMachineId
            );
            if (result.success) {
              let message = "";
              if (shouldResetMachineId && shouldAutoRestartCursor) {
                message = "账户切换成功！机器码已重置，Cursor已自动重启。";
              } else if (shouldResetMachineId && !shouldAutoRestartCursor) {
                message =
                  "账户切换成功！机器码已重置，请手动重启Cursor查看效果。";
              } else if (!shouldResetMachineId && shouldAutoRestartCursor) {
                message = "账户切换成功（未重置机器码）！Cursor已自动重启。";
              } else {
                message =
                  "账户切换成功（未重置机器码，未自动重启）！请手动重启Cursor查看效果。";
              }
              setToast({
                message,
                type: "success",
              });
              // 延迟一下，等待Cursor配置文件完全写入
              await new Promise((resolve) => setTimeout(resolve, 300));
              // 精准更新：只更新当前账户标记
              switchCurrentAccount(email);
              await fetchActualCurrentToken();
            } else {
              setToast({ message: result.message, type: "error" });
            }
          } catch (error) {
            console.error("Failed to switch account:", error);
            setToast({ message: "切换账户失败", type: "error" });
          }
          setConfirmDialog({ ...confirmDialog, show: false });
        },
      });
    }
  };

  const handleQuickSwitch = async () => {
    if (!quickSwitchEmail || !quickSwitchToken) {
      setToast({ message: "请填写邮箱和Token", type: "error" });
      return;
    }

    if (!isValidEmail(quickSwitchEmail)) {
      setToast({ message: "请输入有效的邮箱地址", type: "error" });
      return;
    }

    setConfirmDialog({
      show: true,
      title: "快速切换账户",
      message: `确定要切换到账户 ${quickSwitchEmail} 吗？这将先执行完全重置，然后直接使用提供的Token登录。`,
      onConfirm: async () => {
        try {
          // 第一步：执行完全重置
          console.log("🔄 开始执行完全重置...");
          setToast({ message: "正在执行完全重置...", type: "success" });

          const resetResult = await CursorService.completeResetMachineIds();
          if (!resetResult.success) {
            setToast({
              message: `重置失败: ${resetResult.message}`,
              type: "error",
            });
            setConfirmDialog({ ...confirmDialog, show: false });
            return;
          }

          console.log("✅ 完全重置成功，开始快速切换账户...");
          setToast({ message: "重置成功，正在切换账户...", type: "success" });

          // 第二步：快速切换账户
          const result = await AccountService.switchAccountWithToken(
            quickSwitchEmail,
            quickSwitchToken,
            quickSwitchAuthType
          );
          if (result.success) {
            setToast({
              message: "账户切换成功！请重启Cursor查看效果。",
              type: "success",
            });
            setQuickSwitchEmail("");
            setQuickSwitchToken("");
            setShowQuickSwitchForm(false);
            // 延迟一下，等待Cursor配置文件完全写入
            await new Promise((resolve) => setTimeout(resolve, 300));
            // 精准更新：只更新当前账户标记
            switchCurrentAccount(quickSwitchEmail);
            await fetchActualCurrentToken();
          } else {
            setToast({ message: result.message, type: "error" });
          }
        } catch (error) {
          console.error("Failed to quick switch account:", error);
          setToast({ message: "快速切换失败", type: "error" });
        }
        setConfirmDialog({ ...confirmDialog, show: false });
      },
    });
  };

  const handleRemoveAccount = async (email: string) => {
    // 测试闪烁
    // showSuccessToastWithFlash(`账户添加成功！${1} 所有Token已自动获取并保存`);
    // return;
    setConfirmDialog({
      show: true,
      title: "删除账户",
      message: `确定要删除账户 ${email} 吗？此操作不可撤销。`,
      onConfirm: async () => {
        try {
          const result = await AccountService.removeAccount(email);
          if (result.success) {
            setToast({ message: "账户删除成功", type: "success" });
            // 精准更新：直接从前端移除账户
            removeAccountByEmail(email);
          } else {
            setToast({ message: result.message, type: "error" });
          }
        } catch (error) {
          console.error("Failed to remove account:", error);
          setToast({ message: "删除账户失败", type: "error" });
        }
        setConfirmDialog({ ...confirmDialog, show: false });
      },
    });
  };

  const handleLogout = async () => {
    setConfirmDialog({
      show: true,
      title: "退出登录",
      message:
        "确定要退出当前账户吗？这将清除所有认证信息，需要重新登录Cursor。",
      onConfirm: async () => {
        try {
          const result = await AccountService.logoutCurrentAccount();
          if (result.success) {
            setToast({
              message: "退出登录成功，请重启Cursor完成退出",
              type: "success",
            });
            await loadAccounts(true);
          } else {
            setToast({ message: result.message, type: "error" });
          }

          // Show detailed results if available
          if (result.details && result.details.length > 0) {
            console.log("Logout details:", result.details);
          }
        } catch (error) {
          console.error("Failed to logout:", error);
          setToast({ message: "退出登录失败", type: "error" });
        }
        setConfirmDialog({ ...confirmDialog, show: false });
      },
    });
  };

  const handleDeleteCursorAccount = async (account: AccountInfo) => {
    if (!account.workos_cursor_session_token) {
      setToast({
        message: "该账户没有 WorkOS Session Token，无法注销",
        type: "error",
      });
      return;
    }

    setConfirmDialog({
      show: true,
      title: "注销 Cursor 账户",
      message: `确定要注销账户 ${account.email} 吗？此操作将永久删除该 Cursor 账户，无法撤销！`,
      onConfirm: async () => {
        try {
          const result = await AccountService.deleteAccount(
            account.workos_cursor_session_token!
          );
          await AccountService.removeAccount(account.email);
          if (result.success) {
            setToast({
              message: "账户注销成功！",
              type: "success",
            });
            await loadAccounts(true);
          } else {
            setToast({ message: result.message, type: "error" });
          }
        } catch (error) {
          console.error("Failed to delete cursor account:", error);
          setToast({ message: "注销账户失败", type: "error" });
        }
        setConfirmDialog({ ...confirmDialog, show: false });
      },
    });
  };

  const handleCancelSubscription = async (account: AccountInfo) => {
    if (!account.workos_cursor_session_token) {
      setToast({
        message: "该账户没有 WorkOS Session Token，无法取消订阅",
        type: "error",
      });
      return;
    }

    try {
      // setCancelSubscriptionLoading (removed - handled by AccountList)(account.email);
      setToast({
        message: "正在打开取消订阅页面，请稍候...",
        type: "success",
      });

      const result = await AccountService.openCancelSubscriptionPage(
        account.workos_cursor_session_token
      );

      if (result.success) {
        // 不要关闭 toast，等待 Rust 端的事件响应
        // setToast 会在事件监听器中处理
      } else {
        // setCancelSubscriptionLoading (removed - handled by AccountList)(null);
        setToast({
          message: result.message,
          type: "error",
        });
      }
    } catch (error) {
      console.error("Failed to open cancel subscription page:", error);
      // setCancelSubscriptionLoading (removed - handled by AccountList)(null);
      setToast({
        message: "打开取消订阅页面失败",
        type: "error",
      });
    }
  };

  const handleManualBindCard = async (account: AccountInfo) => {
    if (!account.workos_cursor_session_token) {
      setToast({
        message: "该账户没有 WorkOS Session Token，无法进行手动绑卡",
        type: "error",
      });
      return;
    }

    try {
      // setManualBindCardLoading (removed - handled by AccountList)(account.email);
      setToast({
        message: "正在打开手动绑卡页面，请稍候...",
        type: "success",
      });

      const result = await AccountService.openManualBindCardPage(
        account.workos_cursor_session_token,
        subscriptionTier,
        allowAutomaticPayment,
        allowTrial
      );

      if (result.success) {
        // setManualBindCardLoading (removed - handled by AccountList)(null);
        setToast({
          message: "手动绑卡页面已打开",
          type: "success",
        });
        // 绑卡操作可能会改变账户状态，刷新列表
        await loadAccounts(true);
      } else {
        // setManualBindCardLoading (removed - handled by AccountList)(null);
        setToast({
          message: result.message,
          type: "error",
        });
      }
    } catch (error) {
      console.error("Failed to open manual bind card page:", error);
      // setManualBindCardLoading (removed - handled by AccountList)(null);
      setToast({
        message: "打开手动绑卡页面失败",
        type: "error",
      });
    }
  };

  const handleCopyBindCardUrl = async (account: AccountInfo) => {
    if (!account.workos_cursor_session_token) {
      setToast({
        message: "该账户没有 WorkOS Session Token，无法获取绑卡链接",
        type: "error",
      });
      return;
    }

    try {
      setToast({
        message: "正在获取绑卡链接，请稍候...",
        type: "success",
      });

      const result = await AccountService.getBindCardUrl(
        account.workos_cursor_session_token,
        subscriptionTier,
        allowAutomaticPayment,
        allowTrial
      );

      if (result.success) {
        // Rust 后端已经复制到剪贴板了
        setToast({
          message: result.message || "绑卡链接已复制到剪贴板",
          type: "success",
        });
      } else {
        setToast({
          message: result.message,
          type: "error",
        });
      }
    } catch (error) {
      console.error("Failed to get bind card URL:", error);
      setToast({
        message: "获取绑卡链接失败",
        type: "error",
      });
    }
  };

  const handleEditAccount = (account: AccountInfo) => {
    console.log("🔍 [DEBUG] handleEditAccount called with account:", account);

    setEditingAccount(account);
    setEditToken(account.token);
    setEditRefreshToken(account.refresh_token || "");
    setEditWorkosSessionToken(account.workos_cursor_session_token || "");
    setShowEditForm(true);
  };

  const handleSaveEdit = async () => {
    if (!editingAccount) return;
    console.log(
      "🔍 [DEBUG] handleSaveEdit called with editingAccount:",
      editingAccount
    );

    try {
      // Determine what to update
      const tokenChanged = editToken !== editingAccount.token;
      const refreshTokenChanged =
        editRefreshToken !== (editingAccount.refresh_token || "");
      const workosSessionTokenChanged =
        editWorkosSessionToken !==
        (editingAccount.workos_cursor_session_token || "");

      console.log("Edit save:", {
        email: editingAccount.email,
        tokenChanged,
        refreshTokenChanged,
        workosSessionTokenChanged,
        editToken: editToken.substring(0, 10) + "...",
        editRefreshToken: editRefreshToken.substring(0, 10) + "...",
        editWorkosSessionToken: editWorkosSessionToken.substring(0, 10) + "...",
        originalToken: editingAccount.token.substring(0, 10) + "...",
        originalRefreshToken:
          (editingAccount.refresh_token || "").substring(0, 10) + "...",
        originalWorkosSessionToken:
          (editingAccount.workos_cursor_session_token || "").substring(0, 10) +
          "...",
      });

      const result = await AccountService.editAccount(
        editingAccount.email,
        tokenChanged ? editToken : undefined,
        refreshTokenChanged ? editRefreshToken || undefined : undefined,
        workosSessionTokenChanged
          ? editWorkosSessionToken || undefined
          : undefined
      );

      if (result.success) {
        setToast({ message: "账户更新成功", type: "success" });
        setShowEditForm(false);
        setEditingAccount(null);
        setEditToken("");
        setEditRefreshToken("");
        setEditWorkosSessionToken("");
        await loadAccounts(true);
      } else {
        setToast({ message: result.message, type: "error" });
      }
    } catch (error) {
      console.error("Failed to edit account:", error);
      setToast({ message: "更新账户失败", type: "error" });
    }
  };

  const handleCancelEdit = () => {
    setShowEditForm(false);
    setEditingAccount(null);
    setEditToken("");
    setEditRefreshToken("");
    setEditWorkosSessionToken("");
  };

  const handleExportAccounts = async () => {
    try {
      // 使用Tauri 2的dialog插件选择导出目录
      const selectedPath = await open({
        multiple: false,
        directory: true,
        title: "选择导出目录",
      });

      if (!selectedPath) {
        return; // 用户取消选择
      }

      const result = await AccountService.exportAccounts(selectedPath);
      if (result.success) {
        setToast({
          message: `账户导出成功！文件保存在：${result.exported_path}`,
          type: "success",
        });
      } else {
        setToast({ message: result.message, type: "error" });
      }
    } catch (error) {
      console.error("Failed to export accounts:", error);
      setToast({ message: "导出账户失败", type: "error" });
    }
  };

  const handleImportAccounts = async () => {
    setConfirmDialog({
      show: true,
      title: "导入账户",
      message:
        "导入将会覆盖当前的账户文件，原文件将备份为account_back.json。确定要继续吗？",
      onConfirm: async () => {
        try {
          // 使用Tauri 2的dialog插件选择要导入的文件
          const selectedFile = await open({
            multiple: false,
            directory: false,
            filters: [
              {
                name: "JSON Files",
                extensions: ["json"],
              },
            ],
            title: "选择要导入的account.json文件",
          });

          if (!selectedFile) {
            setConfirmDialog({ ...confirmDialog, show: false });
            return; // 用户取消选择
          }

          const result = await AccountService.importAccounts(selectedFile);
          if (result.success) {
            setToast({
              message: result.message,
              type: "success",
            });
            // 重新加载账户列表
            await loadAccounts(true);
          } else {
            setToast({ message: result.message, type: "error" });
          }
        } catch (error) {
          console.error("Failed to import accounts:", error);
          setToast({ message: "导入账户失败", type: "error" });
        }
        setConfirmDialog({ ...confirmDialog, show: false });
      },
    });
  };

  // 测试GetCurrentPeriodUsage API
  // const handleTestUsageAPI = async () => {
  //   try {
  //     const tokenInfo = await CursorService.getTokenAuto();
  //     if (!tokenInfo.token) {
  //       setToast({ message: "未找到Token", type: "error" });
  //       return;
  //     }

  //     console.log("🧪 开始测试 GetCurrentPeriodUsage API...");
  //     await CursorService.getCurrentPeriodUsage(tokenInfo.token);
  //     console.log("✅ API 测试完成，结果已输出到控制台");
  //     setToast({ message: "API测试成功！结果已输出到控制台", type: "success" });
  //   } catch (error: any) {
  //     console.error("❌ API 测试失败:", error);
  //     setToast({ message: `API测试失败: ${error}`, type: "error" });
  //   }
  // };

  // 监听自动轮换成功事件
  useEffect(() => {
    const unlisten = listen("auto-switch-success", (event) => {
      console.log("收到自动轮换成功事件:", event.payload);

      // 显示成功提示
      setToast({
        message: "🎉 自动轮换账户成功！",
        type: "success",
      });

      // 刷新账户列表
      loadAccounts(true);
    });

    // 清理函数
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  if (loading) {
    return (
      <div className="flex items-center justify-center h-64">
        <LoadingSpinner />
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <div className="surface-primary rounded-lg shadow">
        <div className="px-4 py-5 sm:p-6">
          <h3 className="mb-4 text-lg font-medium leading-6 text-slate-900 dark:text-slate-100">
            🔐 Token 管理
          </h3>

          {/* Current Account Section */}
          {accountData?.current_account && (
            <div className="surface-accent mb-6 rounded-lg border p-4">
              <div className="flex items-center justify-between">
                <h4 className="mb-2 text-md font-medium text-blue-900 dark:text-blue-100">
                  📧 当前账户
                </h4>
                <button
                  type="button"
                  onClick={handleLogout}
                  className="inline-flex items-center px-3 py-1 text-xs font-medium text-red-700 bg-red-100 border border-transparent rounded hover:bg-red-200 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-red-500"
                >
                  🚪 退出登录
                </button>
              </div>
              <div className="text-sm text-blue-800 dark:text-blue-200">
                <p>
                  <strong>邮箱:</strong> {accountData.current_account.email}
                </p>
                <p>
                  <strong>剩余天数:</strong>{" "}
                  {getRemainingDays(accountData.current_account)}
                </p>
              </div>
            </div>
          )}

          {/* Usage Display Section */}
          {accountData?.current_account && (
            <div className="mb-6">
              {/* 有 WebToken: 显示用量统计 */}
              {accountData.current_account.workos_cursor_session_token && (
                <UsageDisplay
                  token={accountData.current_account.token}
                  className="mb-4"
                  showProgressButton={true}
                  onShowProgress={() => setUsageProgressModalOpen(true)}
                />
              )}

              {/* 没有 WebToken: 直接显示使用进度信息 */}
              {!accountData.current_account.workos_cursor_session_token && (
                <UsageProgressDisplay
                  token={accountData.current_account.token}
                  onShowToast={(message, type) => setToast({ message, type })}
                />
              )}
            </div>
          )}

          {/* 订阅配置 */}
          <div className="surface-accent mb-6 rounded-lg border p-4">
            <h4 className="mb-3 font-medium text-slate-900 dark:text-slate-100 text-md">
              ⚙️ 订阅配置
            </h4>
            <div className="grid grid-cols-1 gap-4 md:grid-cols-3">
              {/* 订阅层级 */}
              <div>
                <label className="block mb-2 text-sm font-medium text-slate-700 dark:text-slate-300">
                  订阅层级
                </label>
                <select
                  title="订阅层级"
                  value={subscriptionTier}
                  onChange={(e) =>
                    setSubscriptionTier(
                      e.target.value as "pro" | "pro_plus" | "ultra"
                    )
                  }
                  className="field-input"
                >
                  <option value="pro">Pro 试用</option>
                  <option value="pro_plus">Pro Plus 试用</option>
                  <option value="ultra">Ultra 版本</option>
                </select>
              </div>

              {/* 自动续费 */}
              <div>
                <label className="block mb-2 text-sm font-medium text-slate-700 dark:text-slate-300">
                  自动续费
                </label>
                <div className="flex items-center space-x-4">
                  <label className="flex items-center">
                    <input
                      type="radio"
                      name="allowAutomaticPayment"
                      checked={allowAutomaticPayment === true}
                      onChange={() => setAllowAutomaticPayment(true)}
                      className="mr-2 text-blue-600"
                    />
                    <span className="text-sm">开启</span>
                  </label>
                  <label className="flex items-center">
                    <input
                      type="radio"
                      name="allowAutomaticPayment"
                      checked={allowAutomaticPayment === false}
                      onChange={() => setAllowAutomaticPayment(false)}
                      className="mr-2 text-blue-600"
                    />
                    <span className="text-sm">关闭</span>
                  </label>
                </div>
              </div>

              {/* 试用开关 */}
              <div>
                <label className="block mb-2 text-sm font-medium text-slate-700 dark:text-slate-300">
                  开启试用
                </label>
                <div className="flex items-center space-x-4">
                  <label className="flex items-center">
                    <input
                      type="radio"
                      name="allowTrial"
                      checked={allowTrial === true}
                      onChange={() => setAllowTrial(true)}
                      className="mr-2 text-blue-600"
                    />
                    <span className="text-sm">开启</span>
                  </label>
                  <label className="flex items-center">
                    <input
                      type="radio"
                      name="allowTrial"
                      checked={allowTrial === false}
                      onChange={() => setAllowTrial(false)}
                      className="mr-2 text-blue-600"
                    />
                    <span className="text-sm">关闭</span>
                  </label>
                </div>
              </div>
            </div>
          </div>

          {/* Action Buttons */}
          <div className="flex flex-wrap gap-3 mb-4">
            <button
              type="button"
              onClick={() => setShowAddForm(!showAddForm)}
              className="inline-flex items-center px-4 py-2 text-sm font-medium text-white bg-blue-600 border border-transparent rounded-md shadow-sm hover:bg-blue-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-blue-500"
            >
              ➕ 添加账户
            </button>
            <button
              type="button"
              onClick={() => setShowQuickSwitchForm(!showQuickSwitchForm)}
              className="inline-flex items-center px-4 py-2 text-sm font-medium text-white bg-green-600 border border-transparent rounded-md shadow-sm hover:bg-green-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-green-500"
            >
              🚀 快速切换
            </button>
            <button
              type="button"
              onClick={handleExportAccounts}
              className="inline-flex items-center px-4 py-2 text-sm font-medium text-white bg-purple-600 border border-transparent rounded-md shadow-sm hover:bg-purple-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-purple-500"
            >
              📤 导出账户
            </button>
            <button
              type="button"
              onClick={handleImportAccounts}
              className="inline-flex items-center px-4 py-2 text-sm font-medium text-white bg-orange-600 border border-transparent rounded-md shadow-sm hover:bg-orange-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-orange-500"
            >
              📥 导入账户
            </button>

            {/* <button
              type="button"
              onClick={handleTestUsageAPI}
              className="inline-flex items-center px-4 py-2 text-sm font-medium text-white border border-transparent rounded-md shadow-sm bg-cyan-600 hover:bg-cyan-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-cyan-500"
            >
              🧪 测试Usage API
            </button> */}

            {/* 无感换号功能按钮 */}
            {seamlessSwitchEnabled === null ? (
              // 检查状态中
              <button
                type="button"
                disabled={true}
                className="inline-flex cursor-not-allowed items-center rounded-md border border-transparent bg-slate-200 px-4 py-2 text-sm font-medium text-slate-400 shadow-sm dark:bg-slate-800 dark:text-slate-500"
              >
                🔄 检查中...
              </button>
            ) : seamlessSwitchEnabled ? (
              // 已启用，显示关闭按钮
              <button
                type="button"
                onClick={handleDisableSeamlessSwitch}
                disabled={seamlessSwitchLoading}
                className={`inline-flex items-center px-4 py-2 text-sm font-medium text-white border border-transparent rounded-md shadow-sm focus:outline-none focus:ring-2 focus:ring-offset-2 ${
                  seamlessSwitchLoading
                    ? "bg-slate-400 cursor-not-allowed dark:bg-slate-700"
                    : "bg-red-600 hover:bg-red-700 focus:ring-red-500"
                }`}
              >
                {seamlessSwitchLoading
                  ? "🔄 处理中..."
                  : "🔴 关闭无感换号+无感重置ID"}
              </button>
            ) : (
              // 未启用，显示开启按钮
              <button
                type="button"
                onClick={handleEnableSeamlessSwitch}
                disabled={seamlessSwitchLoading}
                className={`inline-flex items-center px-4 py-2 text-sm font-medium text-white border border-transparent rounded-md shadow-sm focus:outline-none focus:ring-2 focus:ring-offset-2 ${
                  seamlessSwitchLoading
                    ? "bg-slate-400 cursor-not-allowed dark:bg-slate-700"
                    : "bg-indigo-600 hover:bg-indigo-700 focus:ring-indigo-500"
                }`}
              >
                {seamlessSwitchLoading
                  ? "🔄 处理中..."
                  : "✨ 开启无感换号+无感重置ID"}
              </button>
            )}

            {/* Web服务器配置按钮 */}
            <button
              type="button"
              onClick={() => setShowWebServerConfig(!showWebServerConfig)}
              className="inline-flex items-center px-4 py-2 text-sm font-medium text-white bg-teal-600 border border-transparent rounded-md shadow-sm hover:bg-teal-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-teal-500"
            >
              🌐 自动换号设置
            </button>
          </div>

          {/* 无感重置ID警告提示 - 已开启但未完全应用 */}
          {seamlessSwitchFullStatus?.need_reset_warning && (
            <div className="status-warning mb-4 rounded-lg p-3">
              <div className="flex items-start">
                <div className="flex-shrink-0">
                  <span className="text-yellow-600">⚠️</span>
                </div>
                <div className="ml-2">
                  <h4 className="text-sm font-medium text-yellow-800">
                    您当前无感换号没有应用无感重置ID
                  </h4>
                  <p className="mt-1 text-xs text-yellow-700">
                    强烈建议先<strong>关闭无感换号</strong>
                    再重新开启，以完整应用无感重置ID功能。
                    （无感重置ID是修改请求头重置ID，如果有重置id失败的情况下也建议开启尝试）
                  </p>
                </div>
              </div>
            </div>
          )}

          {/* 关闭状态提示 - 建议开启 */}
          {seamlessSwitchEnabled === false &&
            seamlessSwitchFullStatus &&
            !seamlessSwitchFullStatus.workbench_modified && (
              <div className="status-success mb-4 rounded-lg p-3">
                <div className="flex items-start">
                  <div className="flex-shrink-0">
                    <span className="text-green-600">💡</span>
                  </div>
                  <div className="ml-2">
                    <h4 className="text-sm font-medium text-green-800">
                      如果重置ID失败，建议尝试开启无感换号+无感重置ID
                    </h4>
                    <p className="mt-1 text-xs text-green-700">
                      无感重置ID功能会直接修改请求头，可以解决常规重置ID失败的问题。点击上方按钮即可开启。
                    </p>
                  </div>
                </div>
              </div>
            )}

          {/* 无感换号功能说明 */}
          {seamlessSwitchEnabled !== null && (
            <div className="status-info mb-4 rounded-lg p-3">
              <div className="flex items-start">
                <div className="flex-shrink-0">
                  {seamlessSwitchEnabled ? (
                    <span className="text-green-500">✅</span>
                  ) : (
                    <span className="text-slate-400 dark:text-slate-500">💡</span>
                  )}
                </div>
                <div className="ml-2">
                  <h4 className="text-sm font-medium text-blue-800">
                    {seamlessSwitchEnabled
                      ? "无感换号已启用"
                      : "关于无感换号功能"}
                  </h4>
                  <p className="mt-1 text-xs text-blue-600">
                    {seamlessSwitchEnabled
                      ? "已修改Cursor内核文件，现在切换账号时无需手动重启Cursor即可生效。"
                      : "启用后将修改Cursor内核文件，使账号切换更加丝滑。修改前会自动备份原始文件，可随时恢复。"}
                  </p>
                </div>
              </div>
            </div>
          )}

          {/* Web服务器配置 */}
          <WebServerConfig
            isOpen={showWebServerConfig}
            isSeamlessEnabled={seamlessSwitchEnabled === true}
            onShowToast={(message, type) => setToast({ message, type })}
          />

          {/* Add Account Form */}
          {showAddForm && (
            <div
              ref={addAccountFormRef}
              className="p-4 mb-6 border rounded-lg bg-slate-50 dark:bg-slate-800/70"
            >
              <h4 className="mb-3 font-medium text-slate-900 dark:text-slate-100 text-md">
                {accountData?.accounts?.find((acc) => acc.email === newEmail)
                  ? "更新账户"
                  : "添加新账户"}
              </h4>

              {/* 添加类型选择 */}
              <div className="mb-4">
                <label className="block mb-2 text-sm font-medium text-slate-700 dark:text-slate-300">
                  添加方式
                </label>
                <div className="flex flex-col space-y-2">
                  <label className="flex items-center">
                    <input
                      type="radio"
                      name="addAccountType"
                      value="token"
                      checked={addAccountType === "token"}
                      onChange={(e) =>
                        setAddAccountType(
                          e.target.value as
                            | "token"
                            | "email"
                            | "verification_code"
                        )
                      }
                      className="mr-2"
                    />
                    <span className="text-sm text-slate-700 dark:text-slate-300">🔑 使用Token</span>
                  </label>
                  <label className="flex items-center">
                    <input
                      type="radio"
                      name="addAccountType"
                      value="email"
                      checked={addAccountType === "email"}
                      onChange={(e) =>
                        setAddAccountType(
                          e.target.value as
                            | "token"
                            | "email"
                            | "verification_code"
                        )
                      }
                      className="mr-2"
                    />
                    <span className="text-sm text-slate-700 dark:text-slate-300">
                      📧 使用邮箱密码{" "}
                      <span className="text-xs text-slate-500 dark:text-slate-400">
                        （ip需要纯净最好是直连或者干净的代理不然容易失败）
                      </span>
                    </span>
                  </label>
                  <label className="flex items-center">
                    <input
                      type="radio"
                      name="addAccountType"
                      value="verification_code"
                      checked={addAccountType === "verification_code"}
                      onChange={(e) =>
                        setAddAccountType(
                          e.target.value as
                            | "token"
                            | "email"
                            | "verification_code"
                        )
                      }
                      className="mr-2"
                    />
                    <span className="text-sm text-slate-700 dark:text-slate-300">
                      📱 使用验证码{" "}
                      <span className="text-xs text-slate-500 dark:text-slate-400">
                        （需要手动从邮箱获取验证码）
                      </span>
                    </span>
                  </label>
                </div>
              </div>

              <div className="space-y-3">
                <div>
                  <label className="block text-sm font-medium text-slate-700 dark:text-slate-300">
                    邮箱地址
                  </label>
                  <input
                    type="email"
                    value={newEmail}
                    onChange={(e) => {
                      setNewEmail(e.target.value);
                      currentEmailRef.current = e.target.value; // 同时更新ref
                    }}
                    disabled={accountData?.accounts?.some(
                      (acc) => acc.email === newEmail
                    )}
                    className="field-input mt-1 sm:text-sm disabled:bg-slate-100 disabled:cursor-not-allowed dark:disabled:bg-slate-800"
                    placeholder="请输入邮箱地址"
                  />
                </div>
                {/* 根据添加类型显示不同的输入框 */}
                {addAccountType === "token" ? (
                  <div>
                    <label className="block text-sm font-medium text-slate-700 dark:text-slate-300">
                      Token
                    </label>
                    <textarea
                      value={newToken}
                      onChange={(e) => setNewToken(e.target.value)}
                      rows={3}
                      className="field-input mt-1 sm:text-sm"
                      placeholder="请输入Token"
                    />
                  </div>
                ) : addAccountType === "email" ? (
                  <div>
                    <label className="block text-sm font-medium text-slate-700 dark:text-slate-300">
                      密码
                    </label>
                    <input
                      type="password"
                      value={newPassword}
                      onChange={(e) => setNewPassword(e.target.value)}
                      className="field-input mt-1 sm:text-sm"
                      placeholder="请输入密码"
                    />
                    <p className="mt-1 text-xs text-slate-500 dark:text-slate-400">
                      将自动登录获取所有Token并保存账户：
                      <br />
                      1. 获取 WorkOS Session Token
                      <br />
                      2. 获取 Access Token 和 Refresh Token
                      <br />
                      3. 自动保存完整账户信息
                    </p>

                    {/* 显示窗口选项 */}
                    <div className="mt-3">
                      <label className="flex items-center">
                        <input
                          type="checkbox"
                          checked={showLoginWindow}
                          onChange={(e) => setShowLoginWindow(e.target.checked)}
                          className="mr-2 rounded border-slate-300 text-blue-600 focus:ring-blue-500 dark:border-slate-600 dark:bg-slate-900"
                        />
                        <span className="text-xs text-slate-600 dark:text-slate-300">
                          显示登录窗口 (如果获取失败可勾选此项查看原因)
                        </span>
                      </label>
                    </div>
                  </div>
                ) : (
                  <div>
                    <div className="status-info mb-3 rounded-md p-3">
                      <p className="text-sm text-blue-800">
                        <strong>📱 验证码登录流程：</strong>
                        <br />
                        1. 点击"验证码登录并添加"按钮
                        <br />
                        2. 系统会打开登录窗口并自动填写邮箱
                        <br />
                        3. Cursor会发送验证码到您的邮箱
                        <br />
                        4. 在打开的窗口中输入邮箱收到的验证码
                        <br />
                        5. 登录成功后自动获取所有Token并保存账户
                      </p>
                    </div>
                  </div>
                )}
                {/* 只在Token模式下显示这些输入框 */}
                {addAccountType === "token" && (
                  <>
                    <div>
                      <label className="block text-sm font-medium text-slate-700 dark:text-slate-300">
                        Refresh Token (可选)
                      </label>
                      <textarea
                        value={newRefreshToken}
                        onChange={(e) => setNewRefreshToken(e.target.value)}
                        rows={3}
                        className="field-input mt-1 sm:text-sm"
                        placeholder="请输入Refresh Token (可选)"
                      />
                    </div>
                    <div>
                      <div className="flex items-center justify-between mb-2">
                        <label className="block text-sm font-medium text-slate-700 dark:text-slate-300">
                          WorkOS Session Token (可选)
                        </label>
                        <button
                          type="button"
                          onClick={handleFetchAccessToken}
                          disabled={
                            !newWorkosSessionToken.trim() || fetchingAccessToken
                          }
                          className={`inline-flex items-center px-3 py-1.5 text-xs font-medium leading-4 text-white border border-transparent rounded-md focus:outline-none focus:ring-2 focus:ring-offset-2 ${
                            !newWorkosSessionToken.trim() || fetchingAccessToken
                              ? "bg-slate-400 cursor-not-allowed dark:bg-slate-700"
                              : "bg-blue-600 hover:bg-blue-700 focus:ring-blue-500"
                          }`}
                        >
                          {fetchingAccessToken ? (
                            <>🔄 获取中...</>
                          ) : (
                            <>🔑 获取 AccessToken</>
                          )}
                        </button>
                      </div>
                      <textarea
                        value={newWorkosSessionToken}
                        onChange={(e) =>
                          setNewWorkosSessionToken(e.target.value)
                        }
                        rows={3}
                        className="field-input mt-1 sm:text-sm"
                        placeholder="请输入WorkOS Session Token (可选，用于获取账号用量)"
                      />
                      {newWorkosSessionToken.trim() && (
                        <p className="mt-1 text-xs text-blue-600 dark:text-blue-300">
                          💡 点击右上角按钮可自动获取 AccessToken 和
                          RefreshToken
                        </p>
                      )}
                    </div>
                  </>
                )}
                <div className="flex space-x-3">
                  <button
                    type="button"
                    onClick={handleAddAccount}
                    disabled={autoLoginLoading}
                    className={`inline-flex items-center px-3 py-2 text-sm font-medium leading-4 text-white border border-transparent rounded-md focus:outline-none focus:ring-2 focus:ring-offset-2 ${
                      autoLoginLoading
                        ? "bg-slate-400 cursor-not-allowed dark:bg-slate-700"
                        : "bg-green-600 hover:bg-green-700 focus:ring-green-500"
                    }`}
                  >
                    {autoLoginLoading ? (
                      <>
                        🔄{" "}
                        {addAccountType === "email"
                          ? "自动登录获取中..."
                          : addAccountType === "verification_code"
                          ? "验证码登录中..."
                          : "处理中..."}
                      </>
                    ) : (
                      <>
                        ✅{" "}
                        {(() => {
                          const isUpdate = accountData?.accounts?.find(
                            (acc) => acc.email === newEmail
                          );
                          if (addAccountType === "email") {
                            return isUpdate
                              ? "自动登录并更新"
                              : "自动登录并添加";
                          } else if (addAccountType === "verification_code") {
                            return isUpdate
                              ? "验证码登录并更新"
                              : "验证码登录并添加";
                          } else {
                            return isUpdate ? "更新" : "添加";
                          }
                        })()}
                      </>
                    )}
                  </button>

                  {/* 超时后显示的取消登录按钮 */}
                  {showCancelLoginButton &&
                    (addAccountType === "email" ||
                      addAccountType === "verification_code") && (
                      <>
                        <button
                          type="button"
                          onClick={handleCancelAutoLogin}
                          className="inline-flex items-center px-3 py-2 text-sm font-medium leading-4 text-white bg-red-600 border border-transparent rounded-md hover:bg-red-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-red-500"
                        >
                          🛑 取消登录
                        </button>
                        <button
                          type="button"
                          onClick={handleShowAutoLoginWindow}
                          className="inline-flex items-center px-3 py-2 text-sm font-medium leading-4 text-white bg-blue-600 border border-transparent rounded-md hover:bg-blue-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-blue-500"
                        >
                          👁️ 显示窗口
                        </button>
                      </>
                    )}

                  <button
                    type="button"
                    onClick={() => {
                      // 清除定时器
                      if (autoLoginTimerRef.current) {
                        window.clearTimeout(autoLoginTimerRef.current);
                      }

                      setShowAddForm(false);
                      setNewEmail("");
                      setNewToken("");
                      setNewPassword("");
                      setNewRefreshToken("");
                      setNewWorkosSessionToken("");
                      currentEmailRef.current = ""; // 也清空ref
                      updateAccountOldTokenRef.current = ""; // 也清空旧token ref
                      setAddAccountType("token");
                      setShowLoginWindow(false);
                      setIsUpdatingAccount(false);
                      setReLoginAccount(null);
                      // 重置自动登录相关状态
                      setAutoLoginLoading(false);
                      setAutoLoginTimeout(false);
                      setShowCancelLoginButton(false);
                    }}
                    className="surface-elevated inline-flex items-center rounded-md border border-slate-300 px-3 py-2 text-sm font-medium leading-4 text-slate-700 hover:bg-slate-50 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-blue-500 dark:border-slate-700 dark:text-slate-300 dark:hover:bg-slate-800/70"
                  >
                    ❌ 取消
                  </button>
                </div>
              </div>
            </div>
          )}

          {/* Quick Switch Form */}
          {showQuickSwitchForm && (
            <div className="status-success mb-6 rounded-lg p-4">
              <h4 className="mb-3 font-medium text-slate-900 dark:text-slate-100 text-md">
                🚀 快速切换账户
              </h4>
              <p className="text-subtle mb-3 text-sm">
                直接输入邮箱和Token进行账户切换，无需先添加到账户列表
              </p>
              <div className="space-y-3">
                <div>
                  <label className="block text-sm font-medium text-slate-700 dark:text-slate-300">
                    邮箱地址
                  </label>
                  <input
                    type="email"
                    value={quickSwitchEmail}
                    onChange={(e) => setQuickSwitchEmail(e.target.value)}
                    placeholder="your-email@example.com"
                    className="field-input mt-1 sm:text-sm"
                  />
                </div>
                <div>
                  <label className="block text-sm font-medium text-slate-700 dark:text-slate-300">
                    Access Token
                  </label>
                  <textarea
                    value={quickSwitchToken}
                    onChange={(e) => setQuickSwitchToken(e.target.value)}
                    placeholder="eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9..."
                    rows={3}
                    className="field-input mt-1 sm:text-sm"
                  />
                </div>
                <div>
                  <label
                    htmlFor="auth-type-select"
                    className="block text-sm font-medium text-slate-700 dark:text-slate-300"
                  >
                    认证类型
                  </label>
                  <select
                    id="auth-type-select"
                    value={quickSwitchAuthType}
                    onChange={(e) => setQuickSwitchAuthType(e.target.value)}
                    className="field-input mt-1 sm:text-sm"
                  >
                    <option value="Auth_0">Auth_0 (默认)</option>
                    <option value="Google">Google</option>
                    <option value="GitHub">GitHub</option>
                  </select>
                </div>
                <div className="flex space-x-3">
                  <button
                    type="button"
                    onClick={handleQuickSwitch}
                    className="inline-flex items-center px-3 py-2 text-sm font-medium leading-4 text-white bg-green-600 border border-transparent rounded-md hover:bg-green-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-green-500"
                  >
                    🚀 立即切换
                  </button>
                  <button
                    type="button"
                    onClick={() => {
                      setShowQuickSwitchForm(false);
                      setQuickSwitchEmail("");
                      setQuickSwitchToken("");
                      setQuickSwitchAuthType("Auth_0");
                    }}
                    className="surface-elevated inline-flex items-center rounded-md border border-slate-300 px-3 py-2 text-sm font-medium leading-4 text-slate-700 hover:bg-slate-50 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-blue-500 dark:border-slate-700 dark:text-slate-300 dark:hover:bg-slate-800/70"
                  >
                    ❌ 取消
                  </button>
                </div>
              </div>
            </div>
          )}

          {/* Account List */}
          <AccountList
            onSwitchAccount={handleSwitchAccount}
            onRemoveAccount={handleRemoveAccount}
            onEditAccount={handleEditAccount}
            onViewUsage={handleViewUsage}
            onUpdateAccessToken={handleUpdateAccessToken}
            onReLoginAccount={handleReLoginAccount}
            onViewDashboard={handleViewDashboard}
            onManualBindCard={handleManualBindCard}
            onCopyBindCardUrl={handleCopyBindCardUrl}
            onCancelSubscription={handleCancelSubscription}
            onDeleteCursorAccount={handleDeleteCursorAccount}
            formatDate={formatDate}
          />
        </div>
      </div>

      {/* Edit Account Modal */}
      {showEditForm && editingAccount && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black bg-opacity-50">
          <div className="panel-floating w-full max-w-md rounded-lg p-6">
            <div className="flex items-center justify-between mb-4">
              <h3 className="text-lg font-medium text-slate-900 dark:text-slate-100">编辑账户</h3>
            </div>
            <div className="p-3 mb-4 rounded-lg bg-slate-50 dark:bg-slate-800/70">
              <div className="flex items-center justify-between">
                <div>
                  <label className="block mb-1 text-sm font-medium text-slate-700 dark:text-slate-300">
                    邮箱地址
                  </label>
                  <p className="font-mono text-sm text-slate-900 dark:text-slate-100 break-all">
                    {editingAccount.email}
                  </p>
                </div>
                <button
                  type="button"
                  onClick={async () => {
                    try {
                      await navigator.clipboard.writeText(editingAccount.email);
                      setToast({
                        message: "邮箱地址已复制到剪贴板",
                        type: "success",
                      });
                    } catch (error) {
                      setToast({
                        message: "复制失败，请手动复制",
                        type: "error",
                      });
                    }
                  }}
                  className="inline-flex items-center rounded border border-transparent bg-blue-100 px-2 py-1 text-xs font-medium text-blue-700 hover:bg-blue-200 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-blue-500 dark:bg-blue-500/15 dark:text-blue-200 dark:hover:bg-blue-500/25"
                >
                  📋 复制
                </button>
              </div>
            </div>
            <div className="space-y-4">
              <div>
                <label className="block text-sm font-medium text-slate-700 dark:text-slate-300">
                  Token
                </label>
                <textarea
                  value={editToken}
                  onChange={(e) => setEditToken(e.target.value)}
                  rows={3}
                  className="field-input mt-1 sm:text-sm"
                  placeholder="请输入Token"
                />
              </div>
              <div>
                <label className="block text-sm font-medium text-slate-700 dark:text-slate-300">
                  Refresh Token (可选)
                </label>
                <textarea
                  value={editRefreshToken}
                  onChange={(e) => setEditRefreshToken(e.target.value)}
                  rows={3}
                  className="field-input mt-1 sm:text-sm"
                  placeholder="请输入Refresh Token (可选)"
                />
              </div>
              <div>
                <label className="block text-sm font-medium text-slate-700 dark:text-slate-300">
                  WorkOS Session Token (可选)
                </label>
                <textarea
                  value={editWorkosSessionToken}
                  onChange={(e) => setEditWorkosSessionToken(e.target.value)}
                  rows={3}
                  className="field-input mt-1 sm:text-sm"
                  placeholder="请输入WorkOS Session Token (可选，用于注销账户)"
                />
              </div>
              <div className="flex justify-end space-x-3">
                <button
                  type="button"
                  onClick={handleCancelEdit}
                  className="surface-elevated inline-flex items-center rounded-md border border-slate-300 px-4 py-2 text-sm font-medium text-slate-700 hover:bg-slate-50 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-blue-500 dark:border-slate-700 dark:text-slate-300 dark:hover:bg-slate-800/70"
                >
                  取消
                </button>
                <button
                  type="button"
                  onClick={handleSaveEdit}
                  className="inline-flex items-center px-4 py-2 text-sm font-medium text-white bg-blue-600 border border-transparent rounded-md hover:bg-blue-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-blue-500"
                >
                  保存
                </button>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* Toast */}
      {toast && (
        <Toast
          message={toast.message}
          type={toast.type}
          onClose={() => setToast(null)}
        />
      )}

      {/* 用量查看Modal */}
      <AccountUsageModal
        isOpen={usageModalOpen}
        onClose={() => {
          setUsageModalOpen(false);
          setSelectedAccount(null);
        }}
        account={selectedAccount}
        onShowToast={(message, type) => setToast({ message, type })}
      />

      {/* 使用进度弹窗 */}
      {accountData?.current_account && (
        <UsageProgressModal
          isOpen={usageProgressModalOpen}
          onClose={() => setUsageProgressModalOpen(false)}
          token={accountData.current_account.token}
          onShowToast={(message, type) => setToast({ message, type })}
        />
      )}

      {/* Confirm Dialog */}
      {/* Re-Login Modal */}
      {showReLoginModal && reLoginAccount && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black bg-opacity-50">
          <div className="panel-floating w-full max-w-md rounded-lg p-6">
            <div className="flex items-center justify-between mb-4">
              <h3 className="text-lg font-medium text-slate-900 dark:text-slate-100">
                重新登录账户
              </h3>
              <button
                onClick={handleCloseReLoginModal}
                className="text-slate-400 hover:text-slate-600 dark:text-slate-500 dark:hover:text-slate-300"
              >
                ✕
              </button>
            </div>

            <div className="p-3 mb-4 rounded-lg bg-slate-50 dark:bg-slate-800/70">
              <p className="mb-1 text-sm text-slate-700 dark:text-slate-300">账户邮箱</p>
              <p className="font-mono text-sm text-slate-900 dark:text-slate-100">
                {reLoginAccount.email}
              </p>
            </div>

            <div className="space-y-3">
              <p className="text-subtle mb-4 text-sm">
                该账户的Token已失效，请选择重新登录方式：
              </p>

              <button
                onClick={() => {
                  setNewEmail(reLoginAccount.email);
                  setAddAccountType("email");
                  setIsUpdatingAccount(true);
                  setShowAddForm(true);
                  setShowReLoginModal(false);
                  scrollToAddAccountForm();
                }}
                className="surface-accent flex w-full items-center justify-center rounded-lg border px-4 py-3 text-sm font-medium text-blue-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-blue-500 dark:text-blue-200"
              >
                <span className="mr-2">📧</span>
                使用邮箱密码更新
              </button>

              <button
                onClick={() => {
                  setNewEmail(reLoginAccount.email);
                  setAddAccountType("verification_code");
                  setIsUpdatingAccount(true);
                  setShowAddForm(true);
                  setShowReLoginModal(false);
                  scrollToAddAccountForm();
                }}
                className="status-success flex w-full items-center justify-center rounded-lg px-4 py-3 text-sm font-medium focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-green-500"
              >
                <span className="mr-2">📱</span>
                使用验证码更新
              </button>
            </div>

            <div className="flex justify-end mt-6">
              <button
                onClick={handleCloseReLoginModal}
                className="surface-secondary border-subtle rounded-md border px-4 py-2 text-sm font-medium text-slate-700 hover:bg-slate-200/80 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-gray-500 dark:text-slate-300 dark:hover:bg-slate-700/70"
              >
                取消
              </button>
            </div>
          </div>
        </div>
      )}

      {showManualVerificationModal && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black bg-opacity-50">
          <div className="panel-floating w-full max-w-md rounded-lg p-6">
            <div className="mb-4 flex items-center justify-between">
              <h3 className="text-lg font-medium text-slate-900 dark:text-slate-100">
                手动输入验证码
              </h3>
              <button
                onClick={() => setShowManualVerificationModal(false)}
                className="text-slate-400 hover:text-slate-600 dark:text-slate-500 dark:hover:text-slate-300"
              >
                ✕
              </button>
            </div>
            <p className="mb-2 text-sm text-slate-600 dark:text-slate-300">
              自动获取验证码失败，请输入邮箱验证码继续。
            </p>
            {manualVerificationEmail && (
              <p className="mb-3 text-xs text-slate-500 dark:text-slate-400">
                邮箱：{manualVerificationEmail}
              </p>
            )}
            <input
              type="text"
              value={manualVerificationCode}
              onChange={(e) =>
                setManualVerificationCode(e.target.value.replace(/\D/g, "").slice(0, 6))
              }
              className="field-input mb-4 text-center text-lg tracking-[0.35em]"
              placeholder="请输入6位验证码"
            />
            <div className="flex justify-end gap-2">
              <button
                onClick={() => setShowManualVerificationModal(false)}
                className="surface-secondary border-subtle rounded-md border px-4 py-2 text-sm font-medium text-slate-700 hover:bg-slate-200/80 dark:text-slate-300 dark:hover:bg-slate-700/70"
              >
                取消
              </button>
              <button
                onClick={handleSubmitManualVerificationCode}
                className="rounded-md bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700"
              >
                提交验证码
              </button>
            </div>
          </div>
        </div>
      )}

      {confirmDialog.show && (
        <ConfirmDialog
          isOpen={confirmDialog.show}
          title={confirmDialog.title}
          message={confirmDialog.message}
          onConfirm={confirmDialog.onConfirm}
          onCancel={() => setConfirmDialog({ ...confirmDialog, show: false })}
          checkboxLabel={confirmDialog.checkboxLabel}
          checkboxDefaultChecked={confirmDialog.checkboxDefaultChecked}
          checkboxDisabled={confirmDialog.checkboxDisabled}
          autoCloseCheckboxLabel={confirmDialog.autoCloseCheckboxLabel}
          autoCloseCheckboxDefaultChecked={
            confirmDialog.autoCloseCheckboxDefaultChecked
          }
          autoCloseCheckboxDisabled={confirmDialog.autoCloseCheckboxDisabled}
        />
      )}

      {/* 置顶按钮 */}
      {showScrollToTop && (
        <button
          onClick={() => scrollToTop()}
          className="fixed z-40 flex items-center justify-center w-12 h-12 text-white transition-all duration-300 transform bg-blue-600 rounded-full shadow-lg bottom-6 right-6 hover:bg-blue-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-blue-500 hover:scale-110"
          title="回到顶部"
          aria-label="回到顶部"
        >
          <svg
            className="w-6 h-6"
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24"
            xmlns="http://www.w3.org/2000/svg"
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={2}
              d="M5 10l7-7m0 0l7 7m-7-7v18"
            />
          </svg>
        </button>
      )}
    </div>
  );
};
