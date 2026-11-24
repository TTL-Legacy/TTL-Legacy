use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ── WebSocket authentication ────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthClaims {
    pub sub: String,
    pub vault_ids: Vec<String>,
    pub exp: usize,
}

// ── Locale support ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Locale {
    En,
    Es,
    Fr,
    De,
}

// ── Notification models ──────────────────────────────────────────────────────

// ── Legacy reminder API models (axum + Db contract) ───────────────────────

/// Reminder notification channel.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Channel {
    Email,
    Sms,
    Push,
}

/// Reminder frequency.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Frequency {
    Once,
    Daily,
    Weekly,
    Hourly,
    Monthly,
}

pub type VaultNotificationPreferences = NotificationPreferences;

/// Timezone used by reminder preferences created before #1752, and the
/// fallback when a stored row has no timezone.
pub fn default_reminder_timezone() -> String {
    "UTC".to_string()
}

/// Persisted reminder preferences stored by `Db`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReminderPreferences {
    pub vault_id: u64,
    pub channels: Vec<Channel>,
    pub hours_before_expiry: u32,
    pub frequency: Frequency,
    /// IANA timezone (e.g. `America/New_York`) used to resolve the local
    /// reminder time. The offset is re-evaluated for every occurrence, so DST
    /// transitions do not shift the reminder (#1752).
    #[serde(default = "default_reminder_timezone")]
    pub timezone: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deleted_at: Option<DateTime<Utc>>,
}

/// Request body for setting reminder preferences.
#[derive(Debug, Deserialize, Clone)]
pub struct SetPreferencesRequest {
    pub channels: Vec<Channel>,
    pub hours_before_expiry: u32,
    pub frequency: Frequency,
    /// IANA timezone for the local reminder time. Omitted values keep the
    /// backwards-compatible `UTC` default.
    #[serde(default)]
    pub timezone: Option<String>,
}

/// Notification type sent to a device.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum NotificationType {
    ExpiryWarning,
    CheckInReminder,
    VaultReleased,
    VaultPaused,
    /// Fired whenever a withdrawal attempt is made (successful or failed).
    WithdrawalAlert,
}

/// Delivery status of a single notification attempt.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryStatus {
    Pending,
    Sent,
    Failed,
    Retrying,
}

/// A single attempt entry within a reminder delivery log.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeliveryAttempt {
    pub attempt: u32,
    pub attempted_at: DateTime<Utc>,
    pub error: String,
}

/// Per-notification retry log stored by notification ID.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReminderDeliveryLog {
    pub notification_id: String,
    pub vault_id: String,
    pub owner: String,
    pub status: DeliveryStatus,
    pub attempts: Vec<DeliveryAttempt>,
    /// When the next retry should fire (None if not retrying).
    pub next_retry_at: Option<DateTime<Utc>>,
}

/// A registered device push token.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceToken {
    pub owner: String,
    pub token: String,
    /// "ios" | "android" | "web"
    pub platform: String,
    pub registered_at: DateTime<Utc>,
}

/// Per-owner notification preferences (used by legacy scheduler/reminder engine).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationPreferences {
    pub owner: String,
    pub expiry_warning_enabled: bool,
    pub check_in_reminder_enabled: bool,
    pub vault_released_enabled: bool,
    /// Hours before expiry to send the warning (default 24).
    pub warning_hours_before: u64,
    pub locale: Option<Locale>,
    pub preferred_channel: Option<NotificationChannel>,
    pub fallback_channel: Option<NotificationChannel>,
    pub unsubscribed: bool,
}

impl Default for NotificationPreferences {
    fn default() -> Self {
        Self {
            owner: String::new(),
            expiry_warning_enabled: true,
            check_in_reminder_enabled: true,
            vault_released_enabled: true,
            warning_hours_before: 24,
            locale: None,
            preferred_channel: None,
            fallback_channel: None,
            unsubscribed: false,
        }
    }
}

// ── Withdrawal alert notification preferences ───────────────────────────────

/// Per-vault opt-in preferences for withdrawal alert notifications.
///
/// Each vault owner can independently enable email and/or push alerts for
/// every withdrawal attempt (successful or failed) against their vault.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WithdrawalAlertPreferences {
    pub vault_id: u64,
    pub owner: String,
    /// Send an email notification on every withdrawal attempt.
    pub email_enabled: bool,
    /// Send a push notification on every withdrawal attempt.
    pub push_enabled: bool,
}

impl Default for WithdrawalAlertPreferences {
    fn default() -> Self {
        Self {
            vault_id: 0,
            owner: String::new(),
            email_enabled: false,
            push_enabled: false,
        }
    }
}

/// Request body for `PUT /api/vaults/{vault_id}/withdrawal-alert-preferences`.
#[derive(Debug, Deserialize)]
pub struct SetWithdrawalAlertPrefsRequest {
    pub owner: String,
    pub email_enabled: bool,
    pub push_enabled: bool,
}

/// Describes a single withdrawal attempt that should trigger notifications.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WithdrawalEvent {
    pub vault_id: u64,
    pub owner: String,
    /// Amount attempted (in stroops / base units).
    pub amount: i128,
    /// Whether the withdrawal completed successfully.
    pub success: bool,
    /// Optional human-readable reason for failure.
    pub failure_reason: Option<String>,
    pub attempted_at: chrono::DateTime<chrono::Utc>,
    /// Stellar transaction hash, if available.
    pub tx_hash: Option<String>,
}

// ── Unsubscribe support (#828) ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnsubscribeToken {
    pub token: String,
    pub owner: String,
    pub created_at: DateTime<Utc>,
}

// ── Token-based reminder links (#1286) ──────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReminderToken {
    pub token: String,
    pub vault_id: String,
    pub owner: String,
    pub created_at: DateTime<Utc>,
}

// ── Channel fallback delivery log (#827) ────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelDeliveryLog {
    pub notification_id: String,
    pub channel: NotificationChannel,
    pub status: DeliveryStatus,
    pub attempted_at: DateTime<Utc>,
    pub error: Option<String>,
}

/// A scheduled notification (pending delivery).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledNotification {
    pub id: String,
    pub vault_id: String,
    pub owner: String,
    pub notification_type: NotificationType,
    /// Unix timestamp when this should fire.
    pub scheduled_at: DateTime<Utc>,
    pub status: DeliveryStatus,
    pub max_retry_attempts: u32,
    pub sent_at: Option<DateTime<Utc>>,
    /// Hours left on the vault TTL, quoted in `ExpiryWarning` bodies.
    /// `None` falls back to a generic 24h figure.
    #[serde(default)]
    pub ttl_hours: Option<u64>,
}

