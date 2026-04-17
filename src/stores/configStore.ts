import { create } from 'zustand';
import { persist } from 'zustand/middleware';

// 缓存项接口
interface CacheItem<T> {
  value: T;
  timestamp: number;
  expiresIn?: number; // 过期时间（毫秒），undefined表示永不过期
}

// 排序方式类型
export type SortOrder = 'asc' | 'desc';

// 排序字段类型
export type SortField = 'created_at' | 'trial_days_remaining';

// 邮箱类型
export type EmailType =
  | 'custom'
  | 'cloudflare_temp'
  | 'outlook'
  | 'tempmail'
  | 'self_hosted';

export type RegistrationProvider = 'cursor' | 'codex';
export type ThemeMode = 'light' | 'dark';

// 配置数据接口
export interface ConfigData {
  // 邮箱相关配置
  pinCode?: CacheItem<string>;
  tempmailEmail?: CacheItem<string>;

  // 虚拟卡片相关配置
  customPrefix?: CacheItem<string>;

  // 账户列表排序方式
  accountSortOrder?: CacheItem<SortOrder>;

  // 账户列表排序字段
  accountSortField?: CacheItem<SortField>;

  // 注册页面配置
  emailType?: CacheItem<EmailType>;
  useIncognito?: CacheItem<boolean>;
  enableBankCardBinding?: CacheItem<boolean>;
  useParallelMode?: CacheItem<boolean>;

  /** 自建邮箱 API：拉取邮件列表的 URL */
  selfHostedMailUrl?: CacheItem<string>;
  /** 自建邮箱 API：请求头 JSON 对象字符串 */
  selfHostedMailHeadersJson?: CacheItem<string>;
  /** 自建邮箱 API：响应体中邮件原文的路径，如 results[0].raw */
  selfHostedMailResponsePath?: CacheItem<string>;

  /** Codex 自建邮箱 API：拉取邮件列表的 URL */
  codexSelfHostedMailUrl?: CacheItem<string>;
  /** Codex 自建邮箱 API：请求头 JSON 对象字符串 */
  codexSelfHostedMailHeadersJson?: CacheItem<string>;
  /** 自建邮箱 API：获取验证码前是否先清空邮箱 */
  selfHostedMailClearEnabled?: CacheItem<boolean>;
  /** 自建邮箱 API：清空邮箱请求 URL */
  selfHostedMailClearUrl?: CacheItem<string>;
  /** 自建邮箱 API：清空邮箱请求头 JSON */
  selfHostedMailClearHeadersJson?: CacheItem<string>;
  /** 自建邮箱 API：清空邮箱请求 Method */
  selfHostedMailClearMethod?: CacheItem<string>;

  /** Codex 自建邮箱 API：响应体中邮件原文的路径，如 results[0].raw */
  codexSelfHostedMailResponsePath?: CacheItem<string>;
  /** Codex 自建邮箱 API：获取验证码前是否先清空邮箱 */
  codexSelfHostedMailClearEnabled?: CacheItem<boolean>;
  /** Codex 自建邮箱 API：清空邮箱请求 URL */
  codexSelfHostedMailClearUrl?: CacheItem<string>;
  /** Codex 自建邮箱 API：清空邮箱请求头 JSON */
  codexSelfHostedMailClearHeadersJson?: CacheItem<string>;
  /** Codex 自建邮箱 API：清空邮箱请求 Method */
  codexSelfHostedMailClearMethod?: CacheItem<string>;
  /** Codex 注册时的 CDP 流程覆盖配置（JSON 字符串） */
  codexCdpOverridesJson?: CacheItem<string>;

  registrationProvider?: CacheItem<RegistrationProvider>;
  themeMode?: CacheItem<ThemeMode>;

  // 可以扩展更多配置项
  // 例如：
  // lastUsedProxyConfig?: CacheItem<ProxyConfig>;
  // userPreferences?: CacheItem<UserPreferences>;
}

