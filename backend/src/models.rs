use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

// ── Notification models ──────────────────────────────────────────────────────

/// Notification type sent to a device.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum NotificationType {
    ExpiryWarning,
    CheckInReminder,
    VaultReleased,
    VaultPaused,
}

/// Delivery status of a single notification attempt.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryStatus {
    Pending,
    Sent,
    Failed,
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

/// Per-owner notification preferences.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationPreferences {
    pub owner: String,
    pub expiry_warning_enabled: bool,
    pub check_in_reminder_enabled: bool,
    pub vault_released_enabled: bool,
    /// Hours before expiry to send the warning (default 24).
    pub warning_hours_before: u64,
}

impl Default for NotificationPreferences {
    fn default() -> Self {
        Self {
            owner: String::new(),
            expiry_warning_enabled: true,
            check_in_reminder_enabled: true,
            vault_released_enabled: true,
            warning_hours_before: 24,
        }
    }
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationPreferences {
    pub vault_id: String,
    pub channels: Vec<NotificationChannel>,
    pub frequency: NotificationFrequency,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NotificationPreferencesRequest {
    pub channels: Vec<NotificationChannel>,
    pub frequency: NotificationFrequency,
}