/// Delivery record written after each send attempt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeliveryRecord {
    pub notification_id: String,
    pub vault_id: String,
    pub owner: String,
    pub notification_type: NotificationType,
    pub status: DeliveryStatus,
    pub sent_at: DateTime<Utc>,
    /// FCM message ID on success, error string on failure.
    pub provider_response: String,
}

/// Request body for `POST /notifications/register`.
#[derive(Debug, Deserialize)]
pub struct RegisterTokenRequest {
    pub owner: String,
    pub token: String,
    pub platform: String,
}

/// Request body for `PUT /notifications/preferences`.
#[derive(Debug, Deserialize)]
pub struct UpdatePreferencesRequest {
    pub owner: String,
    pub expiry_warning_enabled: Option<bool>,
    pub check_in_reminder_enabled: Option<bool>,
    pub vault_released_enabled: Option<bool>,
    pub warning_hours_before: Option<u64>,
    pub locale: Option<Locale>,
}

// ── Existing models (unchanged) ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vault {
    pub id: String,
    pub owner: String,
    pub beneficiary: String,
    pub balance: i128,
    pub check_in_interval: u64,
    pub last_check_in: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub status: VaultStatus,
    pub ttl_remaining: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum VaultStatus {
    Active,
    Expired,
    Released,
    Paused,
    /// Vault has been archived — data is frozen and the entry removed from the
    /// active store (Issue #2048).
    Archived,
}

// ── TTL Insurance models ───────────────────────────────────────────────────

/// TTL insurance policy parameters purchased by a vault owner.
///
/// When enabled, the backend scheduler can automatically extend TTL once the
/// owner is considered inactive.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtlInsurancePolicy {
    /// Vault id (matches `Vault.id` semantics in this backend).
    pub vault_id: u64,
    /// How much TTL to extend when triggered.
    pub extension_seconds: u64,
    /// Consider the owner inactive if no proof-of-life/check-in was recorded
    /// within this window.
    pub inactivity_threshold_seconds: u64,
    /// Whether this policy is currently active.
    pub enabled: bool,
    pub purchased_at: DateTime<Utc>,
    pub last_extended_at: Option<DateTime<Utc>>,
}

/// Persisted owner activity signal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OwnerActivity {
    pub owner_id: u64,
    pub last_active_at: DateTime<Utc>,
}

/// POST body to purchase/enable a TTL insurance policy.
#[derive(Debug, Deserialize, Clone)]
pub struct PurchaseTtlInsuranceRequest {
    pub extension_seconds: u64,
    pub inactivity_threshold_seconds: u64,
}

