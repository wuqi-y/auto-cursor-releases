import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import type { AccountInfo, AccountListResult } from '../types/account';

interface AccountStore {
  // 状态
  accountData: AccountListResult | null;
  loading: boolean;
  lastUpdated: number | null;

  // 缓存设置
  cacheTimeout: number; // 缓存超时时间（毫秒）

  // 操作
  setAccountData: (data: AccountListResult) => void;
  setLoading: (loading: boolean) => void;
  clearCache: () => void;
  isCacheValid: () => boolean;

  // 精准更新操作
  switchCurrentAccount: (newCurrentEmail: string) => void;
  removeAccountByEmail: (email: string) => void;

  // 分组相关
  getGroupedAccounts: () => Record<string, AccountInfo[]>;
  getAvailableGroups: () => string[];
  getFreeAccountSubgroups: () => Record<string, AccountInfo[]>;
}

export const useAccountStore = create<AccountStore>()(
  persist(
    (set, get) => ({
      // 初始状态
      accountData: null,
      loading: false,
      lastUpdated: null,
      cacheTimeout: 5 * 60 * 1000, // 5分钟缓存

      // 设置账户数据
      setAccountData: (data: AccountListResult) => {
        set({
          accountData: data,
          lastUpdated: Date.now(),
          loading: false,
        });
      },

      // 设置加载状态
      setLoading: (loading: boolean) => {
        set({ loading });
      },

      // 清除缓存
      clearCache: () => {
        set({
          accountData: null,
          lastUpdated: null,
          loading: false,
        });
      },

      // 检查缓存是否有效
      isCacheValid: () => {
        const { lastUpdated, cacheTimeout } = get();
        if (!lastUpdated) return false;
        return Date.now() - lastUpdated < cacheTimeout;
      },

      // 精准更新：切换当前账户
      switchCurrentAccount: (newCurrentEmail: string) => {
        const { accountData } = get();
        if (!accountData?.accounts) return;

        // 更新所有账户的 is_current 标记
        const updatedAccounts = accountData.accounts.map(acc => ({
          ...acc,
          is_current: acc.email === newCurrentEmail
        }));

        // 更新 current_account
        const newCurrent = updatedAccounts.find(acc => acc.email === newCurrentEmail) || null;

        set({
          accountData: {
            ...accountData,
            accounts: updatedAccounts,
            current_account: newCurrent
          }
        });
      },

      // 精准更新：删除账户
      removeAccountByEmail: (email: string) => {
        const { accountData } = get();
        if (!accountData?.accounts) return;

        // 从数组中移除指定账户
        const updatedAccounts = accountData.accounts.filter(acc => acc.email !== email);

        // 如果删除的是当前账户，清除 current_account
        const newCurrent = accountData.current_account?.email === email
          ? null
          : accountData.current_account;

        set({
          accountData: {
            ...accountData,
            accounts: updatedAccounts,
            current_account: newCurrent
          }
        });
      },

      // 获取分组后的账户
      getGroupedAccounts: () => {
        const { accountData } = get();
        if (!accountData?.accounts) return {};

        const grouped: Record<string, AccountInfo[]> = {};

        accountData.accounts.forEach((account: AccountInfo) => {
          // 添加空值检查，防止 null 或 undefined 的账户对象
          if (!account) return;

          let groupKey = '未知';

          // 根据授权状态和订阅类型分组
          if (account.auth_status === 'unauthorized') {
            groupKey = '未授权';
          } else if (account.auth_status === 'error' || account.auth_status === 'network_error') {
            groupKey = '获取失败';
          } else if (account.subscription_type) {
            // 标准化订阅类型名称
            const subType = account.subscription_type.toLowerCase();
            if (subType.includes('pro')) {
              groupKey = 'Pro';
            } else if (subType.includes('business')) {
              groupKey = 'Business';
            } else if (subType.includes('trial')) {
              groupKey = '试用版';
            } else if (subType.includes('free')) {
              groupKey = '免费版';
            } else {
              groupKey = account.subscription_type;
            }
          } else if (account.auth_status === undefined || account.subscription_type === undefined) {
            groupKey = '加载中';
          }

          if (!grouped[groupKey]) {
            grouped[groupKey] = [];
          }
          grouped[groupKey].push(account);
        });

        return grouped;
      },

      // 获取可用的分组
      getAvailableGroups: () => {
        const grouped = get().getGroupedAccounts();
        return Object.keys(grouped).sort((a, b) => {
          // 排序优先级：Pro > Business > 试用版 > 免费版 > 其他 > 未授权 > 获取失败 > 加载中
          const priority: Record<string, number> = {
            'Pro': 1,
            'Business': 2,
            '试用版': 3,
            '免费版': 4,
            '未授权': 97,
            '获取失败': 98,
            '加载中': 99,
          };

          const aPriority = priority[a] || 50;
          const bPriority = priority[b] || 50;

          if (aPriority !== bPriority) {
            return aPriority - bPriority;
          }

          return a.localeCompare(b);
        });
      },

      // 获取免费版账户的子分组
      getFreeAccountSubgroups: () => {
        const { accountData } = get();
        if (!accountData?.accounts) return {};

        // 只获取免费版账户
        const freeAccounts = accountData.accounts.filter((account: AccountInfo) => {
          // 添加空值检查
          if (!account) return false;

          if (account.auth_status === 'unauthorized' ||
            account.auth_status === 'error' ||
            account.auth_status === 'network_error') {
            return false;
          }

          if (account.subscription_type) {
            const subType = account.subscription_type.toLowerCase();
            return subType.includes('free');
          }

          return false;
        });

        const subgroups: Record<string, AccountInfo[]> = {};

        freeAccounts.forEach((account: AccountInfo) => {
          // 添加空值检查（双重保险）
          if (!account) return;

          let subgroupKey = '未知状态';

          // 根据订阅状态进行子分组
          if (!account.subscription_status || account.subscription_status === '') {
            subgroupKey = '未绑卡';
          } else if (account.subscription_status === 'canceled') {
            subgroupKey = '已取消';
          } else if (account.subscription_status === 'unpaid') {
            subgroupKey = '未付费';
          } else if (account.subscription_status === 'active') {
            subgroupKey = '活跃';
          } else if (account.subscription_status === 'past_due') {
            subgroupKey = '逾期';
          } else if (account.subscription_status === 'incomplete') {
            subgroupKey = '未完成';
          } else {
            subgroupKey = account.subscription_status;
          }

          if (!subgroups[subgroupKey]) {
            subgroups[subgroupKey] = [];
          }
          subgroups[subgroupKey].push(account);
        });

        return subgroups;
      },
    }),
    {
      name: 'account-store',
      // 只持久化账户数据和最后更新时间，不持久化loading状态
      partialize: (state) => ({
        accountData: state.accountData,
        lastUpdated: state.lastUpdated,
        cacheTimeout: state.cacheTimeout,
      }),
    }
  )
);
