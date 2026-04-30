export interface HaozhumaPhoneFilters {
  isp: string;
  province: string;
  ascription: string;
  paragraph: string;
  exclude: string;
  uid: string;
  author: string;
}

export interface HaozhumaRetryConfig {
  max_phone_retry: number;
  poll_interval_seconds: number;
  send_check_timeout_seconds: number;
  sms_poll_timeout_seconds: number;
}

export interface HaozhumaConfig {
  enabled: boolean;
  api_domain: string;
  username: string;
  password: string;
  project_id: string;
  default_country_code: string;
  /**
   * 可选：如果配置了指定手机号，则注册时不再调用豪猪取号 API。
   * 支持带 '+' 或不带 '+' 的任意格式，Python 侧会自动抽取数字。
   */
  fixed_phone: string;
  phone_filters: HaozhumaPhoneFilters;
  retry: HaozhumaRetryConfig;
}

export interface HaozhumaConfigValidation {
  isValid: boolean;
  errors: string[];
}

export interface HaozhumaConfigSaveResult {
  success: boolean;
  message: string;
}

export const EMPTY_HAOZHUMA_CONFIG: HaozhumaConfig = {
  enabled: false,
  api_domain: "api.haozhuma.com",
  username: "",
  password: "",
  project_id: "",
  default_country_code: "86",
  fixed_phone: "",
  phone_filters: {
    isp: "",
    province: "",
    ascription: "",
    paragraph: "",
    exclude: "",
    uid: "",
    author: "",
  },
  retry: {
    max_phone_retry: 10,
    poll_interval_seconds: 1,
    send_check_timeout_seconds: 30,
    sms_poll_timeout_seconds: 90,
  },
};