/// POST body to record owner activity (proof-of-life).
#[derive(Debug, Deserialize, Clone)]
pub struct RecordOwnerActivityRequest {
    pub owner_id: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultEvent {
    pub vault_id: String,
    pub event_type: EventType,
    pub timestamp: DateTime<Utc>,
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    CheckIn,
    TtlUpdate,
    StatusChange,
    Deposit,
    Withdrawal,
    Release,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchQuery {
    pub owner: Option<String>,
    pub beneficiary: Option<String>,
    pub status: Option<VaultStatus>,
    pub created_after: Option<DateTime<Utc>>,
    pub created_before: Option<DateTime<Utc>>,
    /// Maximum number of results to return (1–100, default 20).
    pub limit: Option<u32>,
    /// Opaque cursor returned by a previous response; omit for the first page.
    pub cursor: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchResult {
    pub vaults: Vec<Vault>,
    /// Cursor to pass as `cursor` in the next request; absent when there are no more results.
    pub next_cursor: Option<String>,
    pub limit: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ComparisonResult {
    pub vaults: Vec<Vault>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExportData {
    pub vault: Vault,
    pub history: Vec<VaultEvent>,
    pub audit_log: Vec<AuditEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub timestamp: DateTime<Utc>,
    pub action: String,
    pub actor: String,
    pub details: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WebSocketMessage {
    pub message_type: String,
    pub data: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ComplianceReport {
    pub vault_id: String,
    pub owner: String,
    pub beneficiary: String,
    pub report_generated_at: DateTime<Utc>,
    pub fund_movements: Vec<FundMovement>,
    pub beneficiary_changes: Vec<BeneficiaryChange>,
    pub ttl_history: Vec<TtlEvent>,
    pub total_deposits: i128,
    pub total_withdrawals: i128,
    pub current_balance: i128,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FundMovement {
    pub timestamp: DateTime<Utc>,
    pub movement_type: String,
    pub amount: i128,
    pub balance_after: i128,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BeneficiaryChange {
    pub timestamp: DateTime<Utc>,
    pub old_beneficiary: String,
    pub new_beneficiary: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TtlEvent {
    pub timestamp: DateTime<Utc>,
    pub event_type: String,
    pub ttl_remaining: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VaultTemplate {
    pub id: String,
    pub name: String,
    pub description: String,
    pub check_in_interval: u64,
    pub recommended_for: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VaultTemplateList {
    pub templates: Vec<VaultTemplate>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateVaultFromTemplate {
    pub template_id: String,
    pub owner: String,
    pub beneficiary: String,
}

// ── Task 1: Analytics ────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct VaultAnalytics {
    pub total_vaults: u64,
    pub active_vaults: u64,
    pub average_ttl_seconds: f64,
    pub release_rate: f64, // fraction of vaults that are Released
    pub time_series: Vec<TimeSeriesPoint>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TimeSeriesPoint {
    pub date: String, // ISO-8601 date (YYYY-MM-DD)
    pub vaults_created: u64,
    pub vaults_released: u64,
}

// ── Per-Vault Analytics (#959) ───────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct VaultDetailAnalytics {
    pub vault_id: String,
    pub ttl_history: Vec<TtlHistoryPoint>,
    pub check_in_frequency: CheckInFrequency,
    pub withdrawal_trends: WithdrawalTrends,
    pub beneficiary_status: BeneficiaryStatus,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TtlHistoryPoint {
    pub date: String,
    pub ttl_remaining_seconds: u64,
    pub event: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CheckInFrequency {
    pub average_interval_seconds: u64,
    pub total_check_ins: u64,
    pub next_deadline: String,
    pub days_until_deadline: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WithdrawalTrends {
    pub total_withdrawals: i128,
    pub withdrawal_count: u64,
    pub average_withdrawal_amount: f64,
    pub last_withdrawal_date: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BeneficiaryStatus {
    pub beneficiary_address: String,
    pub is_active: bool,
    pub vault_status: String,
    pub can_receive_funds: bool,
}

// ── Task 2: Backup & Recovery ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultBackup {
    pub backup_id: String,
    pub vault_id: String,
    pub created_at: DateTime<Utc>,
    /// AES-GCM encrypted JSON of the vault state (base64-encoded)
    pub encrypted_payload: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RestoreRequest {
    pub backup_id: String,
    /// The same key used during backup (base64-encoded 32-byte key)
    pub encryption_key: String,
}

// ── Task 3: Sharing & Collaboration ──────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SharePermission {
    ViewOnly,
    Edit,
    Admin,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultShare {
    pub share_id: String,
    pub vault_id: String,
    pub shared_with: String, // address or email
    pub permission: SharePermission,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ShareRequest {
    pub shared_with: String,
    pub permission: SharePermission,
}

// ── Share tokens (temporary access tokens for read-only sharing) ─────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareToken {
    pub token: String,
    pub share_id: String,
    pub vault_id: String,
    pub shared_with: String,
    pub permission: SharePermission,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub revoked: bool,
}

#[derive(Debug, Deserialize)]
pub struct GenerateTokenRequest {
    pub shared_with: String,
    pub permission: Option<SharePermission>,
    /// Seconds until the token expires (default 604800 = 7 days).
    pub expiry_seconds: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ShareTokenResponse {
    pub share: VaultShare,
    pub token: ShareToken,
    pub access_url: String,
}

#[derive(Debug, Deserialize)]
pub struct RevokeTokenRequest {
    pub token: String,
}

// ── Task 4: Notification Preferences ─────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum NotificationChannel {
    Email,
    Sms,
    Push,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum NotificationFrequency {
    Daily,
    Weekly,
    Monthly,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NotificationPreferencesRequest {
    pub channels: Vec<NotificationChannel>,
    pub frequency: NotificationFrequency,
}

// ── Vault Notification Subscription System ──────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SubscriptionChannel {
    Email,
    Sms,
    Webhook,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SubscriptionFrequency {
    Once,
    Daily,
    Weekly,
    Hourly,
    Monthly,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Subscription {
    pub vault_id: u64,
    pub owner: String,
    pub channels: Vec<SubscriptionChannel>,
    pub frequency: SubscriptionFrequency,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SetSubscriptionRequest {
    pub owner: String,
    pub channels: Vec<SubscriptionChannel>,
    pub frequency: SubscriptionFrequency,
}

// ── Idempotency Key support (#825) ──────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdempotencyRecord {
    pub key: String,
    pub response_body: String,
    pub status_code: u16,
    pub created_at: DateTime<Utc>,
}

// ── Release Simulator models ─────────────────────────────────────────────────

/// The scenario to simulate.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScenarioType {
    /// Owner never checks in again — release is immediate at TTL expiry.
    NoCheckIns,
    /// Owner checks in consistently at their configured interval.
    ConsistentCheckIns,
    /// Owner misses one or more specific check-in dates before stopping.
    MissedCheckInDates,
}

impl std::fmt::Display for ScenarioType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScenarioType::NoCheckIns => write!(f, "no_check_ins"),
            ScenarioType::ConsistentCheckIns => write!(f, "consistent_check_ins"),
            ScenarioType::MissedCheckInDates => write!(f, "missed_check_in_dates"),
        }
    }
}

/// Query parameters for the simulate-release endpoint.
#[derive(Debug, Deserialize)]
pub struct SimulateReleaseQuery {
    /// Comma-separated list of scenarios to run.
    /// e.g. `scenarios=no_check_ins,consistent_check_ins`
    /// Defaults to all three scenarios if omitted.
    pub scenarios: Option<String>,
    /// For `missed_check_in_dates`: number of consecutive missed check-ins
    /// before the owner stops (defaults to 1).
    pub missed_count: Option<u32>,
}

/// Projected outcome for a single scenario.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioResult {
    /// Which scenario this result belongs to.
    pub scenario: ScenarioType,
    /// Human-readable description of the scenario.
    pub description: String,
    /// Projected UTC timestamp when the vault will release.
    pub projected_release_at: DateTime<Utc>,
    /// Seconds from now until the projected release.
    pub seconds_until_release: i64,
    /// Confidence level: "high", "medium", or "low".
    pub confidence: String,
    /// Optional extra notes about this scenario's assumptions.
    pub notes: String,
}

/// Response body for GET /api/vaults/{id}/simulate-release.
#[derive(Debug, Serialize, Deserialize)]
pub struct SimulateReleaseResponse {
    pub vault_id: String,
    /// Current TTL remaining in seconds (None if already expired/released).
    pub current_ttl_remaining: Option<u64>,
    /// The vault's configured check-in interval in seconds.
    pub check_in_interval: u64,
    /// Timestamp of the last recorded check-in.
    pub last_check_in: DateTime<Utc>,
    /// Simulation results, one per requested scenario.
    pub scenarios: Vec<ScenarioResult>,
    /// When this simulation was generated.
    pub simulated_at: DateTime<Utc>,
}

// ── Passkey Recovery Flow (#1299) ──────────────────────────────────────────

/// A registered passkey for a vault owner.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Passkey {
    pub passkey_id: String,
    pub owner: String,
    pub vault_id: String,
    pub credential_id: String,
    /// Device name (e.g., "iPhone", "YubiKey 5", "Chrome")
    pub device_name: String,
    pub registered_at: DateTime<Utc>,
    pub last_used: Option<DateTime<Utc>>,
    pub is_backup: bool,
}

/// A single recovery code for account recovery.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryCode {
    pub code_id: String,
    pub owner: String,
    pub vault_id: String,
    /// The recovery code itself (hashed for storage)
    pub code_hash: String,
    pub generated_at: DateTime<Utc>,
    pub used_at: Option<DateTime<Utc>>,
}

/// Recovery codes bundle for a vault owner.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryCodeSet {
    pub set_id: String,
    pub owner: String,
    pub vault_id: String,
    /// Array of plaintext codes (only shown once at generation)
    pub codes: Vec<String>,
    pub generated_at: DateTime<Utc>,
    pub codes_used: u32,
    pub total_codes: u32,
}

/// Request to register an additional passkey.
#[derive(Debug, Deserialize)]
pub struct RegisterPasskeyRequest {
    pub vault_id: String,
    pub owner: String,
    pub credential_id: String,
    pub device_name: String,
    pub is_backup: Option<bool>,
}

/// Response when registering a new passkey.
#[derive(Debug, Serialize)]
pub struct RegisterPasskeyResponse {
    pub passkey_id: String,
    pub vault_id: String,
    pub device_name: String,
    pub registered_at: DateTime<Utc>,
}

/// Request to initiate recovery using a backup passkey or recovery code.
#[derive(Debug, Deserialize)]
pub struct RecoveryRequest {
    pub vault_id: String,
    pub owner: String,
    /// Either a backup passkey credential or a recovery code
    pub recovery_credential: String,
    pub recovery_method: RecoveryMethod,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryMethod {
    BackupPasskey,
    RecoveryCode,
}

/// Response when recovery is successful.
#[derive(Debug, Serialize)]
pub struct RecoveryResponse {
    pub recovery_id: String,
    pub vault_id: String,
    pub owner: String,
    pub recovery_method: RecoveryMethod,
    pub authenticated_at: DateTime<Utc>,
}

/// Request to generate new recovery codes.
#[derive(Debug, Deserialize)]
pub struct GenerateRecoveryCodesRequest {
    pub vault_id: String,
    pub owner: String,
}

/// Response with newly generated recovery codes (shown only once).
#[derive(Debug, Serialize)]
pub struct GenerateRecoveryCodesResponse {
    pub set_id: String,
    pub vault_id: String,
    pub recovery_codes: Vec<String>,
    pub generated_at: DateTime<Utc>,
    pub note: String,
}

// --- Issue #1143: Vesting Bonus Backend API ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VestingBonusConfig {
    pub vault_id: String,
    pub bonus_bps: u32,
    pub on_time_window_seconds: u64,
}

/// Request body for POST /api/vaults/{id}/vesting/claim-bonus.
#[derive(Debug, Deserialize)]
pub struct ClaimBonusRequest {
    /// Caller must be the beneficiary; this is the Stellar account address.
    pub beneficiary: String,
    /// Base64-encoded, beneficiary-signed Soroban transaction XDR.
    pub transaction_xdr: String,
    /// Optional memo for the claim transaction.
    pub memo: Option<String>,
}

/// Response body for POST /api/vaults/{id}/vesting/claim-bonus.
#[derive(Debug, Serialize)]
pub struct ClaimBonusResponse {
    pub vault_id: String,
    pub claimed_amount: i128,
    pub bonus_amount: i128,
    pub tx_hash: String,
    pub claimed_at: DateTime<Utc>,
}

/// Response body for GET /api/vaults/{id}/vesting/bonus.
#[derive(Debug, Serialize)]
pub struct VestingBonusResponse {
    pub vault_id: String,
    pub configured: bool,
    pub bonus_bps: Option<u32>,
    pub on_time_window_seconds: Option<u64>,
}

// ── Persistent audit log (SQLite-backed, written by audit::audit_middleware) ─
// Issue #1173: these types are referenced throughout db.rs/audit.rs but were
// never actually defined anywhere in the crate.

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogEntry {
    pub id: i64,
    pub timestamp: DateTime<Utc>,
    pub user_id: String,
    pub action: String,
    pub resource: String,
    pub result: String,
    pub ip_address: String,
    pub details: Option<serde_json::Value>,
}

#[derive(Debug, Default, Deserialize)]
pub struct AuditLogQuery {
    pub user_id: Option<String>,
    pub action: Option<String>,
    pub resource: Option<String>,
    pub result: Option<String>,
    pub after: Option<DateTime<Utc>>,
    pub before: Option<DateTime<Utc>>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// Response body for `POST /api/vaults/{id}/withdraw` (Issue #2056).
#[derive(Debug, Serialize)]
pub struct TriggerWithdrawalResponse {
    pub vault_id: String,
    pub address: String,
    pub amount: i128,
    pub tx_hash: String,
    pub withdrawn_at: DateTime<Utc>,
}

// ── Issue #2056: Withdrawal rate-limit config ────────────────────────────────

/// Per-vault configuration that caps the total amount that may be withdrawn
/// within a rolling time window.
///
/// When set, `trigger_withdrawal` enforces these limits before forwarding the
/// request to Soroban and returns `AppError::WithdrawalRateLimitExceeded` if
/// the cap would be breached.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WithdrawalRateLimitConfig {
    /// Vault this config belongs to.
    pub vault_id: String,
    /// Maximum total amount (in stroops / base units) that may leave the vault
    /// within any rolling window of `window_seconds`.
    pub max_amount_per_window: i128,
    /// Length of the rolling window in seconds (e.g. 3600 = 1 hour).
    pub window_seconds: u64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Request body for `PUT /api/vaults/{id}/withdrawal-rate-limit`.
#[derive(Debug, Deserialize)]
pub struct SetWithdrawalRateLimitRequest {
    pub max_amount_per_window: i128,
    pub window_seconds: u64,
}

/// A single withdrawal event recorded for rate-limit accounting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WithdrawalRecord {
    pub vault_id: String,
    pub amount: i128,
    pub recorded_at: DateTime<Utc>,
}

// ── Issue #2057: Vault summary with health score ──────────────────────────────

/// Extended vault summary returned by `GET /vaults/{id}/summary`.
///
/// Extends the base `VaultSummary` with a computed 0–100 health score derived
/// from TTL remaining, check-in streak, beneficiary confirmation status, and
/// backup code freshness.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultSummaryResponse {
    pub vault_id: String,
    pub owner: String,
    pub status: VaultStatus,
    pub ttl_remaining: Option<u64>,
    pub balance: i128,
    /// 0–100 health score. See `compute_vault_summary_health_score` for the
    /// full breakdown.
    pub health_score: u8,
}

/// Inputs used to compute the summary health score.
/// All are optional/defaultable so the endpoint degrades gracefully when some
/// data sources are unavailable.
#[derive(Debug, Default)]
pub struct VaultSummaryHealthInputs {
    /// Seconds of TTL remaining.
    pub ttl_remaining: Option<u64>,
    /// Configured check-in interval (seconds).
    pub check_in_interval: u64,
    /// Number of consecutive check-ins the owner has completed without missing.
    pub check_in_streak: u32,
    /// Whether the beneficiary has confirmed their address in the last 90 days.
    pub beneficiary_confirmed: bool,
    /// Age of the most recently generated backup-code set, in seconds.
    /// `None` means no backup codes have ever been generated.
    pub backup_code_age_secs: Option<u64>,
}

/// Compute the 0–100 summary health score from the supplied inputs.
///
/// Factor breakdown (total 100 pts):
/// - TTL buffer    40 pts — ratio of `ttl_remaining` to `check_in_interval`
/// - Streak        20 pts — consecutive check-ins (full at 5+)
/// - Beneficiary   20 pts — 20 if confirmed, 0 otherwise
/// - Backup codes  20 pts — 20 if generated ≤ 180 days ago, 0 otherwise
pub fn compute_vault_summary_health_score(inputs: &VaultSummaryHealthInputs) -> u8 {
    // TTL buffer (0–40)
    let ttl_score: u8 = match inputs.ttl_remaining {
        Some(ttl) if inputs.check_in_interval > 0 => {
            let ratio = ttl as f64 / inputs.check_in_interval as f64;
            (ratio.min(1.0) * 40.0).round() as u8
        }
        _ => 0,
    };

    // Streak (0–20), full at 5 consecutive check-ins
    let streak_score: u8 = {
        let ratio = inputs.check_in_streak as f64 / 5.0_f64;
        (ratio.min(1.0) * 20.0).round() as u8
    };

    // Beneficiary confirmed (0 or 20)
    let beneficiary_score: u8 = if inputs.beneficiary_confirmed { 20 } else { 0 };

    // Backup code freshness (0 or 20) — fresh = generated within 180 days
    const BACKUP_FRESH_SECS: u64 = 180 * 24 * 3600;
    let backup_score: u8 = match inputs.backup_code_age_secs {
        Some(age) if age <= BACKUP_FRESH_SECS => 20,
        _ => 0,
    };

    ttl_score
        .saturating_add(streak_score)
        .saturating_add(beneficiary_score)
        .saturating_add(backup_score)
}

// ── Issue #2058: Mobile push notification for low TTL ────────────────────────

/// Platform-specific push credentials for a device token.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MobilePlatform {
    Android,
    Ios,
}

/// Per-owner opt-in preferences for the low-TTL mobile push notification.
///
/// The notification fires when TTL drops below 48 hours and no check-in has
/// been recorded in the past 24 hours. The user must have explicitly opted in
/// for at least one platform before any push is sent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LowTtlPushPreferences {
    pub owner: String,
    /// Vault ID this preference applies to. Each vault can be configured
    /// independently.
    pub vault_id: String,
    /// Whether the user has opted in to Android (FCM) push for this vault.
    pub android_enabled: bool,
    /// Whether the user has opted in to iOS (APNs) push for this vault.
    pub ios_enabled: bool,
    pub updated_at: DateTime<Utc>,
}

impl Default for LowTtlPushPreferences {
    fn default() -> Self {
        Self {
            owner: String::new(),
            vault_id: String::new(),
            android_enabled: false,
            ios_enabled: false,
            updated_at: DateTime::default(),
        }
    }
}

/// Request body for `PUT /api/vaults/{id}/low-ttl-push-preferences`.
#[derive(Debug, Deserialize)]
pub struct SetLowTtlPushPreferencesRequest {
    pub owner: String,
    pub android_enabled: bool,
    pub ios_enabled: bool,
}

/// Internal record checked before firing a low-TTL push to prevent duplicate
/// alerts within a cooldown window.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LowTtlPushSentRecord {
    pub vault_id: String,
    pub owner: String,
    pub sent_at: DateTime<Utc>,
}

// ── Issue #2059: Portable vault config bundle ─────────────────────────────────

/// A signed, portable JSON bundle that captures the exportable configuration
/// of a vault. It can be re-imported on another account via
/// `POST /vaults/import`.
///
/// The bundle intentionally excludes sensitive runtime state (private keys,
/// beneficiary contact details, withdrawal whitelist entries) and
/// non-portable data (vault ID, owner address, balance, TTL remaining).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultConfigBundle {
    /// Schema version — increment when the shape changes.
    pub version: u8,
    /// ID of the source vault (informational only; the importer must not
    /// reuse it as the new vault ID).
    pub source_vault_id: String,
    pub exported_at: DateTime<Utc>,
    /// Ed25519 signature over the canonical JSON of the `payload` field,
    /// base64url-encoded. Used to detect tampering. In this implementation
    /// the signature is a HMAC-SHA256 keyed on the server secret; the verifier
    /// checks it on import.
    pub signature: String,
    pub payload: VaultConfigPayload,
}

/// The portable vault configuration payload embedded in `VaultConfigBundle`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultConfigPayload {
    pub beneficiaries: Vec<BundleBeneficiary>,
    pub release_conditions: Vec<ReleaseCondition>,
    pub vesting_schedule: Option<BundleVestingSchedule>,
    pub tags: Vec<String>,
}

/// A beneficiary entry as it appears inside a portable config bundle.
/// The address is included; contact details are excluded.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleBeneficiary {
    pub address: String,
    /// Fraction of the vault assigned to this beneficiary, in basis points
    /// (10 000 = 100 %).
    pub share_bps: u32,
}

/// A single release condition (predicate) stored in the portable bundle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseCondition {
    /// Machine-readable condition type, e.g. `"ttl_expired"`,
    /// `"beneficiary_request"`, `"multisig_threshold"`.
    pub condition_type: String,
    /// Arbitrary JSON parameters for this condition type.
    pub params: serde_json::Value,
}

/// Vesting schedule as exported in the portable bundle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleVestingSchedule {
    pub bonus_bps: u32,
    pub on_time_window_seconds: u64,
}

/// Request body for `POST /vaults/import`.
#[derive(Debug, Deserialize)]
pub struct ImportVaultConfigRequest {
    /// The serialised `VaultConfigBundle` JSON.
    pub bundle: VaultConfigBundle,
    /// Target owner address on the new account.
    pub new_owner: String,
}

/// Response from `POST /vaults/import`.
#[derive(Debug, Serialize)]
pub struct ImportVaultConfigResponse {
    /// ID of the newly created vault.
    pub new_vault_id: String,
    pub owner: String,
    pub imported_at: DateTime<Utc>,
    /// Number of beneficiaries applied to the new vault.
    pub beneficiary_count: usize,
    pub tags_applied: Vec<String>,
}

// ── Issue #1173: VaultReleaseHistory ────────────────────────────────────────

/// Response body for GET /api/vaults/{id}/release-history — Issue #1173.
#[derive(Debug, Serialize)]
pub struct VaultReleaseHistory {
    pub vault_id: String,
    /// Every audit-logged API request against this vault's release-related
    /// endpoints (e.g. /simulate-release, /sponsored-release), oldest last.
    pub audit_entries: Vec<AuditLogEntry>,
    /// Sponsored-release attempts/completions recorded for this vault.
    pub sponsored_releases: Vec<crate::fee_sponsorship::SponsoredRelease>,
}

/// State transition entry for compliance reporting — Issue #1854.
///
/// Tracks vault status changes (Active -> Released, Active -> Paused, etc.)
/// for audit and compliance purposes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateTransitionEntry {
    pub id: i64,
    pub vault_id: String,
    pub from_status: VaultStatus,
    pub to_status: VaultStatus,
    pub timestamp: DateTime<Utc>,
    pub triggered_by: String, // "owner", "beneficiary", "system", "admin"
    pub trigger_reason: Option<String>, // e.g., "owner_missing", "manual_release", "pause_requested"
    pub details: Option<serde_json::Value>,
}

/// Query parameters for state transition history.
#[derive(Debug, Default, Deserialize)]
pub struct StateTransitionQuery {
    pub vault_id: Option<String>,
    pub from_status: Option<VaultStatus>,
    pub to_status: Option<VaultStatus>,
    pub triggered_by: Option<String>,
    pub after: Option<DateTime<Utc>>,
    pub before: Option<DateTime<Utc>>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// Response for state transition history.
#[derive(Debug, Serialize)]
pub struct StateTransitionHistory {
    pub vault_id: String,
    pub transitions: Vec<StateTransitionEntry>,
}

// ── Auth: refresh token rotation (Issue #1177) ───────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefreshClaims {
    pub sub: String,
    /// Unique ID for this refresh token — the primary key in `refresh_tokens`.
    pub jti: String,
    /// Shared by every token descended from the same original login; used to
    /// revoke an entire family if a rotated-out token is presented again.
    pub family_id: String,
    pub exp: usize,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub sub: String,
    #[serde(default)]
    pub vault_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

#[derive(Debug, Serialize)]
pub struct TokenPairResponse {
    pub access_token: String,
    pub refresh_token: String,
    /// Access token lifetime in seconds, for client-side proactive refresh.
    pub expires_in: i64,
    /// Fresh CSRF token issued on every successful authentication.
    ///
    /// The server simultaneously sets a `__Host-csrf` HttpOnly cookie carrying
    /// the same value (see issue #1497). Clients must include this value in the
    /// `X-CSRF-Token` header on every subsequent state-mutating request.
    pub csrf_token: String,
}

// ── Issue #1337: Beneficiary archival notification ───────────────────────────

/// Contact information for a vault beneficiary.
///
/// The beneficiary registers their contact details so the backend can notify
/// them when the vault's TTL expires and funds become claimable.
/// The vault owner cannot read the contact details — they are stored
/// server-side and used exclusively for outbound notifications.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeneficiaryContact {
    /// Stellar address of the beneficiary.
    pub beneficiary_address: String,
    /// Vault ID this contact entry belongs to.
    pub vault_id: String,
    /// Optional email address for archival notifications.
    pub email: Option<String>,
    /// Optional phone number for SMS archival notifications.
    pub phone: Option<String>,
    /// When this contact entry was created or last updated.
    pub updated_at: DateTime<Utc>,
    /// Whether the beneficiary has opted in to archival notifications.
    /// Defaults to `true` — the beneficiary can opt out at any time.
    pub opted_in: bool,
}

/// Request body for `POST /api/vaults/{id}/beneficiary/contact`.
#[derive(Debug, Deserialize)]
pub struct SetBeneficiaryContactRequest {
    /// Stellar address of the beneficiary (used to verify the caller).
    pub beneficiary_address: String,
    pub email: Option<String>,
    pub phone: Option<String>,
}

/// Request body for `PUT /api/vaults/{id}/beneficiary/notifications/opt-in`.
#[derive(Debug, Deserialize)]
pub struct BeneficiaryNotificationOptInRequest {
    pub beneficiary_address: String,
    /// `true` to opt in, `false` to opt out.
    pub opted_in: bool,
}

/// An archival notification dispatch record — written when the backend sends
/// a TTL-expiry notification to a beneficiary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeneficiaryArchivalNotification {
    pub id: String,
    pub vault_id: String,
    pub beneficiary_address: String,
    /// Channel used for this dispatch ("email" | "sms").
    pub channel: String,
    pub dispatched_at: DateTime<Utc>,
    pub status: DeliveryStatus,
    /// Error message if delivery failed.
    pub error: Option<String>,
}

