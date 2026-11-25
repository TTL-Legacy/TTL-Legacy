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

export interface Passkey {
  passkey_id: string;
  owner: string;
  vault_id: string;
  credential_id: string;
  /**
   * Device name (e.g., "iPhone", "YubiKey 5", "Chrome") — the
   * AAGUID-derived display name, used as a fallback when `friendly_name`
   * is unset or empty.
   */
  device_name: string;
  /**
   * Optional user-editable display name (Issue #2147). When `null` or
   * empty, clients fall back to the AAGUID-derived `device_name`.
   */
  friendly_name?: string | null;
  registered_at: string;
  last_used?: string | null;
  is_backup: boolean;
}

export interface UpdatePasskeyFriendlyNameRequest {
  /**
   * New user-facing display name for the passkey. Empty/null clears the
   * custom name, restoring the AAGUID-derived `device_name` display.
   */
  friendly_name?: string | null;
}
