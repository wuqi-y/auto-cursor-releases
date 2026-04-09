import { invoke } from "@tauri-apps/api/core";
import { AuthCheckResult, TokenInfo, BackupInfo, MachineIds, RestoreResult, ResetResult } from "../types/auth";

export interface CodexTokenFileInfo {
  file_name: string;
  file_path: string;
  updated_at_unix: number;
  content: string;
}

export class CursorService {
  // Cursor Installation
  static async checkCursorInstallation(): Promise<boolean> {
    return await invoke<boolean>("check_cursor_installation");
  }

  static async getCursorPaths(): Promise<[string, string]> {
    return await invoke<[string, string]>("get_cursor_paths");
  }

  static async getCursorVersion(): Promise<string> {
    return await invoke<string>("get_cursor_version");
  }

  static async debugCursorPaths(): Promise<string[]> {
    return await invoke<string[]>("debug_cursor_paths");
  }

  // Machine ID Management
  static async getCurrentMachineIds(): Promise<MachineIds> {
    return await invoke<MachineIds>("get_current_machine_ids");
  }

  static async getMachineIdFileContent(): Promise<string> {
    return await invoke<string>("get_machine_id_file_content");
  }

  static async resetMachineIds(): Promise<ResetResult> {
    return await invoke<ResetResult>("reset_machine_ids");
  }

  static async completeResetMachineIds(): Promise<ResetResult> {
    return await invoke<ResetResult>("complete_cursor_reset");
  }

  // Backup Management
  static async getBackups(): Promise<BackupInfo[]> {
    return await invoke<BackupInfo[]>("get_available_backups");
  }

  static async getBackupDebugInfo(): Promise<{ directory: string; files: string[] }> {
    const [directory, files] = await invoke<[string, string[]]>("get_backup_directory_info");
    return { directory, files };
  }

  static async extractBackupIds(backupPath: string): Promise<MachineIds> {
    return await invoke<MachineIds>("extract_backup_ids", { backupPath });
  }

  static async restoreMachineIds(backupPath: string): Promise<RestoreResult> {
    return await invoke<RestoreResult>("restore_machine_ids", { backupPath });
  }

  static async deleteBackup(backupPath: string): Promise<{ success: boolean; message: string }> {
    return await invoke<{ success: boolean; message: string }>("delete_backup", { backupPath });
  }

  // Token Management
  static async getTokenAuto(): Promise<TokenInfo> {
    return await invoke<TokenInfo>("get_token_auto");
  }

  static async listCodexTokenFiles(): Promise<CodexTokenFileInfo[]> {
    return await invoke<CodexTokenFileInfo[]>("list_codex_token_files");
  }

  static async checkUserAuthorized(token: string): Promise<AuthCheckResult> {
    return await invoke<AuthCheckResult>("check_user_authorization", { token });
  }

  static async getUserInfo(token: string): Promise<AuthCheckResult> {
    return await invoke<AuthCheckResult>("get_user_info", { token });
  }

  static async getSubscriptionInfoOnly(token: string): Promise<AuthCheckResult> {
    return await invoke<AuthCheckResult>("get_subscription_info_only", { token });
  }


  static async getCurrentPeriodUsage(token: string): Promise<any> {
    const result = await invoke<any>("get_current_period_usage", { token });
    console.log("🎯 GetCurrentPeriodUsage API 结果:", JSON.stringify(result, null, 2));
    return result;
  }

  // Logging Management
  static async getLogFilePath(): Promise<string> {
    return await invoke<string>("get_log_file_path");
  }

  static async testLogging(): Promise<string> {
    return await invoke<string>("test_logging");
  }

  static async openLogFile(): Promise<string> {
    return await invoke<string>("open_log_file");
  }

  static async openLogDirectory(): Promise<string> {
    return await invoke<string>("open_log_directory");
  }

  static async debugWindowsCursorPaths(): Promise<string[]> {
    return await invoke<string[]>("debug_windows_cursor_paths");
  }

  static async setCustomCursorPath(path: string): Promise<string> {
    return await invoke<string>("set_custom_cursor_path", { path });
  }