// Store接口
interface ConfigStore {
  // 状态
  configData: ConfigData;

  // 通用缓存操作
  setCache: <T>(key: keyof ConfigData, value: T, expiresIn?: number) => void;
  getCache: <T>(key: keyof ConfigData) => T | null;
  removeCache: (key: keyof ConfigData) => void;
  clearExpiredCache: () => void;
  clearAllCache: () => void;
  isCacheValid: (key: keyof ConfigData) => boolean;

  // 专用方法 - Pin码
  setPinCode: (pinCode: string, expiresIn?: number) => void;
  getPinCode: () => string | null;
  removePinCode: () => void;

  // 专用方法 - 临时邮箱
  setTempmailEmail: (email: string, expiresIn?: number) => void;
  getTempmailEmail: () => string | null;
  removeTempmailEmail: () => void;

  // 专用方法 - 自定义卡头
  setCustomPrefix: (prefix: string, expiresIn?: number) => void;
  getCustomPrefix: () => string | null;
  removeCustomPrefix: () => void;

  // 专用方法 - 账户排序
  setAccountSortOrder: (sortOrder: SortOrder) => void;
  getAccountSortOrder: () => SortOrder | null;
  toggleAccountSortOrder: () => SortOrder;

  // 专用方法 - 账户排序字段
  setAccountSortField: (sortField: SortField) => void;
  getAccountSortField: () => SortField | null;
  toggleAccountSortField: () => SortField;

  // 专用方法 - 注册页面配置
  setEmailType: (emailType: EmailType, expiresIn?: number) => void;
  getEmailType: () => EmailType | null;
  removeEmailType: () => void;

  setUseIncognito: (useIncognito: boolean, expiresIn?: number) => void;
  getUseIncognito: () => boolean | null;
  removeUseIncognito: () => void;

  setEnableBankCardBinding: (enableBankCardBinding: boolean, expiresIn?: number) => void;
  getEnableBankCardBinding: () => boolean | null;
  removeEnableBankCardBinding: () => void;

  setUseParallelMode: (useParallelMode: boolean, expiresIn?: number) => void;
  getUseParallelMode: () => boolean | null;
  removeUseParallelMode: () => void;

  setSelfHostedMailUrl: (url: string, expiresIn?: number) => void;
  getSelfHostedMailUrl: () => string | null;
  setSelfHostedMailHeadersJson: (json: string, expiresIn?: number) => void;
  getSelfHostedMailHeadersJson: () => string | null;
  setSelfHostedMailResponsePath: (path: string, expiresIn?: number) => void;
  getSelfHostedMailResponsePath: () => string | null;
  setSelfHostedMailClearEnabled: (enabled: boolean, expiresIn?: number) => void;
  getSelfHostedMailClearEnabled: () => boolean | null;
  setSelfHostedMailClearUrl: (url: string, expiresIn?: number) => void;
  getSelfHostedMailClearUrl: () => string | null;
  setSelfHostedMailClearHeadersJson: (json: string, expiresIn?: number) => void;
  getSelfHostedMailClearHeadersJson: () => string | null;
  setSelfHostedMailClearMethod: (method: string, expiresIn?: number) => void;
  getSelfHostedMailClearMethod: () => string | null;

  setCodexSelfHostedMailUrl: (url: string, expiresIn?: number) => void;
  getCodexSelfHostedMailUrl: () => string | null;
  setCodexSelfHostedMailHeadersJson: (json: string, expiresIn?: number) => void;
  getCodexSelfHostedMailHeadersJson: () => string | null;
  setCodexSelfHostedMailResponsePath: (path: string, expiresIn?: number) => void;
  getCodexSelfHostedMailResponsePath: () => string | null;
  setCodexSelfHostedMailClearEnabled: (enabled: boolean, expiresIn?: number) => void;
  getCodexSelfHostedMailClearEnabled: () => boolean | null;
  setCodexSelfHostedMailClearUrl: (url: string, expiresIn?: number) => void;
  getCodexSelfHostedMailClearUrl: () => string | null;
  setCodexSelfHostedMailClearHeadersJson: (json: string, expiresIn?: number) => void;
  getCodexSelfHostedMailClearHeadersJson: () => string | null;
  setCodexSelfHostedMailClearMethod: (method: string, expiresIn?: number) => void;
  getCodexSelfHostedMailClearMethod: () => string | null;
  setCodexCdpOverridesJson: (json: string, expiresIn?: number) => void;
  getCodexCdpOverridesJson: () => string | null;

