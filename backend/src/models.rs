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

/// Persisted reminder preferences stored by `Db`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReminderPreferences {
    pub vault_id: u64,
    pub channels: Vec<Channel>,
    pub hours_before_expiry: u32,
    pub frequency: Frequency,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deleted_at: Option<DateTime<Utc>>,
}

/// Request body for setting reminder preferences.
#[derive(Debug, Deserialize, Clone)]
pub struct SetPreferencesRequest {
    pub channels: Vec<Channel>,
    pub hours_before_expiry: u32,
    pub frequency: Frequency,
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
    pub page: Option<u32>,
    pub limit: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchResult {
    pub vaults: Vec<Vault>,
    pub total: u32,
    pub page: u32,
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
    /// Optional memo for the claim transaction.
    pub memo: Option<String>,
}

/// Response body for POST /api/vaults/{id}/vesting/claim-bonus.
#[derive(Debug, Serialize)]
pub struct ClaimBonusResponse {
    pub vault_id: String,
    pub claimed_amount: i128,
    pub bonus_amount: i128,
    pub transaction_hash: String,
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
