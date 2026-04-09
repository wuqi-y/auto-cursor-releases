import { invoke } from "@tauri-apps/api/core";
import type { UsageResponse } from "../types/usage";

export class UsageService {
  /**
   * Get aggregated usage data for a specific time period
   * @param token - The access token
   * @param startDate - Start date as Unix timestamp in milliseconds
   * @param endDate - End date as Unix timestamp in milliseconds
   * @param teamId - Team ID (usually -1 for personal accounts)
   * @returns Promise<UsageResponse>
   */
  static async getUsageForPeriod(
    token: string,
    startDate: number,
    endDate: number,
    teamId: number = -1
  ): Promise<UsageResponse> {
    try {
      console.log('🔄 获取用量数据:', {
        tokenLength: token.length,
        startDate: new Date(startDate).toISOString(),
        endDate: new Date(endDate).toISOString(),
        teamId
      });

      const result = await invoke<UsageResponse>("get_usage_for_period", {
        token,
        startDate,
        endDate,
        teamId
      });

      console.log('📥 用量数据响应:', result);
      return result;
    } catch (error) {
      console.error('获取用量数据失败:', error);
      return {
        success: false,
        message: `获取用量数据失败: ${error instanceof Error ? error.message : '未知错误'}`
      };
    }
  }

  /**
   * Get usage data for the last N days
   * @param token - The access token
   * @param days - Number of days to look back (default: 30)
   * @param teamId - Team ID (usually -1 for personal accounts)
   * @returns Promise<UsageResponse>
   */
  static async getUsageForLastDays(
    token: string,
    days: number = 30,
    teamId: number = -1
  ): Promise<UsageResponse> {
    const endDate = Date.now();
    const startDate = endDate - (days * 24 * 60 * 60 * 1000);

    return this.getUsageForPeriod(token, startDate, endDate, teamId);
  }

  /**
   * Get usage data for current month
   * @param token - The access token
   * @param teamId - Team ID (usually -1 for personal accounts)
   * @returns Promise<UsageResponse>
   */
  static async getUsageForCurrentMonth(
    token: string,
    teamId: number = -1
  ): Promise<UsageResponse> {
    const now = new Date();
    const startDate = new Date(now.getFullYear(), now.getMonth(), 1).getTime();
    const endDate = now.getTime();

    return this.getUsageForPeriod(token, startDate, endDate, teamId);
  }
}