// ── Vault clone config (Issue #2050) ────────────────────────────────────────

/// Controls which optional fields are copied when cloning a vault.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VaultCloneConfig {
    /// Copy the beneficiary field (default: always copied).
    /// Optional fields below default to false.
    pub include_tags: bool,
    pub include_notes: bool,
    pub include_beneficiaries: bool,
    pub include_vesting_schedule: bool,
    pub include_release_conditions: bool,
}

/// Response from clone_vault.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloneVaultResponse {
    pub original_vault_id: String,
    pub cloned_vault_id: String,
    pub cloned_vault: Vault,
}

// ── Vault summary / bulk summary ────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultSummary {
    pub vault_id: String,
    pub owner: String,
    pub status: VaultStatus,
    pub ttl_remaining: Option<u64>,
    pub balance: i128,
}

#[derive(Debug, Deserialize)]
pub struct BulkSummaryRequest {
    pub vault_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultSummaryEntry {
    pub vault_id: String,
    pub status: Option<VaultStatus>,
    pub ttl_remaining: Option<u64>,
    pub balance: Option<i128>,
    pub last_check_in: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct BulkSummaryResponse {
    pub summaries: Vec<VaultSummaryEntry>,
}

// ── Vault health scoring ────────────────────────────────────────────────────

/// Per-factor breakdown of a vault health score.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthFactors {
    pub ttl_buffer: u8,
    pub streak: u8,
    pub balance: u8,
    pub passkey_diversity: u8,
}

/// 0-100 health score with a per-factor breakdown.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultHealthResponse {
    pub score: u8,
    pub factors: HealthFactors,
    pub computed_at: DateTime<Utc>,
}

