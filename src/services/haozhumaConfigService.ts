import { invoke } from "@tauri-apps/api/core";
import {
  EMPTY_HAOZHUMA_CONFIG,
  HaozhumaConfig,
  HaozhumaConfigSaveResult,
  HaozhumaConfigValidation,
} from "../types/haozhumaConfig";

export class HaozhumaConfigService {
  static async testHaozhumaApi(
    config: HaozhumaConfig,
  ): Promise<{
    success: boolean;
    message: string;
    token_len?: number;
    phone_last4?: string | null;
  }> {
    try {
      const resultStr = await invoke<string>("test_haozhuma_api", {
        config,
      });
      const result = JSON.parse(resultStr) as {
        success: boolean;
        message: string;
        token_len?: number;
        phone_last4?: string | null;
      };
      return result;
    } catch (error) {
      return {
        success: false,
        message: `测试失败: ${error}`,
      };
    }
  }

  static async getHaozhumaConfig(): Promise<HaozhumaConfig> {
    try {
      const result = await invoke<string>("read_haozhuma_config");
      if (result) {
        const config = JSON.parse(result) as Partial<HaozhumaConfig>;
        return {
          ...EMPTY_HAOZHUMA_CONFIG,
          ...config,
          phone_filters: {
            ...EMPTY_HAOZHUMA_CONFIG.phone_filters,
            ...(config.phone_filters || {}),
          },
          retry: {
            ...EMPTY_HAOZHUMA_CONFIG.retry,
            ...(config.retry || {}),
          },
        };
      }
    } catch (error) {
      console.log("读取豪猪配置失败，返回空配置:", error);
    }
    return EMPTY_HAOZHUMA_CONFIG;
  }

  static async saveHaozhumaConfig(
    config: HaozhumaConfig,
  ): Promise<HaozhumaConfigSaveResult> {
    try {
      await invoke<string>("save_haozhuma_config", {
        config: JSON.stringify(config, null, 2),
      });
      return { success: true, message: "豪猪配置保存成功" };
    } catch (error) {
      console.error("保存豪猪配置失败:", error);
      return { success: false, message: `保存失败: ${error}` };
    }
  }

  static validateHaozhumaConfig(
    config: HaozhumaConfig,
  ): HaozhumaConfigValidation {
    const errors: string[] = [];

    if (!config.api_domain.trim()) {
      errors.push("API 域名不能为空");
    }
    if (!config.username.trim()) {
      errors.push("豪猪账号不能为空");
    }
    if (!config.password.trim()) {
      errors.push("豪猪密码不能为空");
    }
    if (!config.project_id.trim()) {
      errors.push("项目 ID 不能为空");
    }
    if (!/^\d+$/.test(config.default_country_code.trim())) {
      errors.push("默认国家区号必须是数字");
    }

    const fixedPhoneDigits = (config.fixed_phone || "")
      .trim()
      .replace(/[^\d]/g, "");
    if (fixedPhoneDigits) {
      if (fixedPhoneDigits.length < 6) {
        errors.push("指定手机号必须包含有效数字（至少 6 位）");
      }
    }

    const retryValues = Object.entries(config.retry);
    for (const [key, value] of retryValues) {
      if (!Number.isFinite(value) || value <= 0) {
        errors.push(`${key} 必须大于 0`);
      }
    }

    return {
      isValid: errors.length === 0,
      errors,
    };
  }
}
