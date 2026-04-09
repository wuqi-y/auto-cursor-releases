// 邮箱配置相关的类型定义

export interface EmailConfig {
  worker_domain: string;
  email_domain: string;
  admin_password: string;
  access_password: string;
}

// 空的邮箱配置模板
export const EMPTY_EMAIL_CONFIG: EmailConfig = {
  worker_domain: "",
  email_domain: "",
  admin_password: "",
  access_password: "",
};

// 邮箱配置验证规则
export interface EmailConfigValidation {
  isValid: boolean;
  errors: string[];
}

// 邮箱配置保存结果
export interface EmailConfigSaveResult {
  success: boolean;
  message: string;
}