// ── Token price oracle (Issue #2066) ────────────────────────────────────────

/// Which price oracle provider to use.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum OracleProvider {
    /// Redstone — pull-model oracle, prices embedded in transactions.
    Redstone,
    /// Pyth Network — push-model oracle with on-chain price feeds.
    Pyth,
}

/// Configuration for the token price oracle used by the backend to estimate
/// vault values in USD.
///
/// One instance of this config is stored per deployment (admin-managed).
/// The scheduler reads it to decide how often to refresh cached prices.
///
/// Issue #2066.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenPriceOracleConfig {
    /// Which oracle provider to use.
    pub provider: OracleProvider,
    /// Base URL of the oracle REST or WebSocket endpoint.
    /// Example: `"https://api.redstone.finance/prices"` or
    ///          `"https://hermes.pyth.network/api"`.
    pub endpoint_url: String,
    /// How often the backend fetches a fresh price (seconds). Must be ≥ 10.
    pub refresh_interval_seconds: u64,
    /// XLM price feed identifier as understood by the chosen provider.
    /// For Redstone: `"XLM"`. For Pyth: the hex feed ID.
    pub xlm_feed_id: String,
    /// Whether the oracle integration is currently enabled.
    pub enabled: bool,
    /// When this config was last updated.
    pub updated_at: DateTime<Utc>,
}

