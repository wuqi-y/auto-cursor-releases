import React, { useState, useEffect } from "react";
import { createPortal } from "react-dom";
import { AccountInfo } from "../types/account";
import { AccountCard } from "./AccountCard";
import { useAccountStore } from "../stores/accountStore";
import {
  useConfigStore,
  type SortOrder,
  type SortField,
} from "../stores/configStore";
import { AccountService } from "../services/accountService";
import {
  CursorService,
  type CodexTokenFileInfo,
} from "../services/cursorService";
import { useConfirmDialog } from "./ConfirmDialog";
import { invoke } from "@tauri-apps/api/core";

interface AccountListProps {
  // 功能函数 - 这些需要父组件提供因为涉及到全局状态
  onSwitchAccount: (email: string) => void;
  onRemoveAccount: (email: string) => void;
  onEditAccount: (account: AccountInfo) => void;
  onViewUsage: (account: AccountInfo) => void;
  onUpdateAccessToken: (account: AccountInfo) => void;
  onReLoginAccount: (account: AccountInfo) => void;
  onViewDashboard: (account: AccountInfo) => void;
  onManualBindCard: (account: AccountInfo) => void;
  onCopyBindCardUrl: (account: AccountInfo) => void;
  onCancelSubscription: (account: AccountInfo) => void;
  onDeleteCursorAccount: (account: AccountInfo) => void;
  formatDate: (dateString: string) => string;
}