  setRegistrationProvider: (provider: RegistrationProvider, expiresIn?: number) => void;
  getRegistrationProvider: () => RegistrationProvider | null;
  removeRegistrationProvider: () => void;

  setThemeMode: (themeMode: ThemeMode, expiresIn?: number) => void;
  getThemeMode: () => ThemeMode | null;
  removeThemeMode: () => void;

  // 工具方法
  getAllValidCache: () => Record<string, any>;
}

// 默认过期时间配置（毫秒）
const DEFAULT_EXPIRY = {
  pinCode: undefined, // pin码永不过期
  tempmailEmail: undefined, // 临时邮箱永不过期
  customPrefix: undefined, // 自定义卡头永不过期
  accountSortOrder: undefined, // 排序方式永不过期
  accountSortField: undefined, // 排序字段永不过期
  emailType: undefined, // 邮箱类型永不过期
  useIncognito: undefined, // 无痕模式永不过期
  enableBankCardBinding: undefined, // 自动绑定银行卡永不过期
  useParallelMode: undefined, // 并行模式永不过期
  selfHostedMailUrl: undefined,
  selfHostedMailHeadersJson: undefined,
  selfHostedMailResponsePath: undefined,
  selfHostedMailClearEnabled: undefined,
  selfHostedMailClearUrl: undefined,
  selfHostedMailClearHeadersJson: undefined,
  selfHostedMailClearMethod: undefined,
  codexSelfHostedMailUrl: undefined,
  codexSelfHostedMailHeadersJson: undefined,
  codexSelfHostedMailResponsePath: undefined,
  codexSelfHostedMailClearEnabled: undefined,
  codexSelfHostedMailClearUrl: undefined,
  codexSelfHostedMailClearHeadersJson: undefined,
  codexSelfHostedMailClearMethod: undefined,
  codexCdpOverridesJson: undefined,
  registrationProvider: undefined,
  themeMode: undefined,
};