/// A single cached price entry returned by the backend oracle poller.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedTokenPrice {
    /// Token symbol (e.g. `"XLM"` or the SEP-41 contract address).
    pub token_id: String,
    /// USD price (rounded to 8 decimal places).
    pub price_usd: f64,
    /// When this price was fetched from the upstream oracle.
    pub fetched_at: DateTime<Utc>,
    /// Source oracle provider.
    pub provider: OracleProvider,
}

/// Response body for `GET /vaults/{id}/estimated-value`.
///
/// Issue #2066.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EstimatedVaultValue {
    pub vault_id: String,
    /// Vault balance in stroops (1 XLM = 10 000 000 stroops).
    pub balance_stroops: i128,
    /// Estimated USD value of the vault's balance, using the latest cached price.
    /// `None` when no price data is available.
    pub estimated_value_usd: Option<f64>,
    /// Token symbol used for the valuation.
    pub token_symbol: String,
    /// USD price per token used for the calculation.
    pub price_per_token_usd: Option<f64>,
    /// When the price used for valuation was fetched.
    pub price_fetched_at: Option<DateTime<Utc>>,
    /// Whether the price data is considered stale (older than 2× refresh interval).
    pub is_stale: bool,
    pub estimated_at: DateTime<Utc>,
}

/// Request body for `PUT /admin/oracle/config`.
#[derive(Debug, Deserialize)]
pub struct SetOracleConfigRequest {
    pub provider: OracleProvider,
    pub endpoint_url: String,
    pub refresh_interval_seconds: u64,
    pub xlm_feed_id: String,
    pub enabled: bool,
}