  static async getCustomCursorPath(): Promise<string | null> {
    return await invoke<string | null>("get_custom_cursor_path");
  }

  static async clearCustomCursorPath(): Promise<string> {
    return await invoke<string>("clear_custom_cursor_path");
  }

  // 浏览器路径管理
  static async selectBrowserFile(): Promise<string | null> {
    return await invoke<string | null>("select_browser_file");
  }

  static async setCustomBrowserPath(path: string): Promise<string> {
    return await invoke<string>("set_custom_browser_path", { path });
  }

  static async getCustomBrowserPath(): Promise<string | null> {
    return await invoke<string | null>("get_custom_browser_path");
  }

  static async clearCustomBrowserPath(): Promise<string> {
    return await invoke<string>("clear_custom_browser_path");
  }

  // 验证码登录
  static async verificationCodeLogin(
    email: string,
    verificationCode: string,
    showWindow?: boolean
  ): Promise<{ success: boolean; message: string }> {
    return await invoke("verification_code_login", {
      email,
      verificationCode,
      showWindow,
    });
  }

  // 检查验证码登录的 Cookie
  static async checkVerificationLoginCookies(): Promise<void> {
    return await invoke("check_verification_login_cookies");
  }

  // Tempmail注册
  static async registerWithTempmail(
    email: string,
    firstName: string,
    lastName: string,
    tempmailEmail: string,
    tempmailPin: string,
    useIncognito?: boolean,
    enableBankCardBinding?: boolean,
    skipPhoneVerification?: boolean,
    selectedCardIndex?: number,
    config?: any
  ): Promise<any> {
    return await invoke("register_with_tempmail", {
      email,
      firstName,
      lastName,
      tempmailEmail,
      tempmailPin,
      useIncognito,
      enableBankCardBinding,
      skipPhoneVerification,
      selectedCardIndex,
      config,
    });
  }

  /** 自建邮箱 API：按配置的 URL/Headers/JSON 路径拉取邮件原文并自动解析验证码 */
  static async registerWithSelfHostedMailApi(
    email: string,
    firstName: string,
    lastName: string,
    selfHostedMailUrl: string,
    selfHostedMailHeadersJson: string,
    selfHostedMailResponsePath: string,
    selfHostedMailClearEnabled?: boolean,
    selfHostedMailClearUrl?: string,
    selfHostedMailClearHeadersJson?: string,
    selfHostedMailClearMethod?: string,
    useIncognito?: boolean,
    enableBankCardBinding?: boolean,
    skipPhoneVerification?: boolean,
    selectedCardIndex?: number,
    config?: any
  ): Promise<any> {
    return await invoke("register_with_self_hosted_mail_api", {
      email,
      firstName,
      lastName,
      selfHostedMailUrl,
      selfHostedMailHeadersJson,
      selfHostedMailResponsePath,
      selfHostedMailClearEnabled,
      selfHostedMailClearUrl,
      selfHostedMailClearHeadersJson,
      selfHostedMailClearMethod,
      useIncognito,
      enableBankCardBinding,
      skipPhoneVerification,
      selectedCardIndex,
      config,
    });
  }

  static async registerCodexWithSelfHostedMailApi(
    email: string,
    firstName: string,
    lastName: string,
    selfHostedMailUrl: string,
    selfHostedMailHeadersJson: string,
    selfHostedMailResponsePath: string,
    selfHostedMailClearEnabled?: boolean,
    selfHostedMailClearUrl?: string,
    selfHostedMailClearHeadersJson?: string,
    selfHostedMailClearMethod?: string,
    useIncognito?: boolean,
    manualVerification?: boolean,
    config?: any
  ): Promise<any> {
    return await invoke("register_codex_with_self_hosted_mail_api", {
      email,
      firstName,
      lastName,
      selfHostedMailUrl,
      selfHostedMailHeadersJson,
      selfHostedMailResponsePath,
      selfHostedMailClearEnabled,
      selfHostedMailClearUrl,
      selfHostedMailClearHeadersJson,
      selfHostedMailClearMethod,
      useIncognito,
      manualVerification,
      config,
    });
  }
}
