import type { AccountInfo } from "../types/account";

/**
 * 获取账户剩余天数的显示文本
 * @param account 账户信息
 * @returns string 剩余天数的显示文本
 */
export const getRemainingDays = (account: AccountInfo): string => {
  if (
    account.trial_days_remaining !== undefined &&
    account.trial_days_remaining !== null
  ) {
    return `${account.trial_days_remaining} 天`;
  }
  if (account.subscription_type) {
    if (
      account.subscription_type.toLowerCase().includes("pro") ||
      account.subscription_type.toLowerCase().includes("business")
    ) {
      return "付费订阅";
    }
    if (account.subscription_type.toLowerCase().includes("trial")) {
      return "试用中";
    }
  }
  return "未知";
};

/**
 * 检查账户是否有有效的WorkOS Session Token
 * @param account 账户信息
 * @returns boolean
 */
export const hasValidWorkosToken = (account: AccountInfo): boolean => {
  return !!account.workos_cursor_session_token?.trim();
};

/**
 * 检查账户是否有有效的Token
 * @param account 账户信息
 * @returns boolean
 */
export const hasValidToken = (account: AccountInfo): boolean => {
  return !!account.token?.trim();
};

/**
 * 检查邮箱地址是否有效
 * @param email 邮箱地址
 * @returns boolean
 */
export const isValidEmail = (email: string): boolean => {
  return email.trim().includes("@");
};

/**
 * 获取账户的授权状态显示文本
 * @param account 账户信息
 * @returns string
 */
export const getAuthStatusText = (account: AccountInfo): string => {
  switch (account.auth_status) {
    case "authorized":
      return "已授权";
    case "unauthorized":
      return "未授权";
    case "error":
      return "错误";
    case "network_error":
      return "网络错误";
    case "not_fetched":
      return "未获取";
    default:
      return "未知";
  }
};
