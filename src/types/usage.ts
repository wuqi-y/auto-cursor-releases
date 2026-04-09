export interface ModelUsage {
  model_intent: string;
  input_tokens: string;
  output_tokens: string;
  cache_write_tokens: string;
  cache_read_tokens: string;
  total_cents: number;
}

export interface AggregatedUsageData {
  aggregations: ModelUsage[];
  total_input_tokens: string;
  total_output_tokens: string;
  total_cache_write_tokens: string;
  total_cache_read_tokens: string;
  total_cost_cents: number;
}

export interface UsageRequest {
  start_date: number;
  end_date: number;
  team_id: number;
}

export interface UsageResponse {
  success: boolean;
  message: string;
  data?: AggregatedUsageData;
}

export interface DateRange {
  startDate: Date;
  endDate: Date;
}