// ── Escalation (#1101) ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum EscalationTier {
    T1,
    T2,
    T3,
}

impl EscalationTier {
    /// Hours before TTL expiry at which this tier triggers.
    pub fn hours_before_expiry(self) -> u32 {
        match self {
            EscalationTier::T1 => 168,
            EscalationTier::T2 => 72,
            EscalationTier::T3 => 24,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EscalationEvent {
    pub id: String,
    pub vault_id: u64,
    pub tier: EscalationTier,
    pub dispatched_at: DateTime<Utc>,
    pub channels: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EscalationState {
    pub vault_id: u64,
    pub last_escalation_tier: Option<EscalationTier>,
    pub escalated_at: Option<DateTime<Utc>>,
}

// ── Vault timeline ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimelineEventKind {
    EscalationSent,
    WebhookDelivered,
    WebhookFailed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineEvent {
    pub id: String,
    pub vault_id: String,
    pub kind: TimelineEventKind,
    pub timestamp: DateTime<Utc>,
    pub description: String,
    pub amount: Option<i128>,
    pub metadata: serde_json::Value,
}

// ── Webhook delivery / retry (#1102) ────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WebhookDeliveryStatus {
    Pending,
    Retrying,
    Delivered,
    DeliveryFailed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookAttempt {
    pub attempted_at: DateTime<Utc>,
    pub http_status: u16,
    pub response_body: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookDelivery {
    pub id: String,
    pub vault_id: String,
    pub event_type: String,
    pub payload: serde_json::Value,
    pub endpoint_url: String,
    pub status: WebhookDeliveryStatus,
    pub attempt_count: u32,
    pub next_retry_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub attempts: Vec<WebhookAttempt>,
}

/// Owner-registered endpoint for vault events. An empty `event_types` list
/// subscribes to every event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookSubscription {
    pub id: String,
    pub vault_id: String,
    pub endpoint_url: String,
    pub event_types: Vec<String>,
    pub created_at: DateTime<Utc>,
}

// ── Two-factor authentication (2FA) ─────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TwoFactorMethod {
    Sms,
    Email,
    Totp,
}

#[derive(Debug, Deserialize)]
pub struct Enable2FARequest {
    pub method: TwoFactorMethod,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub otp: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Enable2FAResponse {
    pub vault_id: String,
    pub method: TwoFactorMethod,
    pub secret: Option<String>,
    pub provisioning_uri: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TwoFactorConfig {
    pub vault_id: String,
    pub method: TwoFactorMethod,
    pub enabled: bool,
    pub secret: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub created_at: DateTime<Utc>,
    pub verified_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct TwoFactorStatusResponse {
    pub vault_id: String,
    pub enabled: bool,
    pub method: Option<TwoFactorMethod>,
    pub verified: bool,
    pub phone: Option<String>,
    pub email: Option<String>,
}

// ── Request/response bodies for scenario simulation & sponsored releases ────

#[derive(Debug, Deserialize)]
pub struct ScenarioSimulationRequest {
    pub scenario: String,
    pub params: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct SponsoredReleaseRequest {
    pub vault_id: String,
    pub sponsor_address: String,
}

#[derive(Debug, Serialize)]
pub struct SponsoredReleaseResponse {
    pub release_id: String,
    pub vault_id: String,
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct SponsoredReleaseListResponse {
    pub releases: Vec<serde_json::Value>,
}

// ── Issue #2069: vault full-text search over tags and notes ─────────────────

/// Owner-supplied searchable metadata for a vault (persisted separately from
/// the in-memory `Vault` record). Indexed by an FTS5 virtual table so the
/// `GET /api/vaults/search?q=` endpoint can match against tags and notes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultMetadata {
    pub vault_id: String,
    pub owner: String,
    /// Tag strings (stored as a JSON array in SQLite).
    pub tags: Vec<String>,
    pub notes: String,
    pub updated_at: DateTime<Utc>,
}

/// Body for `PUT /api/vaults/{vault_id}/metadata`.
#[derive(Debug, Deserialize)]
pub struct SetVaultMetadataRequest {
    pub owner: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

/// Query parameters for `GET /api/vaults/search?q=` (full-text search).
#[derive(Debug, Deserialize)]
pub struct VaultSearchQueryParams {
    pub q: String,
    /// Authenticated owner; results are scoped to this owner.
    #[serde(default)]
    pub owner: Option<String>,
    #[serde(default)]
    pub limit: Option<u32>,
    /// Opaque cursor — the offset the next page should start from.
    #[serde(default)]
    pub cursor: Option<String>,
}

/// A single full-text search hit. Live vault fields (`status`, `balance`,
/// `ttl_remaining`) are populated when the vault is present in the in-memory
/// store; they are `None` for metadata-only rows.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultSearchHit {
    pub vault_id: String,
    pub owner: String,
    pub tags: Vec<String>,
    pub notes: String,
    /// Matched-text snippet with `<mark>` highlights (FTS5 `snippet()`).
    pub snippet: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<VaultStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub balance: Option<i128>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl_remaining: Option<u64>,
}

/// Response for `GET /api/vaults/search?q=`.
#[derive(Debug, Serialize)]
pub struct VaultSearchResponse {
    pub query: String,
    pub vaults: Vec<VaultSearchHit>,
    pub next_cursor: Option<String>,
    pub limit: u32,
}

// ── Issue #2068: check-in delegation audit log ──────────────────────────────

/// Kind of check-in delegation event recorded by the TTL watcher.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CheckInDelegationEventKind {
    Grant,
    Revoke,
}

impl CheckInDelegationEventKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Grant => "grant",
            Self::Revoke => "revoke",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "grant" => Self::Grant,
            _ => Self::Revoke,
        }
    }
}

/// Append-only check-in delegation audit entry, populated by the TTL watcher
/// whenever it observes a grant/revoke transition, and exposed via
/// `GET /api/vaults/{vault_id}/delegation-audit`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckInDelegationAuditEntry {
    pub id: i64,
    pub vault_id: String,
    pub event_kind: CheckInDelegationEventKind,
    /// Address delegated to. `None` on revoke.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delegate: Option<String>,
    pub actor: String,
    pub timestamp: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

/// Response for `GET /api/vaults/{vault_id}/delegation-audit`.
#[derive(Debug, Serialize)]
pub struct CheckInDelegationAuditResponse {
    pub vault_id: String,
    pub entries: Vec<CheckInDelegationAuditEntry>,
}

/// Current check-in delegation state for a vault (the source of truth the
/// watcher diffs against its last-seen state to emit audit events).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckInDelegationState {
    pub vault_id: String,
    pub owner: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delegate: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub granted_at: Option<DateTime<Utc>>,
}

/// Body for `POST /api/vaults/{vault_id}/delegation` (grant).
#[derive(Debug, Deserialize)]
pub struct GrantCheckInDelegationRequest {
    pub owner: String,
    pub delegate: String,
}

// ── Issue #2070: multi-party withdrawal approval workflow ───────────────────

/// Default approval TTL (48h) when a request omits `approval_ttl_seconds`.
pub const DEFAULT_WITHDRAWAL_APPROVAL_TTL_SECS: u64 = 48 * 3_600;

/// Lifecycle status of a withdrawal approval workflow.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum WithdrawalApprovalStatus {
    /// Awaiting the configured approvals; auto-cancels at `expires_at`.
    Pending,
    /// All required approvers have approved; the withdrawal may execute.
    Approved,
    /// Owner/canceller explicitly cancelled the pending request.
    Cancelled,
    /// The approval TTL elapsed before all approvals were collected.
    Expired,
    /// The withdrawal was executed on-chain.
    Executed,
}

impl WithdrawalApprovalStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Approved => "approved",
            Self::Cancelled => "cancelled",
            Self::Expired => "expired",
            Self::Executed => "executed",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "approved" => Self::Approved,
            "cancelled" => Self::Cancelled,
            "expired" => Self::Expired,
            "executed" => Self::Executed,
            _ => Self::Pending,
        }
    }
}

