import React, { createContext, useContext, useReducer, ReactNode } from "react";
import type { AggregatedUsageData } from "../types/usage";

// State 类型定义
interface UsageState {
  usageData: Record<string, AggregatedUsageData>; // key: token, value: usage data
  loading: Record<string, boolean>; // key: token, value: loading state
  error: Record<string, string | null>; // key: token, value: error message
  lastFetch: Record<string, number>; // key: token, value: timestamp
}

// Action 类型定义
type UsageAction =
  | { type: "SET_LOADING"; payload: { token: string; loading: boolean } }
  | {
      type: "SET_USAGE_DATA";
      payload: { token: string; data: AggregatedUsageData };
    }
  | { type: "SET_ERROR"; payload: { token: string; error: string | null } }
  | { type: "CLEAR_TOKEN_DATA"; payload: { token: string } }
  | { type: "CLEAR_ALL_DATA" };

// 初始状态
const initialState: UsageState = {
  usageData: {},
  loading: {},
  error: {},
  lastFetch: {},
};

// Reducer 函数
const usageReducer = (state: UsageState, action: UsageAction): UsageState => {
  switch (action.type) {
    case "SET_LOADING":
      return {
        ...state,
        loading: {
          ...state.loading,
          [action.payload.token]: action.payload.loading,
        },
        error: {
          ...state.error,
          [action.payload.token]: null, // 清除错误状态
        },
      };

    case "SET_USAGE_DATA":
      return {
        ...state,
        usageData: {
          ...state.usageData,
          [action.payload.token]: action.payload.data,
        },
        loading: {
          ...state.loading,
          [action.payload.token]: false,
        },
        error: {
          ...state.error,
          [action.payload.token]: null,
        },
        lastFetch: {
          ...state.lastFetch,
          [action.payload.token]: Date.now(),
        },
      };

    case "SET_ERROR":
      return {
        ...state,
        loading: {
          ...state.loading,
          [action.payload.token]: false,
        },
        error: {
          ...state.error,
          [action.payload.token]: action.payload.error,
        },
      };

    case "CLEAR_TOKEN_DATA":
      const newState = { ...state };
      delete newState.usageData[action.payload.token];
      delete newState.loading[action.payload.token];
      delete newState.error[action.payload.token];
      delete newState.lastFetch[action.payload.token];
      return newState;

    case "CLEAR_ALL_DATA":
      return initialState;

    default:
      return state;
  }
};

// Context 类型定义
interface UsageContextType {
  state: UsageState;
  dispatch: React.Dispatch<UsageAction>;
  getUsageData: (
    token: string,
    startDate?: number,
    endDate?: number,
    teamId?: number,
    forceRefresh?: boolean
  ) => Promise<void>;
  shouldRefresh: (token: string, maxAge?: number) => boolean;
}

// 创建 Context
const UsageContext = createContext<UsageContextType | undefined>(undefined);

// Provider 组件
interface UsageProviderProps {
  children: ReactNode;
}

export const UsageProvider: React.FC<UsageProviderProps> = ({ children }) => {
  const [state, dispatch] = useReducer(usageReducer, initialState);

  // 获取用量数据的函数
  const getUsageData = async (
    token: string,
    startDate?: number,
    endDate?: number,
    teamId: number = -1,
    forceRefresh: boolean = false
  ) => {
    // 检查是否应该刷新数据（除非强制刷新）
    if (!forceRefresh && !shouldRefresh(token)) {
      console.log("🔄 使用缓存的用量数据");
      return;
    }

    dispatch({ type: "SET_LOADING", payload: { token, loading: true } });

    try {
      console.log("🔄 从API获取用量数据...");

      // 动态导入服务，避免循环依赖
      const { UsageService } = await import("../services/usageService");

      // 如果没有提供时间范围，使用默认的最近30天
      const endTime = endDate || Date.now();
      const startTime = startDate || endTime - 30 * 24 * 60 * 60 * 1000;

      const result = await UsageService.getUsageForPeriod(
        token,
        startTime,
        endTime,
        teamId
      );

      if (result.success && result.data) {
        dispatch({
          type: "SET_USAGE_DATA",
          payload: { token, data: result.data },
        });
      } else {
        dispatch({
          type: "SET_ERROR",
          payload: { token, error: result.message },
        });
      }
    } catch (error) {
      const errorMessage =
        error instanceof Error ? error.message : "获取用量数据失败";
      dispatch({
        type: "SET_ERROR",
        payload: { token, error: errorMessage },
      });
    }
  };

  // 检查是否应该刷新数据（缓存策略）
  const shouldRefresh = (
    token: string,
    maxAge: number = 5 * 60 * 1000
  ): boolean => {
    const lastFetch = state.lastFetch[token];
    const hasData = !!state.usageData[token];
    const isLoading = state.loading[token];

    // 如果正在加载，不需要刷新
    if (isLoading) return false;

    // 如果没有数据，需要加载
    if (!hasData || !lastFetch) return true;

    // 如果数据过期（默认5分钟），需要刷新
    const now = Date.now();
    return now - lastFetch > maxAge;
  };

  const contextValue: UsageContextType = {
    state,
    dispatch,
    getUsageData,
    shouldRefresh,
  };

  return (
    <UsageContext.Provider value={contextValue}>
      {children}
    </UsageContext.Provider>
  );
};

// Hook 用于使用 Context
export const useUsage = (): UsageContextType => {
  const context = useContext(UsageContext);
  if (!context) {
    throw new Error("useUsage must be used within a UsageProvider");
  }
  return context;
};

// 选择器 Hook，用于获取特定 token 的状态
export const useUsageByToken = (token: string) => {
  const { state, getUsageData, shouldRefresh } = useUsage();

  return {
    usageData: state.usageData[token] || null,
    loading: state.loading[token] || false,
    error: state.error[token] || null,
    lastFetch: state.lastFetch[token] || null,
    fetchUsageData: (
      startDate?: number,
      endDate?: number,
      teamId?: number,
      forceRefresh?: boolean
    ) => getUsageData(token, startDate, endDate, teamId, forceRefresh),
    shouldRefresh: (maxAge?: number) => shouldRefresh(token, maxAge),
  };
};
