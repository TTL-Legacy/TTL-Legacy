/// Topic string emitted when a vault is archived — Issue #2048.
pub const ARCHIVE_TOPIC: &str = "vault.archived";

/// Emit a tracing event at info level for a vault archival.
///
/// Fields emitted:
/// - `vault_id`: the ID of the vault being archived
/// - `owner`: the owner address / identifier
/// - `topic`: always `ARCHIVE_TOPIC` ("vault.archived")
pub fn emit_archive(vault_id: &str, owner: &str) {
    tracing::info!(
        vault_id = vault_id,
        owner = owner,
        topic = ARCHIVE_TOPIC,
        "vault archived",
    );
}
