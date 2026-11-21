export type Locale = "en" | "es" | "fr" | "de";
export type Channel = "email" | "sms" | "push";
export type Frequency = "once" | "daily" | "weekly" | "hourly" | "monthly";
export type Platform = "ios" | "android" | "web";

export interface HealthResponse {
  status: "ok";
  version: string;
}

export interface ReadyResponse {
  status: "ok";
  version: string;
  database: "connected";
}

export interface ErrorResponse {
  error: string;
  status_code?: number;
}

export interface ReminderPreferences {
  vault_id: string;
  channels: Channel[];
  hours_before_expiry: number;
  frequency: Frequency;
}

export interface SetPreferencesRequest {
  channels: Channel[];
  hours_before_expiry: number;
  frequency: Frequency;
}

export interface RegisterTokenRequest {
  owner: string;
  token: string;
  platform: Platform;
}

export interface DeviceToken {
  owner: string;
  token: string;
  platform: Platform;
  registered_at: string;
}

export interface CsrfTokenResponse {
  csrf_token: string;
}

export type UpgradeProposalStatus =
  | "pending"
  | "approved"
  | "rejected"
  | "executed"
  | "expired";

export interface UpgradeProposal {
  id: string;
  proposed_wasm_hash: string;
  proposer: string;
  timelock_until: string;
  eta: string;
  approvals: string[];
  status: UpgradeProposalStatus;
  created_at: string;
}

export interface UpgradeProposalListResponse {
  proposals: UpgradeProposal[];
}

export interface ApproveUpgradeProposalRequest {
  proposal_id: string;
  approver: string;
}

export interface RejectUpgradeProposalRequest {
  proposal_id: string;
  rejector: string;
  reason?: string;
}

export interface UpgradeProposalActionResponse {
  proposal: UpgradeProposal;
}

export interface AdminAddressResponse {
  admin_address: string;
}

/** Placeholder symbol re-exported for compatibility (see index.ts). */
export const PROTOCOL_CONFIG_PROPOSED_TOPIC = "protocol_config_proposed";

// ── Vault notes (Issue #2055) ────────────────────────────────────────────────

/** Request body for `set_vault_notes`. */
export interface SetVaultNotesRequest {
  caller: string;
  name: string;
  description: string;
  notes: string;
}

/** Response returned by `get_vault_notes`. */
export interface GetVaultNotesResponse {
  vault_id: string;
  notes: string;
}

/** Vault check-in record (Issue #2052). */
export interface CheckInRecord {
  timestamp: number;
  ledger_sequence: number;
  /** 0 = owner passkey, 1 = delegate, 2 = batch. */
  method: number;
}

/** Paginated check-in history response (Issue #2052). */
export interface CheckInHistoryResponse {
  vault_id: string;
  records: CheckInRecord[];
  page: number;
  page_size: number;
}

/** Vault pause/resume event (Issue #2053). */
export interface PauseEvent {
  id: number;
  vault_id: string;
  actor: string;
  /** `"pause"` or `"resume"`. */
  event_type: string;
  reason: string;
  timestamp: string;
}

/** Paginated pause history response (Issue #2053). */
export interface PauseHistoryResponse {
  vault_id: string;
  events: PauseEvent[];
  page: number;
  page_size: number;
  total: number;
}