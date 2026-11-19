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