export const AccountList: React.FC<AccountListProps> = ({
  onSwitchAccount,
  onRemoveAccount,
  onEditAccount,
  onViewUsage,
  onUpdateAccessToken,
  onReLoginAccount,
  onViewDashboard,
  onManualBindCard,
  onCopyBindCardUrl,
  onCancelSubscription,
  onDeleteCursorAccount,
  formatDate,
}) => {
  // 使用store管理账户数据
  const {
    accountData,
    setAccountData,
    setLoading,
    isCacheValid,
    getGroupedAccounts,
    getAvailableGroups,
    getFreeAccountSubgroups,
    lastUpdated,
  } = useAccountStore();

  // 使用configStore管理排序状态
  const {
    getAccountSortOrder,
    toggleAccountSortOrder,
    getAccountSortField,
    toggleAccountSortField,
  } = useConfigStore();

  // 内部状态管理
  const [activeTab, setActiveTab] = useState<string>("all");
  const [sortOrder, setSortOrder] = useState<SortOrder>(() => {
    // 从 store 中读取初始排序状态，默认降序（最新的在前）
    return getAccountSortOrder() || "desc";
  });
  const [sortField, setSortField] = useState<SortField>(() => {
    // 从 store 中读取初始排序字段，默认按创建时间
    return getAccountSortField() || "created_at";
  });
  const [activeFreeSubgroup, setActiveFreeSubgroup] = useState<string>("all");
  const [refreshing, setRefreshing] = useState(false);
  const [accountTypeView, setAccountTypeView] = useState<"cursor" | "codex">(
    "cursor"
  );
  const [codexTokenFiles, setCodexTokenFiles] = useState<CodexTokenFileInfo[]>(
    []
  );
  const [codexLoading, setCodexLoading] = useState(false);
  const [searchQuery, setSearchQuery] = useState<string>("");
  const [selectedAccounts, setSelectedAccounts] = useState<Set<string>>(
    new Set()
  );
  const [showBatchActions, setShowBatchActions] = useState(false);
  const [includeEmailInExport, setIncludeEmailInExport] = useState(false);
  const [includeWorkosTokenInExport, setIncludeWorkosTokenInExport] =
    useState(true); // 默认导出 workos token
  const [fetchSubscriptionInfo, setFetchSubscriptionInfo] = useState(() => {
    const saved = localStorage.getItem("fetchSubscriptionInfo");
    return saved !== null ? JSON.parse(saved) : true;
  });
  const [subscriptionCache, setSubscriptionCache] = useState<{
    data: Map<string, any>;
    timestamp: number;
    isValid: boolean;
  }>({
    data: new Map(),
    timestamp: 0,
    isValid: false,
  });
  const [actualCurrentToken, setActualCurrentToken] = useState<string | null>(
    null
  );
  const [updateAccessTokenLoading, _setUpdateAccessTokenLoading] = useState<
    string | null
  >(null);
  const [manualBindCardLoading, _setManualBindCardLoading] = useState<
    string | null
  >(null);
  const [cancelSubscriptionLoading, _setCancelSubscriptionLoading] = useState<
    string | null
  >(null);
  const [openMenuEmail, setOpenMenuEmail] = useState<string | null>(null);
  const [batchAutoSwitchLoading, setBatchAutoSwitchLoading] = useState(false);
  const [showBatchEditTags, setShowBatchEditTags] = useState(false);
  const [batchEditingTags, setBatchEditingTags] = useState<
    Array<{ text: string; color: string }>
  >([{ text: "", color: "#3B82F6" }]);
  const [batchEditTagsLoading, setBatchEditTagsLoading] = useState(false);
  const [codexPreviewFile, setCodexPreviewFile] =
    useState<CodexTokenFileInfo | null>(null);
  const { showConfirm, ConfirmDialog } = useConfirmDialog();

  const SUBSCRIPTION_CACHE_DURATION = 5 * 60 * 1000; // 5分钟缓存有效期

  // 初始化：加载账户数据
  useEffect(() => {
    loadAccounts();
    fetchActualCurrentToken();
  }, []);

  // 监听账户数据变化，自动刷新当前Token（用于切换账户后更新高亮）
  useEffect(() => {
    // 优先使用 accountData.current_account 的 token
    if (accountData?.current_account?.token) {
      console.log(
        "🔄 从 current_account 更新当前token:",
        accountData.current_account.email
      );
      setActualCurrentToken(accountData.current_account.token);
    } else {
      // 如果 current_account 为空，则调用 API 获取
      console.log("🔄 current_account 为空，调用 API 获取当前token...");
      fetchActualCurrentToken();

      // 延迟500ms再获取一次，确保Cursor配置文件已经写入完成
      const timer = setTimeout(() => {
        console.log("🔄 延迟500ms后再次获取当前token...");
        fetchActualCurrentToken();
      }, 500);

      return () => clearTimeout(timer);
    }
  }, [accountData?.current_account?.token, lastUpdated]); // 监听当前账户token和最后更新时间

  // 检查订阅信息缓存是否有效
  const isSubscriptionCacheValid = () => {
    const now = Date.now();
    return (
      subscriptionCache.isValid &&
      now - subscriptionCache.timestamp < SUBSCRIPTION_CACHE_DURATION
    );
  };

  // 从缓存获取订阅信息
  const getSubscriptionFromCache = (email: string) => {
    if (!isSubscriptionCacheValid()) return null;
    return subscriptionCache.data.get(email);
  };

  // 更新订阅信息缓存
  const updateSubscriptionCache = (email: string, subscriptionData: any) => {
    const newCache = {
      data: new Map(subscriptionCache.data),
      timestamp: Date.now(),
      isValid: true,
    };
    newCache.data.set(email, subscriptionData);
    setSubscriptionCache(newCache);
  };

  // 清空订阅信息缓存
  const clearSubscriptionCache = () => {
    setSubscriptionCache({
      data: new Map(),
      timestamp: 0,
      isValid: false,
    });
  };

  // 更新获取订阅信息设置
  const updateFetchSubscriptionInfo = (checked: boolean) => {
    setFetchSubscriptionInfo(checked);
    localStorage.setItem("fetchSubscriptionInfo", JSON.stringify(checked));
    if (!checked) {
      clearSubscriptionCache();
    }
  };

  // 获取当前Cursor实际使用的token
  const fetchActualCurrentToken = async () => {
    try {
      const tokenInfo = await CursorService.getTokenAuto();
      if (tokenInfo && tokenInfo.token) {
        setActualCurrentToken(tokenInfo.token);
      } else {
        setActualCurrentToken(null);
      }
    } catch (error) {
      console.error("获取当前token失败:", error);
      setActualCurrentToken(null);
    }
  };

  const loadCodexTokenFiles = async () => {
    try {
      setCodexLoading(true);
      const files = await CursorService.listCodexTokenFiles();
      setCodexTokenFiles(files);
    } catch (error) {
      console.error("加载 Codex token 列表失败:", error);
      setCodexTokenFiles([]);
    } finally {
      setCodexLoading(false);
    }
  };

  // 加载账户数据
  const loadAccounts = async (forceRefresh: boolean = false) => {
    try {
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

      if (result.success && result.accounts) {
        setAccountData(result);
        setLoading(false);

        if (fetchSubscriptionInfo) {
          result.accounts.forEach(async (account, index) => {
            // 添加空值检查
            if (!account) return;

            try {
              const cachedData = getSubscriptionFromCache(account.email);
              let authResult;

              if (cachedData && !forceRefresh) {
                authResult = cachedData;
              } else {
                authResult = await CursorService.getSubscriptionInfoOnly(
                  account.token
                );
                if (authResult.success) {
                  updateSubscriptionCache(account.email, authResult);
                }
              }

              const currentData = useAccountStore.getState().accountData;
              if (!currentData?.accounts) return;

              const updatedAccounts = [...currentData.accounts];

              if (authResult.success && authResult.user_info?.account_info) {
                const subscriptionType =
                  authResult.user_info?.account_info?.subscription_type;

                updatedAccounts[index] = {
                  ...updatedAccounts[index],
                  subscription_type: subscriptionType,
                  subscription_status:
                    authResult.user_info?.account_info?.subscription_status,
                  trial_days_remaining:
                    authResult.user_info?.account_info?.trial_days_remaining,
                  auth_status: "authorized",
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
            }
          });
        } else {
          const updatedAccounts = result.accounts
            .filter((account) => account) // 过滤掉 null/undefined
            .map((account) => ({
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
    } finally {
      setLoading(false);
    }
  };

  // 刷新账户数据
  const handleRefresh = async () => {
    setRefreshing(true);
    try {
      if (accountTypeView === "codex") {
        await loadCodexTokenFiles();
      } else {
        clearSubscriptionCache();
        await loadAccounts(true);
        await fetchActualCurrentToken();
      }
    } catch (error) {
      console.error("刷新失败:", error);
    } finally {
      setRefreshing(false);
    }
  };

  useEffect(() => {
    if (accountTypeView === "codex") {
      loadCodexTokenFiles();
    }
  }, [accountTypeView]);

  useEffect(() => {
    if (!codexPreviewFile) return;
    const originalOverflow = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    return () => {
      document.body.style.overflow = originalOverflow;
    };
  }, [codexPreviewFile]);

  // 批量操作相关函数
  const handleSelectAccount = (email: string, checked: boolean) => {
    const newSelected = new Set(selectedAccounts);
    if (checked) {
      newSelected.add(email);
    } else {
      newSelected.delete(email);
    }
    setSelectedAccounts(newSelected);
    setShowBatchActions(newSelected.size > 0);
  };

  // 精准更新账户标签
  const handleUpdateAccountTags = (
    email: string,
    tags: Array<{ text: string; color: string }>
  ) => {
    if (!accountData) return;

    const updatedAccounts = accountData.accounts.map((acc) => {
      if (acc.email === email) {
        return { ...acc, custom_tags: tags };
      }
      return acc;
    });

    setAccountData({
      ...accountData,
      accounts: updatedAccounts,
    });
  };

  const handleSelectAll = (checked: boolean) => {
    if (checked && accountData?.accounts) {
      // 获取当前标签页显示的账户列表
      let currentAccounts: AccountInfo[] = [];

      if (activeTab === "all") {
        currentAccounts = accountData.accounts.filter((account) => account);
      } else if (activeTab === "自动轮换") {
        currentAccounts = accountData.accounts
          .filter((account) => account)
          .filter((account) => account.isAutoSwitch === true);
      } else {
        const grouped = getGroupedAccounts();
        currentAccounts = grouped[activeTab] || [];
      }

      // 如果是免费版且选择了子分组，进一步过滤
      if (activeTab === "免费版" && activeFreeSubgroup !== "all") {
        const freeSubgroups = getFreeAccountSubgroups();
        currentAccounts = freeSubgroups[activeFreeSubgroup] || [];
      }

      // 获取当前显示账户的邮箱列表
      const currentEmails = currentAccounts.map((acc) => acc.email);
      setSelectedAccounts(new Set(currentEmails));
      setShowBatchActions(true);
    } else {
      setSelectedAccounts(new Set());
      setShowBatchActions(false);
    }
  };

  const handleBatchDelete = () => {
    if (selectedAccounts.size === 0) return;

    showConfirm({
      title: "🗑️ 批量删除账户",
      message: `确定要删除选中的 ${selectedAccounts.size} 个账户吗？此操作不可撤销。`,
      confirmText: "删除",
      cancelText: "取消",
      type: "danger",
      onConfirm: async () => {
        let successCount = 0;
        let failCount = 0;

        for (const email of selectedAccounts) {
          try {
            const result = await AccountService.removeAccount(email);
            if (result.success) {
              successCount++;
            } else {
              failCount++;
            }
          } catch (error) {
            failCount++;
          }
        }

        alert(`批量删除完成：成功 ${successCount} 个，失败 ${failCount} 个`);
        setSelectedAccounts(new Set());
        setShowBatchActions(false);
        await loadAccounts(true);
      },
    });
  };

  const handleBatchExport = async () => {
    if (selectedAccounts.size === 0) return;

    const selectedAccountsData = accountData?.accounts
      ?.filter((acc) => acc) // 过滤掉 null/undefined
      ?.filter((acc) => selectedAccounts.has(acc.email));

    if (!selectedAccountsData || selectedAccountsData.length === 0) {
      alert("没有找到选中的账户数据");
      return;
    }

    const exportData = selectedAccountsData.map((account) => {
      const data: any = {
        token: account.token,
      };

      if (includeWorkosTokenInExport) {
        data.workos_cursor_session_token = account.workos_cursor_session_token;
      }

      if (includeEmailInExport) {
        data.email = account.email;
      }

      return data;
    });

    try {
      const result = await invoke<{ success: boolean; message: string }>(
        "write_clipboard",
        {
          text: JSON.stringify(exportData, null, 2),
        }
      );

      if (result.success) {
        alert(`✅ 已复制 ${selectedAccounts.size} 个账户数据到剪贴板`);
      } else {
        alert(`❌ 复制到剪贴板失败: ${result.message}`);
      }
    } catch (error) {
      console.error("复制到剪贴板失败:", error);
      alert(`❌ 复制到剪贴板失败: ${error}`);
    }
  };

  const handleImportFromClipboard = async () => {
    try {
      // 读取剪贴板内容
      const result = await invoke<{
        success: boolean;
        data?: string;
        message: string;
      }>("read_clipboard");

      // 先检查读取操作是否成功
      if (!result.success) {
        alert(`❌ 读取剪贴板失败: ${result.message}`);
        return;
      }

      // 再检查数据是否存在且不为空
      if (!result.data || result.data.trim() === "") {
        alert("❌ 剪贴板为空，请先复制账户数据");
        return;
      }

      const clipboardText = result.data;

      // 尝试解析 JSON
      let importData: any[];
      try {
        importData = JSON.parse(clipboardText);
      } catch (parseError) {
        const demoFormat = `[{"token": "eyJhbGc...","workos_cursor_session_token": "user_01K8QZ3..." (可选),"email": "example@icloud.com" (可选)}]`;
        alert(
          `❌ JSON 格式错误，请确保复制的是正确的格式\n\n正确格式示例：\n${demoFormat}\n\n注意：workos_cursor_session_token 和 email 都是可选字段`
        );
        return;
      }

      // 验证是否为数组
      if (!Array.isArray(importData)) {
        alert("❌ 数据格式错误：必须是 JSON 数组格式");
        return;
      }

      if (importData.length === 0) {
        alert("❌ 没有可导入的账户数据");
        return;
      }

      // 验证和处理每个账户
      const validAccounts: Array<{
        token: string;
        workos_cursor_session_token?: string;
        email: string;
      }> = [];

      // 获取现有账户的所有 token
      const existingTokens = new Set(
        accountData?.accounts
          ? accountData.accounts
              .filter((acc) => acc && acc.token)
              .map((acc) => acc.token)
          : []
      );

      const duplicateTokens: string[] = [];

      for (let i = 0; i < importData.length; i++) {
        const item = importData[i];

        // 验证必需字段
        if (!item.token || typeof item.token !== "string") {
          alert(`❌ 第 ${i + 1} 个账户缺少 token 字段或格式错误`);
          return;
        }

        // workos_cursor_session_token 是可选的，不强制验证

        // 检查 token 是否重复
        if (existingTokens.has(item.token)) {
          // 找到对应的邮箱
          const existingAccount = accountData?.accounts?.find(
            (acc) => acc && acc.token === item.token
          );
          const emailInfo = existingAccount
            ? existingAccount.email
            : "未知邮箱";
          duplicateTokens.push(`第 ${i + 1} 个账户 (已存在: ${emailInfo})`);
          continue; // 跳过重复的账户
        }

        // 如果没有 email，生成一个
        let email = item.email;
        if (!email || typeof email !== "string" || email.trim() === "") {
          const timestamp = Date.now() + i; // 加上索引避免时间戳完全相同
          email = `${timestamp}@auto-cursor.com`;
        }

        validAccounts.push({
          token: item.token,
          workos_cursor_session_token:
            item.workos_cursor_session_token || undefined,
          email: email.trim(),
        });
      }

      // 如果有重复的 token，提示用户
      if (duplicateTokens.length > 0) {
        const message = `❌ 发现重复的账户，已自动跳过：\n\n${duplicateTokens.join(
          "\n"
        )}${
          validAccounts.length > 0
            ? `\n\n将继续导入其余 ${validAccounts.length} 个账户`
            : ""
        }`;
        alert(message);

        // 如果所有账户都重复，直接返回
        if (validAccounts.length === 0) {
          return;
        }
      }

      // 批量添加账户
      let successCount = 0;
      let failCount = 0;
      const errors: string[] = [];

      for (const account of validAccounts) {
        try {
          const result = await AccountService.addAccount(
            account.email,
            account.token,
            account.token, // refreshToken (不需要)
            account.workos_cursor_session_token // workosSessionToken
          );

          if (result.success) {
            successCount++;
          } else {
            failCount++;
            errors.push(`${account.email}: ${result.message}`);
          }
        } catch (error) {
          failCount++;
          errors.push(`${account.email}: ${error}`);
        }
      }

      // 刷新账户列表
      await loadAccounts(true);

      // 显示结果
      let message = `导入完成！\n✅ 成功: ${successCount} 个\n❌ 失败: ${failCount} 个`;
      if (errors.length > 0 && errors.length <= 5) {
        message += `\n\n失败原因：\n${errors.join("\n")}`;
      } else if (errors.length > 5) {
        message += `\n\n部分失败原因：\n${errors
          .slice(0, 5)
          .join("\n")}\n...还有 ${errors.length - 5} 个错误`;
      }

      alert(message);
    } catch (error) {
      console.error("导入失败:", error);
      alert(`❌ 导入失败: ${error}`);
    }
  };

  // 切换排序顺序
  const handleToggleSortOrder = () => {
    const newOrder = toggleAccountSortOrder();
    setSortOrder(newOrder);
  };

  // 切换排序字段
  const handleToggleSortField = () => {
    const newField = toggleAccountSortField();
    setSortField(newField);
  };

  const handleBatchSetAutoSwitch = (isAutoSwitch: boolean) => {
    if (selectedAccounts.size === 0) return;

    const action = isAutoSwitch ? "启用" : "禁用";
    const actionIcon = isAutoSwitch ? "🔄" : "⏸️";

    showConfirm({
      title: `${actionIcon} ${action}自动轮换`,
      message: `确定要为选中的 ${selectedAccounts.size} 个账户${action}自动轮换吗？`,
      confirmText: `${action}轮换`,
      cancelText: "取消",
      type: isAutoSwitch ? "info" : "warning",
      onConfirm: async () => {
        setBatchAutoSwitchLoading(true);
        try {
          const emails = Array.from(selectedAccounts);
          const result = await AccountService.batchSetAutoSwitch(
            emails,
            isAutoSwitch
          );

          if (result.success) {
            alert(`批量设置完成：成功更新 ${result.updated_count} 个账户`);
            setSelectedAccounts(new Set());
            setShowBatchActions(false);
            await loadAccounts(true); // 重新加载账户列表
          } else {
            alert(`批量设置失败：${result.message}`);
          }
        } catch (error) {
          alert(
            `批量设置失败：${
              error instanceof Error ? error.message : "未知错误"
            }`
          );
        } finally {
          setBatchAutoSwitchLoading(false);
        }
      },
    });
  };

  // 批量编辑标签
  const handleBatchEditTags = () => {
    if (selectedAccounts.size === 0) return;
    // 初始化标签编辑状态，默认一个空标签
    setBatchEditingTags([{ text: "", color: "#3B82F6" }]);
    setShowBatchEditTags(true);
  };

  // 保存批量编辑的标签
  const handleSaveBatchTags = async () => {
    if (selectedAccounts.size === 0) return;

    // 过滤掉空标签，只保留第一个非空标签（限制只能1个）
    const filteredTags = batchEditingTags
      .filter((tag) => tag.text.trim() !== "")
      .slice(0, 1);

    if (filteredTags.length === 0) {
      alert("请至少输入一个标签");
      return;
    }

    setBatchEditTagsLoading(true);
    try {
      const emails = Array.from(selectedAccounts);
      let successCount = 0;
      let failCount = 0;
      const successEmails: string[] = [];

      // 循环更新每个账户的标签
      for (const email of emails) {
        try {
          const result = await AccountService.updateCustomTags(
            email,
            filteredTags
          );
          if (result.success) {
            successCount++;
            successEmails.push(email);
          } else {
            failCount++;
            console.error(`更新 ${email} 的标签失败:`, result.message);
          }
        } catch (error) {
          failCount++;
          console.error(`更新 ${email} 的标签失败:`, error);
        }
      }

      if (successCount > 0) {
        // 批量更新前端显示 - 使用 getState() 获取最新状态
        const currentData = useAccountStore.getState().accountData;
        if (currentData) {
          const updatedAccounts = currentData.accounts.map((acc) => {
            if (successEmails.includes(acc.email)) {
              return { ...acc, custom_tags: filteredTags };
            }
            return acc;
          });
          setAccountData({
            ...currentData,
            accounts: updatedAccounts,
          });
        }

        alert(
          `批量更新标签完成：成功 ${successCount} 个${
            failCount > 0 ? `，失败 ${failCount} 个` : ""
          }`
        );
        setShowBatchEditTags(false);
        setBatchEditingTags([{ text: "", color: "#3B82F6" }]);
        setSelectedAccounts(new Set());
        setShowBatchActions(false);
      } else {
        alert(`批量更新标签失败：所有账户更新都失败了`);
      }
    } catch (error) {
      alert(
        `批量更新标签失败：${
          error instanceof Error ? error.message : "未知错误"
        }`
      );
    } finally {
      setBatchEditTagsLoading(false);
    }
  };

  // 如果没有账户数据，显示空状态（Cursor 视图）
  if (
    accountTypeView === "cursor" &&
    (!accountData?.accounts || accountData.accounts.length === 0)
  ) {
    return (
      <div>
        <div className="flex items-center justify-between mb-4">
          <div className="flex items-center space-x-3">
            <h4 className="font-medium text-slate-900 dark:text-slate-100 text-md">账户列表</h4>
          </div>
        </div>
        <p className="text-muted text-sm">暂无保存的账户</p>
      </div>
    );
  }

  const groupedAccounts = getGroupedAccounts();
  const accounts = accountData?.accounts ?? [];

  // 获取自动轮换账户
  const autoSwitchAccounts = accounts
    .filter((account) => account) // 过滤掉 null/undefined
    .filter((account) => account.isAutoSwitch === true);

  let accountsToShow =
    activeTab === "all"
      ? accounts.filter((account) => account) // 过滤掉 null/undefined
      : activeTab === "自动轮换"
      ? autoSwitchAccounts
      : groupedAccounts[activeTab] || [];

  // 如果当前是免费版分组，且选择了特定的子分组，则进一步过滤
  if (activeTab === "免费版" && activeFreeSubgroup !== "all") {
    const freeSubgroups = getFreeAccountSubgroups();
    accountsToShow = freeSubgroups[activeFreeSubgroup] || [];
  }

  // 应用排序
  accountsToShow = [...accountsToShow].sort((a, b) => {
    if (sortField === "created_at") {
      // 按创建时间排序
      // 解析日期（created_at 格式：YYYY-MM-DD HH:MM:SS 或 YYYY/M/D HH:MM:SS）
      const getTimestamp = (dateStr: string) => {
        if (!dateStr) return 0;
        return new Date(dateStr).getTime();
      };

      const timeA = getTimestamp(a.created_at);
      const timeB = getTimestamp(b.created_at);

      if (sortOrder === "asc") {
        return timeA - timeB; // 升序：旧的在前
      } else {
        return timeB - timeA; // 降序：新的在前
      }
    } else {
      // 按剩余试用天数排序
      const daysA = a.trial_days_remaining ?? -1; // 没有试用天数的放最后
      const daysB = b.trial_days_remaining ?? -1;

      if (sortOrder === "asc") {
        return daysA - daysB; // 升序：少的在前
      } else {
        return daysB - daysA; // 降序：多的在前
      }
    }
  });

  // 应用搜索筛选
  if (searchQuery.trim()) {
    const query = searchQuery.toLowerCase().trim();
    accountsToShow = accountsToShow.filter((account) => {
      // 搜索邮箱
      if (account.email.toLowerCase().includes(query)) {
        return true;
      }
      // 搜索自定义标签
      if (account.custom_tags && account.custom_tags.length > 0) {
        return account.custom_tags.some((tag) =>
          tag.text.toLowerCase().includes(query)
        );
      }
      return false;
    });
  }

  return (
    <div>
      {/* 标题栏和刷新按钮 */}
      <div className="flex items-center justify-between mb-4">
        <div className="flex items-center space-x-3">
          <h4 className="font-medium text-slate-900 dark:text-slate-100 text-md">账户列表</h4>
          <div className="surface-secondary flex items-center rounded-lg p-1">
            <button
              type="button"
              onClick={() => setAccountTypeView("cursor")}
              className={`px-2 py-1 text-xs rounded-md transition-colors ${
                accountTypeView === "cursor"
                  ? "surface-elevated text-blue-700 dark:text-blue-300 shadow"
                  : "text-subtle hover:text-slate-900 dark:hover:text-slate-100"
              }`}
            >
              cursor
            </button>
            <button
              type="button"
              onClick={() => setAccountTypeView("codex")}
              className={`px-2 py-1 text-xs rounded-md transition-colors ${
                accountTypeView === "codex"
                  ? "surface-elevated text-violet-700 dark:text-violet-300 shadow"
                  : "text-subtle hover:text-slate-900 dark:hover:text-slate-100"
              }`}
            >
              codex
            </button>
          </div>
          <button
            type="button"
            onClick={handleRefresh}
            disabled={refreshing}
            className="inline-flex items-center px-3 py-1 text-xs font-medium text-blue-700 bg-blue-100 border border-transparent rounded hover:bg-blue-200 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-blue-500 disabled:opacity-50"
          >
            {refreshing ? "🔄 刷新中..." : "🔄 刷新"}
          </button>
          {accountTypeView === "cursor" && (
            <>
              <button
                type="button"
                onClick={handleToggleSortField}
                className="inline-flex items-center px-3 py-1 text-xs font-medium text-purple-700 bg-purple-100 border border-transparent rounded hover:bg-purple-200 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-purple-500"
                title={
                  sortField === "created_at"
                    ? "当前：按创建时间排序，点击切换为按剩余天数"
                    : "当前：按剩余天数排序，点击切换为按创建时间"
                }
              >
                {sortField === "created_at" ? "📅 创建时间" : "⏰ 剩余天数"}
              </button>
              <button
                type="button"
                onClick={handleToggleSortOrder}
                className="surface-secondary inline-flex items-center rounded border border-transparent px-3 py-1 text-xs font-medium text-slate-700 hover:bg-slate-200/80 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-gray-500 dark:text-slate-300 dark:hover:bg-slate-700/70"
                title={
                  sortOrder === "asc"
                    ? "当前：升序，点击切换为降序"
                    : "当前：降序，点击切换为升序"
                }
              >
                {sortOrder === "asc" ? "↑ 升序" : "↓ 降序"}
              </button>
            </>
          )}
        </div>
        <div className="flex items-center space-x-2">
          {useAccountStore.getState().lastUpdated && (
            <span className="text-muted text-xs">
              最后更新:{" "}
              {new Date(
                useAccountStore.getState().lastUpdated!
              ).toLocaleString()}
            </span>
          )}
        </div>
      </div>

      {/* 搜索框 */}
      {accountTypeView === "cursor" && (
        <div className="mb-4">
        <div className="relative">
          <input
            type="text"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            placeholder="🔍 搜索邮箱或自定义标签..."
            className="field-input px-4 py-2"
          />
          {searchQuery && (
            <button
              onClick={() => setSearchQuery("")}
              className="text-muted absolute right-3 top-1/2 -translate-y-1/2 transform hover:text-slate-700 dark:hover:text-slate-200"
              title="清除搜索"
            >
              ✕
            </button>
          )}
        </div>
        {searchQuery && (
          <div className="text-muted mt-2 text-xs">
            找到 {accountsToShow.length} 个匹配的账户
          </div>
        )}
      </div>
      )}

      {/* 分组标签 */}
      {accountTypeView === "cursor" && <div className="flex flex-wrap gap-2 mb-4">
        <button
          type="button"
          onClick={() => setActiveTab("all")}
          className={`px-3 py-1.5 text-sm font-medium rounded-full border transition-colors ${
            activeTab === "all"
              ? "border-blue-400/40 bg-blue-100 text-blue-800 dark:border-blue-500/30 dark:bg-blue-500/15 dark:text-blue-200"
              : "surface-secondary border-subtle text-slate-700 hover:bg-slate-200/80 dark:text-slate-300 dark:hover:bg-slate-700/70"
          }`}
        >
          全部 ({accounts.length})
        </button>

        {/* 自动轮换标签 */}
        <button
          type="button"
          onClick={() => setActiveTab("自动轮换")}
          className={`px-3 py-1.5 text-sm font-medium rounded-full border transition-colors ${
            activeTab === "自动轮换"
              ? "border-purple-400/40 bg-purple-100 text-purple-800 dark:border-purple-500/30 dark:bg-purple-500/15 dark:text-purple-200"
              : "surface-secondary border-subtle text-slate-700 hover:bg-slate-200/80 dark:text-slate-300 dark:hover:bg-slate-700/70"
          }`}
        >
          🔄 自动轮换 ({autoSwitchAccounts.length})
        </button>

        {getAvailableGroups().map((group) => {
          const count = groupedAccounts[group]?.length || 0;
          return (
            <button
              key={group}
              type="button"
              onClick={() => setActiveTab(group)}
              className={`px-3 py-1.5 text-sm font-medium rounded-full border transition-colors ${
                activeTab === group
                  ? "border-blue-400/40 bg-blue-100 text-blue-800 dark:border-blue-500/30 dark:bg-blue-500/15 dark:text-blue-200"
                  : "surface-secondary border-subtle text-slate-700 hover:bg-slate-200/80 dark:text-slate-300 dark:hover:bg-slate-700/70"
              }`}
            >
              {group} ({count})
            </button>
          );
        })}
      </div>
      }

      {/* 免费版子分组 */}
      {accountTypeView === "cursor" && activeTab === "免费版" && (
        <div className="flex flex-wrap gap-2 mb-4 ml-4">
          <button
            type="button"
            onClick={() => setActiveFreeSubgroup("all")}
            className={`px-2 py-1 text-xs font-medium rounded-full border transition-colors ${
              activeFreeSubgroup === "all"
                ? "border-emerald-400/40 bg-emerald-100 text-emerald-800 dark:border-emerald-500/30 dark:bg-emerald-500/15 dark:text-emerald-200"
                : "surface-secondary border-subtle text-subtle hover:bg-slate-200/70 dark:hover:bg-slate-700/70"
            }`}
          >
            全部免费版 ({groupedAccounts["免费版"]?.length || 0})
          </button>
          {Object.entries(getFreeAccountSubgroups()).map(
            ([subgroup, accounts]) => (
              <button
                key={subgroup}
                type="button"
                onClick={() => setActiveFreeSubgroup(subgroup)}
                className={`px-2 py-1 text-xs font-medium rounded-full border transition-colors ${
                  activeFreeSubgroup === subgroup
                    ? "border-emerald-400/40 bg-emerald-100 text-emerald-800 dark:border-emerald-500/30 dark:bg-emerald-500/15 dark:text-emerald-200"
                    : "surface-secondary border-subtle text-subtle hover:bg-slate-200/70 dark:hover:bg-slate-700/70"
                }`}
              >
                {subgroup} ({accounts.length})
              </button>
            )
          )}
        </div>
      )}

      {/* 控制栏 */}
      {accountTypeView === "cursor" && <div className="flex items-center justify-between mb-4">
        <div className="flex items-center space-x-4 flex-nowrap">
          {accountData?.accounts && accountData.accounts.length > 0 && (
            <label className="text-subtle flex items-center space-x-2 whitespace-nowrap text-sm">
              <input
                type="checkbox"
                checked={
                  accountsToShow.length > 0 &&
                  accountsToShow.every((acc) => selectedAccounts.has(acc.email))
                }
                onChange={(e) => handleSelectAll(e.target.checked)}
                className="h-4 w-4 rounded border-slate-300 text-blue-600 focus:ring-blue-500 dark:border-slate-600 dark:bg-slate-900"
              />
              <span>全选</span>
            </label>
          )}
          <label
            className="text-subtle flex items-center space-x-2 whitespace-nowrap text-sm"
            title="勾选后会调用API获取每个账户的详细订阅信息，不勾选只加载本地文件数据"
          >
            <input
              type="checkbox"
              checked={fetchSubscriptionInfo}
              onChange={(e) => updateFetchSubscriptionInfo(e.target.checked)}
              className="h-4 w-4 rounded border-slate-300 text-blue-600 focus:ring-blue-500 dark:border-slate-600 dark:bg-slate-900"
            />
            <span>获取订阅状态</span>
          </label>

          {/* 缓存状态指示器 */}
          {/* {fetchSubscriptionInfo && isSubscriptionCacheValid() && (
            <div className="flex items-center space-x-1 text-xs text-green-600">
              <span>📋</span>
              <span
                title={`缓存时间: ${new Date(
                  subscriptionCache.timestamp
                ).toLocaleString()}`}
              >
                缓存有效 (
                {Math.ceil(
                  (SUBSCRIPTION_CACHE_DURATION -
                    (Date.now() - subscriptionCache.timestamp)) /
                    60000
                )}
                分钟)
              </span>
              <button
                type="button"
                onClick={clearSubscriptionCache}
                className="ml-1 text-red-500 hover:text-red-700"
                title="清空缓存"
              >
                🗑️
              </button>
            </div>
          )} */}
        </div>
        <div className="flex flex-wrap items-center gap-2 ml-8">
          {/* 批量操作按钮 */}
          {showBatchActions && (
            <>
              <div className="surface-accent flex items-center gap-3 rounded-md border px-3 py-1.5">
                <span className="whitespace-nowrap text-xs font-medium text-blue-700 dark:text-blue-200">
                  已选择 {selectedAccounts.size} 个账户
                </span>
                <div className="w-px h-4 bg-blue-300"></div>
                <label className="flex items-center gap-1.5 text-xs text-blue-600 whitespace-nowrap cursor-pointer">
                  <input
                    type="checkbox"
                    checked={includeWorkosTokenInExport}
                    onChange={(e) =>
                      setIncludeWorkosTokenInExport(e.target.checked)
                    }
                    className="h-3.5 w-3.5 rounded border-slate-300 text-blue-600 focus:ring-blue-500 dark:border-slate-600 dark:bg-slate-900"
                  />
                  <span>导出WorkOS Token</span>
                </label>
                <label className="flex items-center gap-1.5 text-xs text-blue-600 whitespace-nowrap cursor-pointer">
                  <input
                    type="checkbox"
                    checked={includeEmailInExport}
                    onChange={(e) => setIncludeEmailInExport(e.target.checked)}
                    className="h-3.5 w-3.5 rounded border-slate-300 text-blue-600 focus:ring-blue-500 dark:border-slate-600 dark:bg-slate-900"
                  />
                  <span>导出邮箱</span>
                </label>
              </div>
              <div className="flex items-center gap-2">
                <button
                  type="button"
                  onClick={handleBatchExport}
                  className="inline-flex items-center justify-center px-3 py-1.5 text-xs font-medium text-blue-700 bg-blue-100 border border-blue-200 rounded-md hover:bg-blue-200 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-offset-1 whitespace-nowrap transition-colors"
                >
                  📋 导出选中
                </button>
                <button
                  type="button"
                  onClick={() => handleBatchSetAutoSwitch(true)}
                  disabled={batchAutoSwitchLoading}
                  className="inline-flex items-center justify-center px-3 py-1.5 text-xs font-medium text-green-700 bg-green-100 border border-green-200 rounded-md hover:bg-green-200 focus:outline-none focus:ring-2 focus:ring-green-500 focus:ring-offset-1 disabled:opacity-50 disabled:cursor-not-allowed whitespace-nowrap transition-colors"
                >
                  {batchAutoSwitchLoading ? "⏳ 设置中..." : "🔄 启用轮换"}
                </button>
                <button
                  type="button"
                  onClick={() => handleBatchSetAutoSwitch(false)}
                  disabled={batchAutoSwitchLoading}
                  className="inline-flex items-center justify-center px-3 py-1.5 text-xs font-medium text-orange-700 bg-orange-100 border border-orange-200 rounded-md hover:bg-orange-200 focus:outline-none focus:ring-2 focus:ring-orange-500 focus:ring-offset-1 disabled:opacity-50 disabled:cursor-not-allowed whitespace-nowrap transition-colors"
                >
                  {batchAutoSwitchLoading ? "⏳ 设置中..." : "⏸️ 禁用轮换"}
                </button>
                <button
                  type="button"
                  onClick={handleBatchEditTags}
                  className="inline-flex items-center justify-center px-3 py-1.5 text-xs font-medium text-purple-700 bg-purple-100 border border-purple-200 rounded-md hover:bg-purple-200 focus:outline-none focus:ring-2 focus:ring-purple-500 focus:ring-offset-1 whitespace-nowrap transition-colors"
                >
                  🏷️ 批量编辑标签
                </button>
                <button
                  type="button"
                  onClick={handleBatchDelete}
                  className="inline-flex items-center justify-center px-3 py-1.5 text-xs font-medium text-red-700 bg-red-100 border border-red-200 rounded-md hover:bg-red-200 focus:outline-none focus:ring-2 focus:ring-red-500 focus:ring-offset-1 whitespace-nowrap transition-colors"
                >
                  🗑️ 删除选中
                </button>
              </div>
            </>
          )}

          {/* 从剪切板导入按钮 - 只在没有勾选账户时显示 */}
          {!showBatchActions && (
            <div className="flex items-center space-x-1">
              <button
                type="button"
                onClick={handleImportFromClipboard}
                className="inline-flex items-center px-3 py-1 text-xs font-medium text-purple-700 bg-purple-100 border border-transparent rounded hover:bg-purple-200 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-purple-500"
              >
                📥 从剪切板导入
              </button>
              <span className="text-muted text-xs">
                (必须是从软件导出的格式)
              </span>
            </div>
          )}
        </div>
      </div>
      }

      {/* 账户列表 - 网格布局两列 */}
      {accountTypeView === "cursor" ? (
        <div
          className={`grid grid-cols-1 lg:grid-cols-2 gap-3 transition-opacity duration-200 ${
            refreshing ? "opacity-60" : "opacity-100"
          }`}
        >
          {accountsToShow.map((account, index) => (
            <AccountCard
              key={`${account.email}-${index}`}
              account={account}
              index={index}
              isSelected={selectedAccounts.has(account.email)}
              actualCurrentToken={actualCurrentToken}
              updateAccessTokenLoading={updateAccessTokenLoading}
              manualBindCardLoading={manualBindCardLoading}
              cancelSubscriptionLoading={cancelSubscriptionLoading}
              openMenuEmail={openMenuEmail}
              onSelectAccount={handleSelectAccount}
              onSwitchAccount={onSwitchAccount}
              onRemoveAccount={onRemoveAccount}
              onEditAccount={onEditAccount}
              onViewUsage={onViewUsage}
              onUpdateAccessToken={onUpdateAccessToken}
              onReLoginAccount={onReLoginAccount}
              onViewDashboard={onViewDashboard}
              onManualBindCard={onManualBindCard}
              onCopyBindCardUrl={onCopyBindCardUrl}
              onCancelSubscription={onCancelSubscription}
              onDeleteCursorAccount={onDeleteCursorAccount}
              onSetOpenMenuEmail={setOpenMenuEmail}
              onUpdateAccountTags={handleUpdateAccountTags}
              formatDate={formatDate}
            />
          ))}
        </div>
      ) : (
        <div className="rounded-lg border border-violet-200/70 bg-violet-50/90 p-3 dark:border-violet-500/25 dark:bg-violet-500/10">
          {codexLoading ? (
            <p className="text-sm text-slate-600 dark:text-slate-300">加载 Codex token 列表中...</p>
          ) : codexTokenFiles.length === 0 ? (
            <p className="text-sm text-slate-600 dark:text-slate-300">
              暂未找到 Codex token 文件（用户目录 `.auto-cursor-vip/codex_tokens`）。
            </p>
          ) : (
            <div className="space-y-2">
              {codexTokenFiles.map((item) => (
                <div
                  key={item.file_path}
                  className="surface-elevated rounded border p-2"
                >
                  <div className="flex items-center justify-between gap-2">
                    <span className="text-xs text-slate-800 dark:text-slate-100">{item.file_name}</span>
                    <div className="flex items-center gap-2">
                      <button
                        type="button"
                        onClick={async () => {
                          await navigator.clipboard.writeText(item.content);
                          alert(`已复制 ${item.file_name}`);
                        }}
                        className="surface-secondary border-subtle rounded px-2 py-1 text-xs text-slate-700 hover:bg-slate-100 dark:text-slate-300 dark:hover:bg-slate-800"
                      >
                        复制
                      </button>
                      <button
                        type="button"
                        onClick={async () => {
                          // 去掉文件名，得到目录路径
                          const dirPath = item.file_path.replace(
                            /[\\/][^\\/]*$/,
                            ""
                          );
                          try {
                            await invoke<string>("open_directory_by_path", {
                              path: dirPath,
                            });
                          } catch (error) {
                            const msg =
                              error instanceof Error
                                ? error.message
                                : "未知错误";
                            alert(`打开文件夹失败：${msg}`);
                          }
                        }}
                        className="surface-secondary border-subtle rounded px-2 py-1 text-xs text-slate-700 hover:bg-slate-100 dark:text-slate-300 dark:hover:bg-slate-800"
                        title="打开对应文件夹"
                      >
                        📂 打开文件夹
                      </button>
                      <button
                        type="button"
                        onClick={() => setCodexPreviewFile(item)}
                        className="surface-accent cursor-pointer rounded border px-2 py-1 text-xs text-blue-700 dark:text-blue-200"
                      >
                        查看
                      </button>
                    </div>
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>
      )}

      {/* Codex 文件预览弹窗 */}
      {codexPreviewFile &&
        createPortal(
          <div className="fixed inset-0 z-[9999] grid place-items-center bg-black/65 p-4">
            <div className="panel-floating flex h-[78vh] w-[92vw] max-w-4xl flex-col rounded-xl p-4">
              <div className="mb-3 flex items-center justify-between gap-3">
                <div className="min-w-0">
                  <h3 className="truncate text-sm font-semibold text-slate-900 dark:text-slate-100">
                    {codexPreviewFile.file_name}
                  </h3>
                  <p className="truncate text-xs text-slate-500 dark:text-slate-400">
                    {codexPreviewFile.file_path}
                  </p>
                </div>
                <button
                  type="button"
                  onClick={() => setCodexPreviewFile(null)}
                  className="surface-secondary border-subtle rounded px-2 py-1 text-xs text-slate-700 hover:bg-slate-100 dark:text-slate-300 dark:hover:bg-slate-800"
                >
                  关闭
                </button>
              </div>

              <pre className="panel-code min-h-0 flex-1 overflow-auto whitespace-pre-wrap rounded p-3 text-xs">
                {codexPreviewFile.content}
              </pre>
            </div>
          </div>,
          document.body
        )}

      {/* 批量编辑标签模态框 */}
      {showBatchEditTags && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black bg-opacity-50">
          <div className="panel-floating w-full max-w-md rounded-lg p-6">
            <h3 className="mb-4 text-lg font-semibold">
              批量编辑标签（已选择 {selectedAccounts.size} 个账户）
            </h3>

            <div className="mb-4 space-y-3 overflow-y-auto max-h-64">
              {batchEditingTags.map((tag, index) => (
                <div key={index} className="flex items-center gap-2">
                  <input
                    type="text"
                    value={tag.text}
                    onChange={(e) => {
                      const newTags = [...batchEditingTags];
                      newTags[index].text = e.target.value.slice(0, 10);
                      setBatchEditingTags(newTags);
                    }}
                    placeholder="标签文案（最多10字）"
                    maxLength={10}
                    className="field-input flex-1 focus:ring-purple-500"
                  />
                  <input
                    type="color"
                    value={tag.color}
                    onChange={(e) => {
                      const newTags = [...batchEditingTags];
                      newTags[index].color = e.target.value;
                      setBatchEditingTags(newTags);
                    }}
                    aria-label="标签颜色"
                    className="h-10 w-12 cursor-pointer rounded-md border border-slate-300 bg-white dark:border-slate-600 dark:bg-slate-900"
                  />
                  <button
                    onClick={() => {
                      const newTags = batchEditingTags.filter(
                        (_, i) => i !== index
                      );
                      setBatchEditingTags(newTags);
                    }}
                    className="rounded-md bg-red-100 px-2 py-2 text-sm text-red-700 hover:bg-red-200 dark:bg-red-500/15 dark:text-red-300 dark:hover:bg-red-500/25"
                  >
                    删除
                  </button>
                </div>
              ))}
            </div>

            <button
              onClick={() => {
                if (batchEditingTags.length < 1) {
                  setBatchEditingTags([
                    ...batchEditingTags,
                    { text: "", color: "#3B82F6" },
                  ]);
                }
              }}
              disabled={batchEditingTags.length >= 1}
              className={`mb-4 w-full rounded-md px-4 py-2 text-sm ${
                batchEditingTags.length >= 1
                  ? "cursor-not-allowed bg-slate-100 text-slate-400 dark:bg-slate-800 dark:text-slate-500"
                  : "bg-purple-100 text-purple-700 hover:bg-purple-200 dark:bg-purple-500/15 dark:text-purple-200 dark:hover:bg-purple-500/25"
              }`}
            >
              + 添加标签 {batchEditingTags.length >= 1 && "(最多1个)"}
            </button>

            <div className="flex justify-end gap-2">
              <button
                onClick={() => {
                  setShowBatchEditTags(false);
                  setBatchEditingTags([{ text: "", color: "#3B82F6" }]);
                }}
                disabled={batchEditTagsLoading}
                className="surface-secondary rounded-md px-4 py-2 text-sm text-slate-700 hover:bg-slate-200/80 disabled:cursor-not-allowed disabled:opacity-50 dark:text-slate-300 dark:hover:bg-slate-700/70"
              >
                取消
              </button>
              <button
                onClick={handleSaveBatchTags}
                disabled={batchEditTagsLoading}
                className="px-4 py-2 text-sm text-white bg-purple-600 rounded-md hover:bg-purple-700 disabled:opacity-50 disabled:cursor-not-allowed"
              >
                {batchEditTagsLoading ? "保存中..." : "保存"}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* 确认对话框 */}
      <ConfirmDialog />
    </div>
  );
};
