// 用户分析数据类型
export interface UserAnalyticsData {
  dailyMetrics: DailyMetric[];
  period: Period;
  totalMembersInTeam: number;
}

export interface DailyMetric {
  date: string;
  activeUsers?: number;
  acceptedLinesAdded?: number;
  acceptedLinesDeleted?: number;
  totalApplies?: number;
  totalAccepts?: number;
  totalTabsShown?: number;
  totalTabsAccepted?: number;
  composerRequests?: number;
  agentRequests?: number;
  subscriptionIncludedReqs?: number;
  modelUsage?: ModelCount[];
  extensionUsage?: NameCount[];
  tabExtensionUsage?: NameCount[];
  clientVersionUsage?: NameCount[];
}

export interface Period {
  startDate: string;
  endDate: string;
}

export interface ModelCount {
  name: string;
  count: number;
}

export interface NameCount {
  name: string;
  count: number;
}

// 过滤使用事件数据类型
export interface FilteredUsageEventsData {
  totalUsageEventsCount: number;
  usageEventsDisplay: UsageEventDisplay[];
}

export interface UsageEventDisplay {
  timestamp: string;
  model: string;
  kind: string;
  requestsCosts?: number;
  usageBasedCosts: string;
  isTokenBasedCall: boolean;
  tokenUsage?: TokenUsageDetail;
  owningUser: string;
}

export interface TokenUsageDetail {
  inputTokens?: number;  // 可选字段，某些记录可能没有输入tokens
  outputTokens?: number; // 可选字段，某些记录可能没有输出tokens
  cacheWriteTokens?: number; // 可选字段，某些记录可能没有缓存写入tokens
  cacheReadTokens?: number;  // 可选字段，某些记录可能没有缓存读取tokens
  totalCents?: number;   // 可选字段，某些记录可能没有费用信息
}

// API 响应类型
export interface AnalyticsApiResponse<T> {
  success: boolean;
  message: string;
  data?: T;
}
