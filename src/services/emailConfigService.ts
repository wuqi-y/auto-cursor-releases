import { invoke } from "@tauri-apps/api/core";
import { EmailConfig, EMPTY_EMAIL_CONFIG, EmailConfigValidation, EmailConfigSaveResult } from "../types/emailConfig";

export class EmailConfigService {

  /**
   * 获取邮箱配置
   */
  static async getEmailConfig(): Promise<EmailConfig> {
    try {
      const result = await invoke<string>('read_email_config');
      if (result) {
        const config = JSON.parse(result) as EmailConfig;
        return config;
      }
    } catch (error) {
      console.log('读取邮箱配置失败，返回空配置:', error);
    }
    return EMPTY_EMAIL_CONFIG;
  }

  /**
   * 保存邮箱配置
   */
  static async saveEmailConfig(config: EmailConfig): Promise<EmailConfigSaveResult> {
    try {
      await invoke<string>('save_email_config', { config: JSON.stringify(config) });
      return { success: true, message: '邮箱配置保存成功' };
    } catch (error) {
      console.error('保存邮箱配置失败:', error);
      return { success: false, message: `保存失败: ${error}` };
    }
  }

  /**
   * 验证邮箱配置
   */
  static validateEmailConfig(config: EmailConfig): EmailConfigValidation {
    const errors: string[] = [];

    // 验证 worker_domain
    if (!config.worker_domain || config.worker_domain.trim() === '') {
      errors.push('Worker域名不能为空');
    } else if (!this.isValidDomain(config.worker_domain)) {
      errors.push('Worker域名格式不正确');
    }

    // 验证 email_domain
    if (!config.email_domain || config.email_domain.trim() === '') {
      errors.push('邮箱域名不能为空');
    } else if (!this.isValidDomain(config.email_domain)) {
      errors.push('邮箱域名格式不正确');
    }

    // 验证 admin_password
    if (!config.admin_password || config.admin_password.trim() === '') {
      errors.push('管理员密码不能为空');
    } else if (config.admin_password.length < 6) {
      errors.push('管理员密码至少需要6位');
    }

    // 验证 access_password
    if (!config.access_password || config.access_password.trim() === '') {
      errors.push('访问密码不能为空');
    } else if (config.access_password.length < 6) {
      errors.push('访问密码至少需要6位');
    }

    return {
      isValid: errors.length === 0,
      errors,
    };
  }

  /**
   * 验证域名格式
   */
  private static isValidDomain(domain: string): boolean {
    const domainRegex = /^[a-zA-Z0-9][a-zA-Z0-9-]{0,61}[a-zA-Z0-9](?:\.[a-zA-Z0-9][a-zA-Z0-9-]{0,61}[a-zA-Z0-9])*$/;
    return domainRegex.test(domain);
  }

  /**
   * 测试邮箱配置连接
   */
  static async testEmailConfig(config: EmailConfig): Promise<{ success: boolean; message: string }> {
    try {
      // 这里可以实现一个简单的连接测试
      // 比如尝试访问 worker domain 的健康检查接口
      const testUrl = `https://${config.worker_domain}/`;

      // 使用 fetch 测试连接
      const controller = new AbortController();
      const timeoutId = setTimeout(() => controller.abort(), 5000); // 5秒超时

      const response = await fetch(testUrl, {
        method: 'GET',
        signal: controller.signal,
      });

      clearTimeout(timeoutId);

      if (response.ok) {
        return { success: true, message: '邮箱配置测试成功' };
      } else {
        return { success: false, message: `连接测试失败: HTTP ${response.status}` };
      }
    } catch (error) {
      if (error instanceof Error && error.name === 'AbortError') {
        return { success: false, message: '连接测试超时' };
      }
      return { success: false, message: `连接测试失败: ${error}` };
    }
  }
}
