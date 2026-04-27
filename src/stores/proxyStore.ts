import { create } from 'zustand';
import { persist } from 'zustand/middleware';

export type ProxyType = 'http' | 'socks' | 'vless';

export interface ProxyConfig {
  enabled: boolean;
  proxy_type: ProxyType;
  http_proxy: string;
  socks_proxy: string;
  vless_url: string;
  xray_http_port: number;
  xray_socks_port: number;
  no_proxy: string;
}

interface ProxyStore {
  // 状态
  enabled: boolean;
  proxyType: ProxyType;
  httpProxy: string;
  socksProxy: string;
  vlessUrl: string;
  xrayHttpPort: number;
  xraySocksPort: number;
  noProxy: string;

  // 操作
  setEnabled: (enabled: boolean) => void;
  setProxyType: (type: ProxyType) => void;
  setHttpProxy: (proxy: string) => void;
  setSocksProxy: (proxy: string) => void;
  setVlessUrl: (url: string) => void;
  setXrayHttpPort: (port: number) => void;
  setXraySocksPort: (port: number) => void;
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
  vlessUrl: '',
  xrayHttpPort: 8991,
  xraySocksPort: 1990,
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
      vlessUrl: defaultConfig.vlessUrl,
      xrayHttpPort: defaultConfig.xrayHttpPort,
      xraySocksPort: defaultConfig.xraySocksPort,
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

      // 设置 VLESS URL
      setVlessUrl: (vlessUrl: string) => {
        set({ vlessUrl });
      },

      // 设置 Xray HTTP 端口
      setXrayHttpPort: (xrayHttpPort: number) => {
        set({ xrayHttpPort });
      },

      // 设置 Xray SOCKS 端口
      setXraySocksPort: (xraySocksPort: number) => {
        set({ xraySocksPort });
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
          vlessUrl: config.vless_url ?? state.vlessUrl,
          xrayHttpPort: config.xray_http_port ?? state.xrayHttpPort,
          xraySocksPort: config.xray_socks_port ?? state.xraySocksPort,
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
          vlessUrl: defaultConfig.vlessUrl,
          xrayHttpPort: defaultConfig.xrayHttpPort,
          xraySocksPort: defaultConfig.xraySocksPort,
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
          vless_url: state.proxyType === 'vless' ? state.vlessUrl : '',
          xray_http_port: state.xrayHttpPort,
          xray_socks_port: state.xraySocksPort,
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
        vlessUrl: state.vlessUrl,
        xrayHttpPort: state.xrayHttpPort,
        xraySocksPort: state.xraySocksPort,
        noProxy: state.noProxy,
      }),
    }
  )
);
