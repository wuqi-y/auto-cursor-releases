export interface UserAuthInfo {
  is_authorized: boolean;
  token_length: number;
  token_valid: boolean;
  api_status?: number;
  error_message?: string;
  checksum?: string;
  account_info?: AccountInfo;
}

export interface AccountInfo {
  email?: string;
  username?: string;
  subscription_type?: string;
  subscription_status?: string;
  trial_days_remaining?: number;
  usage_info?: string;
  aggregated_usage?: AggregatedUsageData;
}

export interface AggregatedUsageData {
  aggregations: ModelUsage[];
  total_input_tokens: string;
  total_output_tokens: string;
  total_cache_write_tokens: string;
  total_cache_read_tokens: string;
  total_cost_cents: number;
}

export interface ModelUsage {
  model_intent: string;
  input_tokens: string;
  output_tokens: string;
  cache_write_tokens: string;
  cache_read_tokens: string;
  total_cents: number;
}

export interface AuthCheckResult {
  success: boolean;
  message: string;
  details: string[];
  user_info?: UserAuthInfo;
}

export interface BackupInfo {
  path: string;
  filename: string;
  timestamp: string;
  size: number;
  date_formatted: string;
}

export interface MachineIds {
  "telemetry.devDeviceId": string;
  "telemetry.macMachineId": string;
  "telemetry.machineId": string;
  "telemetry.sqmId": string;
  "storage.serviceMachineId": string;
}

export interface RestoreResult {
  success: boolean;
  message: string;
  details: string[];
}

export interface ResetResult {
  success: boolean;
  message: string;
  details: string[];
  new_ids?: MachineIds;
}

export interface TokenInfo {
  token?: string;
  source: string;
  found: boolean;
  message: string;
}
