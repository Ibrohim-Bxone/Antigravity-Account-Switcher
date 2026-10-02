export type EngineStatus = "ready" | "busy" | "error" | "offline";

export type TokenStatus =
  | "valid"
  | "expiring"
  | "expired"
  | "refreshing"
  | "unknown";



export type OperationStatus =
  | "awaiting_confirmation"
  | "in_progress"
  | "completed"
  | "failed"
  | "cancelled";

export interface QuotaBucketSummary {
  bucket_id: string;
  window: string;
  remaining_fraction: number;
  reset_time?: string | null;
  display_name: string;
  description?: string | null;
}

export interface QuotaGroupSummary {
  display_name: string;
  description: string;
  buckets: QuotaBucketSummary[];
}

export interface ProfileQuotaSummary {
  subscription_tier: string;
  quota_groups: QuotaGroupSummary[];
}

export interface ProfileSummary {
  profile_id: string;
  display_name: string;
  account_email?: string | null;
  created_at?: string | null;
  last_activated_at?: string | null;
  token_expiry?: string | null;
  token_status: TokenStatus;
  has_refresh_token?: boolean;
  quota?: ProfileQuotaSummary | null;
}



export interface SwitchOperation {
  operation_id: string;
  from_profile_id?: string | null;
  to_profile_id: string;
  current_step: number;
  status: OperationStatus;
  message?: string | null;
  error?: string | null;
  editor_was_running?: boolean;
}

export interface RecoveryState {
  required: boolean;
  operation_id?: string | null;
  current_step: number;
  from_profile_id?: string | null;
  to_profile_id?: string | null;
  reason?: string | null;
  can_resume: boolean;
  can_rollback: boolean;
}

export interface AppSettings {
  http_port: number;
  antigravity_path: string;
  smart_switch_enabled: boolean;
  switch_level: number;
  patch_cooldown_ms: number;
  sqlite_db_path?: string;
  data_dir?: string;
  logs_file?: string;
  minimize_to_tray: boolean;
}



export interface VertexAiBalance {
  enabled: boolean;
  project_id: string;
  billing_account_id?: string;
  initial_credit: number;
  remaining_credit: number;
  currency: string;
  last_updated?: string;
}

export interface ExternalAccountQuota {
  id: string;
  provider: "claude" | "chatgpt";
  name: string;
  email?: string;
  plan: string; // "Pro", "Plus", "Team"
  window_label?: string;
  // 5-Hour Limit
  five_hour_percentage: number;
  five_hour_reset?: string;
  five_hour_target_time?: string; // e.g. "19:11" or "7:11 PM"
  last_calculated_at?: number;
  auto_recover?: boolean;
  // Weekly Limit
  weekly_percentage: number;
  weekly_reset?: string;
  // Cloud session credits (Cursor / Claude)
  cloud_credits_remaining?: number;
  cloud_credits_total?: number;
  cloud_credits_expiry?: string;
  context_window?: string;
  model?: string;
  percentage_mode?: "used" | "remaining";
  // Legacy / fallback fields
  remaining_percentage?: number;
  remaining_messages?: number;
  total_messages?: number;
  reset_time?: string;
  status: "active" | "low" | "exhausted" | "paused";
  source?: string;
}

export interface AppState {
  profiles: ProfileSummary[];
  active_profile_id: string | null;
  engine_status: EngineStatus;
  editor_running: boolean;
  operation: SwitchOperation | null;
  recovery: RecoveryState | null;
  settings: AppSettings;
  isAppLocked: boolean;
  hasMasterPassword: boolean;

  app_version?: string | null;
  antigravity_version?: string | null;
  last_error?: string | null;
  vertex_ai?: VertexAiBalance;
  external_accounts?: ExternalAccountQuota[];
}


export interface AddProfileInput {
  display_name: string;
  account_email?: string;
}

export type DemoScenario =
  | "dashboard"
  | "empty"
  | "recovery"
  | "progress"
  | "error";

export interface CommandError {
  message: string;
  command?: string;
  details?: unknown;
}
