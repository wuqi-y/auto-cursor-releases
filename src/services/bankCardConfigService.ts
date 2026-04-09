import { invoke } from "@tauri-apps/api/core";
import { BankCardConfig, BankCardConfigList, DEFAULT_BANK_CARD_CONFIG } from "../types/bankCardConfig";

export class BankCardConfigService {

  /**
   * 获取银行卡配置（单张卡，兼容旧版本）
   */
  static async getBankCardConfig(): Promise<BankCardConfig> {
    try {
      const result = await invoke<string>('read_bank_card_config');
      if (result) {
        const parsed = JSON.parse(result);

        // 如果是新格式（包含 cards 数组）
        if (parsed.cards && Array.isArray(parsed.cards) && parsed.cards.length > 0) {
          return {
            ...DEFAULT_BANK_CARD_CONFIG,
            ...parsed.cards[0],
          };
        }

        // 如果是旧格式（单张卡）
        const config = parsed as BankCardConfig;
        return {
          ...DEFAULT_BANK_CARD_CONFIG,
          ...config,
        };
      }
    } catch (error) {
      console.log('读取银行卡配置失败，使用默认配置:', error);
    }
    return DEFAULT_BANK_CARD_CONFIG;
  }

  /**
   * 获取所有银行卡配置（批量注册用）
   */
  static async getBankCardConfigList(): Promise<BankCardConfigList> {
    try {
      const result = await invoke<string>('read_bank_card_config');
      if (result) {
        const parsed = JSON.parse(result);

        // 如果是新格式（包含 cards 数组）
        if (parsed.cards && Array.isArray(parsed.cards)) {
          return parsed as BankCardConfigList;
        }

        // 如果是旧格式（单张卡），转换为数组格式
        return {
          cards: [parsed as BankCardConfig]
        };
      }
    } catch (error) {
      console.log('读取银行卡配置失败，使用默认配置:', error);
    }
    return { cards: [DEFAULT_BANK_CARD_CONFIG] };
  }

  /**
   * 保存银行卡配置（单张卡，兼容旧版本）
   */
  static async saveBankCardConfig(config: BankCardConfig): Promise<{ success: boolean; message: string }> {
    try {
      // 先读取现有配置
      const existing = await this.getBankCardConfigList();

      // 更新第一张卡或添加新卡
      if (existing.cards.length > 0) {
        existing.cards[0] = config;
      } else {
        existing.cards = [config];
      }

      const configJson = JSON.stringify(existing, null, 2);
      await invoke('save_bank_card_config', { config: configJson });
      return { success: true, message: '银行卡配置保存成功' };
    } catch (error) {
      console.error('保存银行卡配置失败:', error);
      return { success: false, message: `保存失败: ${error}` };
    }
  }

  /**
   * 保存所有银行卡配置（批量注册用）
   */
  static async saveBankCardConfigList(configList: BankCardConfigList): Promise<{ success: boolean; message: string }> {
    try {
      const configJson = JSON.stringify(configList, null, 2);
      await invoke('save_bank_card_config', { config: configJson });
      return { success: true, message: '银行卡配置保存成功' };
    } catch (error) {
      console.error('保存银行卡配置失败:', error);
      return { success: false, message: `保存失败: ${error}` };
    }
  }

  /**
   * 验证银行卡配置
   */
  static validateBankCardConfig(config: BankCardConfig): { isValid: boolean; errors: string[] } {
    const errors: string[] = [];

    if (!config.cardNumber || config.cardNumber === '--' || config.cardNumber.length < 13) {
      errors.push('银行卡号至少需要13位数字');
    }

    if (!config.cardExpiry || config.cardExpiry === '--' || !/^\d{2}\/\d{2}$/.test(config.cardExpiry)) {
      errors.push('有效期格式应为 MM/YY');
    }

    if (!config.cardCvc || config.cardCvc === '--' || config.cardCvc.length < 3) {
      errors.push('CVC码至少需要3位数字');
    }

    if (!config.billingName.trim() || config.billingName === '--') {
      errors.push('持卡人姓名不能为空');
    }

    // 只有选择中国时才验证这些字段
    if (config.billingCountry === 'China') {
      if (!config.billingPostalCode.trim() || config.billingPostalCode === '--') {
        errors.push('邮政编码不能为空');
      }

      if (!config.billingLocality.trim() || config.billingLocality === '--') {
        errors.push('城市不能为空');
      }

      if (!config.billingDependentLocality.trim() || config.billingDependentLocality === '--') {
        errors.push('区县不能为空');
      }
    }

    if (!config.billingAddressLine1.trim() || config.billingAddressLine1 === '--') {
      errors.push('详细地址不能为空');
    }

    return {
      isValid: errors.length === 0,
      errors,
    };
  }
}