impl std::fmt::Display for WithdrawalApprovalStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// A single recorded approval for a withdrawal workflow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WithdrawalApproval {
    pub withdrawal_id: String,
    pub approver: String,
    pub approved_at: DateTime<Utc>,
}

/// A multi-party withdrawal approval workflow for a high-value vault.
///
/// `required_approvers` is the set of beneficiaries / designated signers whose
/// approval is required; the workflow becomes `Approved` once every one of
/// them has approved. A pending request auto-cancels (`Expired`) once
/// `approval_ttl_seconds` elapses (checked lazily on read and swept by a
/// background watcher).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WithdrawalApprovalWorkflow {
    pub vault_id: String,
    pub withdrawal_id: String,
    /// Amount requested for withdrawal (stroops / base units).
    pub amount: i128,
    /// Address that initiated the withdrawal request.
    pub requested_by: String,
    /// Approvers (a subset of beneficiaries or designated signers) whose
    /// approval is required before the withdrawal may execute.
    pub required_approvers: Vec<String>,
    pub approval_ttl_seconds: u64,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub status: WithdrawalApprovalStatus,
    pub approvals: Vec<WithdrawalApproval>,
    /// On-chain transaction hash once the withdrawal has executed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub executed_tx_hash: Option<String>,
}

/// Body for `POST /api/vaults/{vault_id}/withdrawal-approvals`.
#[derive(Debug, Deserialize)]
pub struct CreateWithdrawalApprovalRequest {
    pub requested_by: String,
    pub amount: i128,
    pub required_approvers: Vec<String>,
    /// Seconds before a pending request auto-cancels (default 48h).
    #[serde(default)]
    pub approval_ttl_seconds: Option<u64>,
}

/// Body for `POST /api/vaults/{vault_id}/withdrawal-approvals/{withdrawal_id}/approvals`.
#[derive(Debug, Deserialize)]
pub struct SubmitWithdrawalApprovalRequest {
    pub approver: String,
}

/// Response for `GET /api/vaults/{vault_id}/withdrawal-approvals`.
#[derive(Debug, Serialize)]
pub struct WithdrawalApprovalListResponse {
    pub vault_id: String,
    pub workflows: Vec<WithdrawalApprovalWorkflow>,
}
