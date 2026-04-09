export interface AccountInfo {
  email: string;
  token: string;
  refresh_token?: string;
  workos_cursor_session_token?: string;
  is_current: boolean;
  created_at: string;
  subscription_type?: string;
  subscription_status?: string;
  trial_days_remaining?: number;
  auth_status?: 'authorized' | 'unauthorized' | 'error' | 'network_error' | 'not_fetched';
  auth_error?: string;
  isAutoSwitch?: boolean;
  // 使用进度相关
  usage_progress?: {
    percentage: number; // 使用进度百分比
    individualUsed: number; // 已使用 (cents)
    individualLimit: number; // 总限额 (cents)
    individualUsedDollars: number; // 已使用 (dollars)
    individualLimitDollars: number; // 总限额 (dollars)
    message?: string; // 进度消息
  };
  // 自定义标签
  custom_tags?: Array<{
    text: string;
    color: string;
  }>;
}

export interface AccountListResult {
  success: boolean;
  accounts: AccountInfo[];
  current_account: AccountInfo | null;
  message: string;
}

export interface SwitchAccountResult {
  success: boolean;
  message: string;
  details: string[];
}

export interface AddAccountResult {
  success: boolean;
  message: string;
}

export interface EditAccountResult {
  success: boolean;
  message: string;
}

export interface RemoveAccountResult {
  success: boolean;
  message: string;
}

export interface LogoutResult {
  success: boolean;
  message: string;
  details: string[];
}
