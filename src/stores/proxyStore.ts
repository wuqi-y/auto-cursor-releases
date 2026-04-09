import { create } from 'zustand';
import { persist } from 'zustand/middleware';

export type ProxyType = 'http' | 'socks';

export interface ProxyConfig {
  enabled: boolean;
  proxy_type: ProxyType;
  http_proxy: string;
  socks_proxy: string;
  no_proxy: string;
}

interface ProxyStore {
  // 状态
  enabled: boolean;
  proxyType: ProxyType;
  httpProxy: string;
  socksProxy: string;
  noProxy: string;

  // 操作
  setEnabled: (enabled: boolean) => void;
  setProxyType: (type: ProxyType) => void;
  setHttpProxy: (proxy: string) => void;
  setSocksProxy: (proxy: string) => void;
  setNoProxy: (noProxy: string) => void;

  // 批量操作
  setProxyConfig: (config: Partial<ProxyConfig>) => void;
  resetToDefaults: () => void;

  // 获取配置对象
  getProxyConfig: () => ProxyConfig;
}

const defaultConfig = {
  enabled: false,
  proxyType: 'http' as ProxyType,
  httpProxy: '127.0.0.1:7890',
  socksProxy: '127.0.0.1:1080',
  noProxy: 'localhost,127.0.0.1',
};

export const useProxyStore = create<ProxyStore>()(
  persist(
    (set, get) => ({
      // 初始状态
      enabled: defaultConfig.enabled,
      proxyType: defaultConfig.proxyType,
      httpProxy: defaultConfig.httpProxy,
      socksProxy: defaultConfig.socksProxy,
      noProxy: defaultConfig.noProxy,

      // 设置启用状态
      setEnabled: (enabled: boolean) => {
        set({ enabled });
      },

      // 设置代理类型
      setProxyType: (proxyType: ProxyType) => {
        set({ proxyType });
      },

      // 设置 HTTP 代理
      setHttpProxy: (httpProxy: string) => {
        set({ httpProxy });
      },

      // 设置 SOCKS 代理
      setSocksProxy: (socksProxy: string) => {
        set({ socksProxy });
      },

      // 设置代理绕过列表
      setNoProxy: (noProxy: string) => {
        set({ noProxy });
      },

      // 批量设置配置
      setProxyConfig: (config: Partial<ProxyConfig>) => {
        set((state) => ({
          ...state,
          enabled: config.enabled ?? state.enabled,
          proxyType: config.proxy_type ?? state.proxyType,
          httpProxy: config.http_proxy ?? state.httpProxy,
          socksProxy: config.socks_proxy ?? state.socksProxy,
          noProxy: config.no_proxy ?? state.noProxy,
        }));
      },

      // 重置为默认配置
      resetToDefaults: () => {
        set({
          enabled: defaultConfig.enabled,
          proxyType: defaultConfig.proxyType,
          httpProxy: defaultConfig.httpProxy,
          socksProxy: defaultConfig.socksProxy,
          noProxy: defaultConfig.noProxy,
        });
      },

      // 获取配置对象
      getProxyConfig: (): ProxyConfig => {
        const state = get();
        return {
          enabled: state.enabled,
          proxy_type: state.proxyType,
          http_proxy: state.proxyType === 'http' ? state.httpProxy : '',
          socks_proxy: state.proxyType === 'socks' ? state.socksProxy : '',
          no_proxy: state.noProxy,
        };
      },
    }),
    {
      name: 'proxy-store',
      // 持久化所有状态
      partialize: (state) => ({
        enabled: state.enabled,
        proxyType: state.proxyType,
        httpProxy: state.httpProxy,
        socksProxy: state.socksProxy,
        noProxy: state.noProxy,
      }),
    }
  )
);