export const useConfigStore = create<ConfigStore>()(
  persist(
    (set, get) => ({
      // 初始状态
      configData: {},

      // 通用设置缓存
      setCache: <T>(key: keyof ConfigData, value: T, expiresIn?: number) => {
        set((state) => ({
          configData: {
            ...state.configData,
            [key]: {
              value,
              timestamp: Date.now(),
              expiresIn,
            } as CacheItem<T>,
          },
        }));
      },

      // 通用获取缓存
      getCache: <T>(key: keyof ConfigData): T | null => {
        const { configData } = get();
        const cacheItem = configData[key] as CacheItem<T> | undefined;

        if (!cacheItem) return null;

        // 检查是否过期
        if (cacheItem.expiresIn) {
          const isExpired = Date.now() - cacheItem.timestamp > cacheItem.expiresIn;
          if (isExpired) {
            get().removeCache(key);
            return null;
          }
        }

        return cacheItem.value;
      },

      // 删除特定缓存
      removeCache: (key: keyof ConfigData) => {
        set((state) => {
          const newConfigData = { ...state.configData };
          delete newConfigData[key];
          return { configData: newConfigData };
        });
      },

      // 清理过期缓存
      clearExpiredCache: () => {
        set((state) => {
          const newConfigData: ConfigData = {};
          const now = Date.now();

          Object.entries(state.configData).forEach(([key, cacheItem]) => {
            if (cacheItem && typeof cacheItem === 'object' && 'timestamp' in cacheItem) {
              // 如果没有过期时间或者没有过期，保留
              if (!cacheItem.expiresIn || (now - cacheItem.timestamp <= cacheItem.expiresIn)) {
                newConfigData[key as keyof ConfigData] = cacheItem;
              }
            }
          });

          return { configData: newConfigData };
        });
      },

      // 清空所有缓存
      clearAllCache: () => {
        set({ configData: {} });
      },

      // 检查缓存是否有效
      isCacheValid: (key: keyof ConfigData): boolean => {
        const cacheItem = get().configData[key];
        if (!cacheItem) return false;

        if (cacheItem.expiresIn) {
          return Date.now() - cacheItem.timestamp <= cacheItem.expiresIn;
        }

        return true;
      },

      // Pin码专用方法
      setPinCode: (pinCode: string, expiresIn = DEFAULT_EXPIRY.pinCode) => {
        get().setCache('pinCode', pinCode, expiresIn);
      },

      getPinCode: () => {
        return get().getCache<string>('pinCode');
      },

      removePinCode: () => {
        get().removeCache('pinCode');
      },

      // 临时邮箱专用方法
      setTempmailEmail: (email: string, expiresIn = DEFAULT_EXPIRY.tempmailEmail) => {
        get().setCache('tempmailEmail', email, expiresIn);
      },

      getTempmailEmail: () => {
        return get().getCache<string>('tempmailEmail');
      },

      removeTempmailEmail: () => {
        get().removeCache('tempmailEmail');
      },

      // 自定义卡头专用方法
      setCustomPrefix: (prefix: string, expiresIn = DEFAULT_EXPIRY.customPrefix) => {
        get().setCache('customPrefix', prefix, expiresIn);
      },

      getCustomPrefix: () => {
        return get().getCache<string>('customPrefix');
      },

      removeCustomPrefix: () => {
        get().removeCache('customPrefix');
      },

      // 账户排序专用方法
      setAccountSortOrder: (sortOrder: SortOrder) => {
        get().setCache('accountSortOrder', sortOrder, DEFAULT_EXPIRY.accountSortOrder);
      },

      getAccountSortOrder: () => {
        return get().getCache<SortOrder>('accountSortOrder');
      },

      toggleAccountSortOrder: (): SortOrder => {
        const currentOrder = get().getAccountSortOrder();
        const newOrder: SortOrder = currentOrder === 'asc' ? 'desc' : 'asc';
        get().setAccountSortOrder(newOrder);
        return newOrder;
      },

      // 账户排序字段专用方法
      setAccountSortField: (sortField: SortField) => {
        get().setCache('accountSortField', sortField, DEFAULT_EXPIRY.accountSortField);
      },

      getAccountSortField: () => {
        return get().getCache<SortField>('accountSortField');
      },

      toggleAccountSortField: (): SortField => {
        const currentField = get().getAccountSortField();
        const newField: SortField = currentField === 'created_at' ? 'trial_days_remaining' : 'created_at';
        get().setAccountSortField(newField);
        return newField;
      },

      // 注册页面配置专用方法
      setEmailType: (emailType: EmailType, expiresIn = DEFAULT_EXPIRY.emailType) => {
        get().setCache('emailType', emailType, expiresIn);
      },

      getEmailType: () => {
        return get().getCache<EmailType>('emailType');
      },

      removeEmailType: () => {
        get().removeCache('emailType');
      },

      setUseIncognito: (useIncognito: boolean, expiresIn = DEFAULT_EXPIRY.useIncognito) => {
        get().setCache('useIncognito', useIncognito, expiresIn);
      },

      getUseIncognito: () => {
        return get().getCache<boolean>('useIncognito');
      },

      removeUseIncognito: () => {
        get().removeCache('useIncognito');
      },

      setEnableBankCardBinding: (enableBankCardBinding: boolean, expiresIn = DEFAULT_EXPIRY.enableBankCardBinding) => {
        get().setCache('enableBankCardBinding', enableBankCardBinding, expiresIn);
      },

      getEnableBankCardBinding: () => {
        return get().getCache<boolean>('enableBankCardBinding');
      },

      removeEnableBankCardBinding: () => {
        get().removeCache('enableBankCardBinding');
      },

      setUseParallelMode: (useParallelMode: boolean, expiresIn = DEFAULT_EXPIRY.useParallelMode) => {
        get().setCache('useParallelMode', useParallelMode, expiresIn);
      },

      getUseParallelMode: () => {
        return get().getCache<boolean>('useParallelMode');
      },

      removeUseParallelMode: () => {
        get().removeCache('useParallelMode');
      },

      setSelfHostedMailUrl: (url: string, expiresIn = DEFAULT_EXPIRY.selfHostedMailUrl) => {
        get().setCache('selfHostedMailUrl', url, expiresIn);
      },
      getSelfHostedMailUrl: () => get().getCache<string>('selfHostedMailUrl'),

      setSelfHostedMailHeadersJson: (
        json: string,
        expiresIn = DEFAULT_EXPIRY.selfHostedMailHeadersJson
      ) => {
        get().setCache('selfHostedMailHeadersJson', json, expiresIn);
      },
      getSelfHostedMailHeadersJson: () =>
        get().getCache<string>('selfHostedMailHeadersJson'),

      setSelfHostedMailResponsePath: (
        path: string,
        expiresIn = DEFAULT_EXPIRY.selfHostedMailResponsePath
      ) => {
        get().setCache('selfHostedMailResponsePath', path, expiresIn);
      },
      getSelfHostedMailResponsePath: () =>
        get().getCache<string>('selfHostedMailResponsePath'),

      setSelfHostedMailClearEnabled: (
        enabled: boolean,
        expiresIn = DEFAULT_EXPIRY.selfHostedMailClearEnabled
      ) => {
        get().setCache('selfHostedMailClearEnabled', enabled, expiresIn);
      },
      getSelfHostedMailClearEnabled: () =>
        get().getCache<boolean>('selfHostedMailClearEnabled'),

      setSelfHostedMailClearUrl: (
        url: string,
        expiresIn = DEFAULT_EXPIRY.selfHostedMailClearUrl
      ) => {
        get().setCache('selfHostedMailClearUrl', url, expiresIn);
      },
      getSelfHostedMailClearUrl: () =>
        get().getCache<string>('selfHostedMailClearUrl'),

      setSelfHostedMailClearHeadersJson: (
        json: string,
        expiresIn = DEFAULT_EXPIRY.selfHostedMailClearHeadersJson
      ) => {
        get().setCache('selfHostedMailClearHeadersJson', json, expiresIn);
      },
      getSelfHostedMailClearHeadersJson: () =>
        get().getCache<string>('selfHostedMailClearHeadersJson'),

      setSelfHostedMailClearMethod: (
        method: string,
        expiresIn = DEFAULT_EXPIRY.selfHostedMailClearMethod
      ) => {
        get().setCache('selfHostedMailClearMethod', method, expiresIn);
      },
      getSelfHostedMailClearMethod: () =>
        get().getCache<string>('selfHostedMailClearMethod'),

      setCodexSelfHostedMailUrl: (
        url: string,
        expiresIn = DEFAULT_EXPIRY.codexSelfHostedMailUrl
      ) => {
        get().setCache('codexSelfHostedMailUrl', url, expiresIn);
      },
      getCodexSelfHostedMailUrl: () =>
        get().getCache<string>('codexSelfHostedMailUrl'),

      setCodexSelfHostedMailHeadersJson: (
        json: string,
        expiresIn = DEFAULT_EXPIRY.codexSelfHostedMailHeadersJson
      ) => {
        get().setCache('codexSelfHostedMailHeadersJson', json, expiresIn);
      },
      getCodexSelfHostedMailHeadersJson: () =>
        get().getCache<string>('codexSelfHostedMailHeadersJson'),

      setCodexSelfHostedMailResponsePath: (
        path: string,
        expiresIn = DEFAULT_EXPIRY.codexSelfHostedMailResponsePath
      ) => {
        get().setCache('codexSelfHostedMailResponsePath', path, expiresIn);
      },
      getCodexSelfHostedMailResponsePath: () =>
        get().getCache<string>('codexSelfHostedMailResponsePath'),

      setCodexSelfHostedMailClearEnabled: (
        enabled: boolean,
        expiresIn = DEFAULT_EXPIRY.codexSelfHostedMailClearEnabled
      ) => {
        get().setCache('codexSelfHostedMailClearEnabled', enabled, expiresIn);
      },
      getCodexSelfHostedMailClearEnabled: () =>
        get().getCache<boolean>('codexSelfHostedMailClearEnabled'),

      setCodexSelfHostedMailClearUrl: (
        url: string,
        expiresIn = DEFAULT_EXPIRY.codexSelfHostedMailClearUrl
      ) => {
        get().setCache('codexSelfHostedMailClearUrl', url, expiresIn);
      },
      getCodexSelfHostedMailClearUrl: () =>
        get().getCache<string>('codexSelfHostedMailClearUrl'),

      setCodexSelfHostedMailClearHeadersJson: (
        json: string,
        expiresIn = DEFAULT_EXPIRY.codexSelfHostedMailClearHeadersJson
      ) => {
        get().setCache('codexSelfHostedMailClearHeadersJson', json, expiresIn);
      },
      getCodexSelfHostedMailClearHeadersJson: () =>
        get().getCache<string>('codexSelfHostedMailClearHeadersJson'),

      setCodexSelfHostedMailClearMethod: (
        method: string,
        expiresIn = DEFAULT_EXPIRY.codexSelfHostedMailClearMethod
      ) => {
        get().setCache('codexSelfHostedMailClearMethod', method, expiresIn);
      },
      getCodexSelfHostedMailClearMethod: () =>
        get().getCache<string>('codexSelfHostedMailClearMethod'),

      setCodexCdpOverridesJson: (
        json: string,
        expiresIn = DEFAULT_EXPIRY.codexCdpOverridesJson
      ) => {
        get().setCache('codexCdpOverridesJson', json, expiresIn);
      },
      getCodexCdpOverridesJson: () =>
        get().getCache<string>('codexCdpOverridesJson'),

      setRegistrationProvider: (
        provider: RegistrationProvider,
        expiresIn = DEFAULT_EXPIRY.registrationProvider
      ) => {
        get().setCache('registrationProvider', provider, expiresIn);
      },
      getRegistrationProvider: () =>
        get().getCache<RegistrationProvider>('registrationProvider'),
      removeRegistrationProvider: () => {
        get().removeCache('registrationProvider');
      },

      setThemeMode: (
        themeMode: ThemeMode,
        expiresIn = DEFAULT_EXPIRY.themeMode
      ) => {
        get().setCache('themeMode', themeMode, expiresIn);
      },
      getThemeMode: () => get().getCache<ThemeMode>('themeMode'),
      removeThemeMode: () => {
        get().removeCache('themeMode');
      },

      // 获取所有有效缓存
      getAllValidCache: () => {
        const { configData } = get();
        const validCache: Record<string, any> = {};

        Object.entries(configData).forEach(([key, cacheItem]) => {
          if (get().isCacheValid(key as keyof ConfigData)) {
            validCache[key] = cacheItem?.value;
          }
        });

        return validCache;
      },
    }),
    {
      name: 'config-store',
      // 持久化配置数据
      partialize: (state) => ({
        configData: state.configData,
      }),
      // 在加载时清理过期缓存
      onRehydrateStorage: () => (state) => {
        if (state) {
          state.clearExpiredCache();
        }
      },
    }
  )
);

// 导出类型以供其他组件使用
export type { CacheItem };
