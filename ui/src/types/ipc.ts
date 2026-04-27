export type ConnectionState =
  | 'idle'
  | 'validating_profile'
  | 'awaiting_credentials'
  | 'preparing_runtime'
  | 'starting_process'
  | 'connecting'
  | 'connected'
  | 'reconnecting'
  | 'disconnecting'
  | 'error'

export type ValidationStatus = 'Ok' | 'Warning' | 'Blocked'

export interface ProfileSummary {
  id: string
  name: string
  remote_summary: string
  has_saved_credentials: boolean
  validation_status: ValidationStatus
  last_used_at: string | null
}

export interface Profile {
  id: string
  name: string
  source_filename: string
  managed_dir: string
  managed_ovpn_path: string
  original_import_path: string
  dns_intent: string[]
  dns_policy: 'SplitDnsPreferred' | 'FullOverride' | 'ObserveOnly'
  credential_mode: 'None' | 'UserPass'
  credential_strategy: 'Prompt' | 'PinTotp'
  remote_summary: string
  has_saved_credentials: boolean
  validation_status: ValidationStatus
}

export interface ValidationFinding {
  severity: 'Info' | 'Warn' | 'Error'
  directive: string
  line: number
  message: string
  action: 'Allow' | 'RequireApproval' | 'Block'
}

export interface ManagedAsset {
  id: string
  relative_path: string
  kind: string
  sha256: string
  origin: string
}

export interface ProfileDetail {
  profile: Profile
  assets: ManagedAsset[]
  findings: ValidationFinding[]
  saved_username?: string | null
  has_saved_pin_totp?: boolean
}

export interface ImportReport {
  status: 'Imported' | 'NeedsApproval' | 'Blocked'
  copied_assets: string[]
  rewritten_paths: string[]
  warnings: ValidationFinding[]
  blocked_directives: ValidationFinding[]
  missing_files: string[]
  errors: string[]
}

export interface ImportProfileResponse {
  profile: ProfileDetail | null
  report: ImportReport
}

export interface DnsObservation {
  config_requested: string[]
  runtime_pushed: string[]
  effective_mode: 'ObserveOnly' | 'ScopedResolvers' | 'GlobalOverride'
  auto_promoted_policy: 'SplitDnsPreferred' | 'FullOverride' | 'ObserveOnly' | null
  restore_status: 'ok' | 'pending_reconcile' | 'restore_failed' | null
  warnings: string[]
}

export interface UserFacingError {
  code: string
  title: string
  message: string
  suggested_fix: string | null
  details_safe: string | null
}

export interface ConnectionSnapshot {
  profile_id: string | null
  state: ConnectionState
  substate: string | null
  started_at: string | null
  pid: number | null
  retry_count: number
  dns_observation: DnsObservation
  log_file_path: string | null
  last_error: UserFacingError | null
}

export interface CredentialPrompt {
  profile_id: string
  remember_supported: boolean
  saved_username: string | null
}

export interface LogEntry {
  ts: string
  stream: string
  level: 'Debug' | 'Info' | 'Warn' | 'Error'
  message: string
  sanitized: boolean
  classification: string
}

export interface Settings {
  openvpn_path_override: string | null
  verbose_logging: boolean
}

export interface OpenVpnDetection {
  discovered_paths: string[]
  selected_path: string | null
}

export type SessionOutcome = 'success' | 'failed' | 'cancelled' | 'in_progress'

export interface SessionSummary {
  session_id: string
  profile_name: string
  started_at: string
  ended_at: string | null
  outcome: SessionOutcome
  log_dir: string
}

export interface HelperStatus {
  helperPath: string
  bundledHelperPath: string | null
  installed: boolean
  reason: string | null
}
