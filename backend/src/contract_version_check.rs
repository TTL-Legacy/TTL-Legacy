/// Contract version verification module.
///
/// Verifies the deployed Soroban contract's version meets the minimum required
/// by this backend build. Called once on startup, before the server
/// begins accepting requests.
///
/// This module defines the version check logic and result structure. The actual
/// contract interaction (RPC call to get_contract_version) is implemented via
/// `fetch_contract_version_via_rpc`, while the check itself accepts an injectable
/// closure so tests can substitute fakes.
///
/// It also provides an upgrade dry-run storage compatibility check: a snapshot
/// of the storage key schema per contract version, plus a check that verifies a
/// previous-version state snapshot still deserializes under the current schema.
use std::collections::BTreeMap;
use std::fmt;
use std::time::Duration;

/// Default timeout applied to the Soroban RPC call when fetching the contract
/// version. Kept short so a hung/unreachable endpoint fails fast at startup.
pub const DEFAULT_RPC_TIMEOUT: Duration = Duration::from_secs(10);

/// Result of a contract version compatibility check.
#[derive(Debug, Clone)]
pub struct VersionCheckResult {
    /// Whether the contract version meets the minimum requirement.
    pub compatible: bool,
    /// The contract version returned by get_contract_version(), if successful.
    pub contract_version: Option<u32>,
    /// The minimum version required by this backend build.
    pub min_required_version: u32,
    /// Error message if the check failed (e.g., contract unreachable).
    pub error: Option<String>,
}

impl fmt::Display for VersionCheckResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(err) = &self.error {
            write!(f, "Version check error: {}", err)
        } else if let Some(version) = self.contract_version {
            write!(
                f,
                "Contract version {} (minimum required: {})",
                version, self.min_required_version
            )
        } else {
            write!(f, "Version check inconclusive")
        }
    }
}

/// A snapshot of the storage key schema for a single contract version.
///
/// Each entry maps a logical storage key to the type name expected to be
/// stored under it. Comparing snapshots across versions lets an upgrade
/// dry-run detect layout changes that would break deserialization of
/// previously persisted state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageSchemaSnapshot {
    /// The contract version this snapshot describes.
    pub version: u32,
    /// Logical storage key -> expected value type name.
    pub keys: BTreeMap<String, String>,
}

impl StorageSchemaSnapshot {
    /// Build a snapshot from an iterator of (key, type) pairs.
    pub fn new<I, K, V>(version: u32, entries: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        Self {
            version,
            keys: entries
                .into_iter()
                .map(|(k, v)| (k.into(), v.into()))
                .collect(),
        }
    }
}

/// Result of an upgrade dry-run storage compatibility check.
#[derive(Debug, Clone)]
pub struct StorageCompatibilityResult {
    /// Whether the previous-version state is compatible with the current schema.
    pub compatible: bool,
    /// The previous contract version that was checked.
    pub from_version: u32,
    /// The target contract version being upgraded to.
    pub to_version: u32,
    /// Storage keys present in the previous schema but missing from the current
    /// one. These would be silently dropped or fail to deserialize.
    pub removed_keys: Vec<String>,
    /// Storage keys whose value type changed between versions. These would fail
    /// to deserialize under the current schema.
    pub type_changed_keys: Vec<String>,
    /// Error message if the check could not be performed.
    pub error: Option<String>,
}

impl fmt::Display for StorageCompatibilityResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(err) = &self.error {
            return write!(f, "Storage compatibility check error: {}", err);
        }
        if self.compatible {
            write!(
                f,
                "Storage compatible: v{} -> v{}",
                self.from_version, self.to_version
            )
        } else {
            write!(
                f,
                "Storage incompatible: v{} -> v{} (removed: {:?}, type changed: {:?})",
                self.from_version, self.to_version, self.removed_keys, self.type_changed_keys
            )
        }
    }
}

/// Perform an upgrade dry-run storage compatibility check between two schema
/// snapshots.
///
/// This never touches live state: it compares the previous version's storage
/// key schema against the current one and reports keys that were removed or
/// whose value type changed. A non-empty report means an upgrade would break
/// deserialization of previously persisted state.
pub fn check_storage_compatibility(
    previous: &StorageSchemaSnapshot,
    current: &StorageSchemaSnapshot,
) -> StorageCompatibilityResult {
    let mut removed_keys = Vec::new();
    let mut type_changed_keys = Vec::new();

    for (key, prev_type) in &previous.keys {
        match current.keys.get(key) {
            None => removed_keys.push(key.clone()),
            Some(cur_type) if cur_type != prev_type => type_changed_keys.push(key.clone()),
            Some(_) => {}
        }
    }

    StorageCompatibilityResult {
        compatible: removed_keys.is_empty() && type_changed_keys.is_empty(),
        from_version: previous.version,
        to_version: current.version,
        removed_keys,
        type_changed_keys,
        error: None,
    }
}

/// Verify that a previous-version state snapshot still deserializes under the
/// current storage schema.
///
/// `state` is the raw JSON state captured from the previous contract version.
/// The check confirms it parses as a JSON object and that every key it contains
/// is still present in `current`. This is the deserialization half of the
/// upgrade dry-run: schema comparison catches structural drift, while this
/// catches state that the current schema can no longer read.
pub fn check_previous_state_deserializes(
    previous: &StorageSchemaSnapshot,
    current: &StorageSchemaSnapshot,
    state: &str,
) -> StorageCompatibilityResult {
    let parsed: serde_json::Value = match serde_json::from_str(state) {
        Ok(v) => v,
        Err(e) => {
            return StorageCompatibilityResult {
                compatible: false,
                from_version: previous.version,
                to_version: current.version,
                removed_keys: Vec::new(),
                type_changed_keys: Vec::new(),
                error: Some(format!("previous-version state is not valid JSON: {}", e)),
            };
        }
    };

    let obj = match parsed.as_object() {
        Some(o) => o,
        None => {
            return StorageCompatibilityResult {
                compatible: false,
                from_version: previous.version,
                to_version: current.version,
                removed_keys: Vec::new(),
                type_changed_keys: Vec::new(),
                error: Some("previous-version state is not a JSON object".to_string()),
            };
        }
    };

    let mut result = check_storage_compatibility(previous, current);
    for key in obj.keys() {
        if !current.keys.contains_key(key) && !result.removed_keys.contains(key) {
            result.removed_keys.push(key.clone());
        }
    }
    result.compatible = result.removed_keys.is_empty() && result.type_changed_keys.is_empty();
    result
}

/// Calls get_contract_version on the configured Soroban contract and
/// compares it against min_required_version. Never panics — returns
/// a result so the caller decides whether to exit.
///
/// # Arguments
/// * `get_version_fn` - A closure that calls get_contract_version on the contract.
///   This allows for easy mocking in tests.
/// * `min_required_version` - The minimum contract version this backend requires.
///
/// # Returns
/// A `VersionCheckResult` containing the compatibility status, contract version,
/// and any error that occurred during the check.
pub async fn check_contract_version<F, Fut>(
    get_version_fn: F,
    min_required_version: u32,
) -> VersionCheckResult
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<u32, String>>,
{
    match get_version_fn().await {
        Ok(version) => {
            if version > min_required_version {
                tracing::warn!(
                    "Deployed contract version {} is higher than backend minimum {}. Accepting with warning.",
                    version,
                    min_required_version
                );
            }
            VersionCheckResult {
                compatible: version >= min_required_version,
                contract_version: Some(version),
                min_required_version,
                error: None,
            }
        }
        Err(e) => VersionCheckResult {
            compatible: false,
            contract_version: None,
            min_required_version,
            error: Some(format!("Unable to reach contract to verify version: {}", e)),
        },
    }
}

/// Fetch the deployed contract's version by invoking `get_contract_version`
/// over Soroban RPC.
///
/// This is the production implementation of the closure passed to
/// [`check_contract_version`]. It performs a real RPC round-trip against
/// `rpc_url` for `contract_id`, applying `timeout` so an unreachable or
/// unresponsive endpoint fails fast with a clear error instead of hanging
/// startup indefinitely.
///
/// # Arguments
/// * `rpc_url` - The Soroban RPC endpoint (e.g. `http://localhost:8000/soroban/rpc`).
/// * `contract_id` - The deployed contract's identifier.
/// * `timeout` - Maximum time to wait for the RPC response.
///
/// # Returns
/// `Ok(version)` on success, or `Err(message)` describing the failure
/// (timeout, transport error, or malformed response).
pub async fn fetch_contract_version_via_rpc(
    rpc_url: &str,
    contract_id: &str,
    timeout: Duration,
) -> Result<u32, String> {
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "getContractVersion",
        "params": {
            "contractId": contract_id,
        }
    });

    let client = reqwest::Client::builder()
        .timeout(timeout)
        .build()
        .map_err(|e| format!("failed to build RPC client: {}", e))?;

    let response = client
        .post(rpc_url)
        .json(&request)
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                format!(
                    "Soroban RPC request to {} timed out after {:?}",
                    rpc_url, timeout
                )
            } else {
                format!("Soroban RPC request to {} failed: {}", rpc_url, e)
            }
        })?;

    if !response.status().is_success() {
        return Err(format!(
            "Soroban RPC returned HTTP {} from {}",
            response.status(),
            rpc_url
        ));
    }

    let body: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("failed to parse Soroban RPC response: {}", e))?;

    if let Some(err) = body.get("error") {
        return Err(format!("Soroban RPC error: {}", err));
    }

    body.get("result")
        .and_then(|r| r.get("version"))
        .and_then(|v| v.as_u64())
        .map(|v| v as u32)
        .ok_or_else(|| {
            "Soroban RPC response missing result.version".to_string()
        })
}

/// Parse MIN_CONTRACT_VERSION from environment variables.
///
/// # Arguments
/// * `env_var_value` - The value of the MIN_CONTRACT_VERSION environment variable.
///
/// # Returns
/// The parsed u32 value, or the default of 1 if the variable is not set or empty.
///
/// # Panics
/// Panics if the value is set but cannot be parsed as a valid u32.
pub fn parse_min_contract_version(env_var_value: Option<String>) -> u32 {
    match env_var_value {
        None | Some(ref s) if s.is_empty() => {
            tracing::debug!("MIN_CONTRACT_VERSION not set, using default of 1");
            1
        }
        Some(s) => s
            .parse::<u32>()
            .expect("MIN_CONTRACT_VERSION must be a valid u32 integer"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // a) returns_compatible_true_when_version_meets_minimum
    #[tokio::test]
    async fn returns_compatible_true_when_version_meets_minimum() {
        let min_required = 1u32;
        let contract_version = 2u32;

        let result = check_contract_version(|| async { Ok(contract_version) }, min_required).await;

        assert!(result.compatible);
        assert_eq!(result.contract_version, Some(2));
        assert_eq!(result.min_required_version, 1);
        assert!(result.error.is_none());
    }

    // b) returns_compatible_true_when_version_exactly_equals_minimum
    #[tokio::test]
    async fn returns_compatible_true_when_version_exactly_equals_minimum() {
        let min_required = 1u32;
        let contract_version = 1u32;

        let result = check_contract_version(|| async { Ok(contract_version) }, min_required).await;

        assert!(result.compatible);
        assert_eq!(result.contract_version, Some(1));
        assert_eq!(result.min_required_version, 1);
        assert!(result.error.is_none());
    }

    // c) returns_compatible_true_with_warning_when_version_above_minimum
    #[tokio::test]
    async fn returns_compatible_true_with_warning_when_version_above_minimum() {
        let min_required = 1u32;
        let contract_version = 5u32;

        let result = check_contract_version(|| async { Ok(contract_version) }, min_required).await;

        assert!(result.compatible);
        assert_eq!(result.contract_version, Some(5));
        assert_eq!(result.min_required_version, 1);
        assert!(result.error.is_none());
    }

    // c) returns_compatible_false_when_version_below_minimum
    #[tokio::test]
    async fn returns_compatible_false_when_version_below_minimum() {
        let min_required = 2u32;
        let contract_version = 1u32;

        let result = check_contract_version(|| async { Ok(contract_version) }, min_required).await;

        assert!(!result.compatible);
        assert_eq!(result.contract_version, Some(1));
        assert_eq!(result.min_required_version, 2);
        assert!(result.error.is_none());
    }

    // d) returns_error_when_contract_unreachable
    #[tokio::test]
    async fn returns_error_when_contract_unreachable() {
        let min_required = 1u32;

        let result = check_contract_version(
            || async { Err("connection refused".to_string()) },
            min_required,
        )
        .await;

        assert!(!result.compatible);
        assert_eq!(result.contract_version, None);
        assert_eq!(result.min_required_version, 1);
        assert!(result.error.is_some());
    }

    // e) storage_compatibility_passes_when_schema_unchanged
    #[test]
    fn storage_compatibility_passes_when_schema_unchanged() {
        let v1 = StorageSchemaSnapshot::new(1, [("Admin", "Address"), ("Total", "i128")]);
        let v2 = StorageSchemaSnapshot::new(2, [("Admin", "Address"), ("Total", "i128")]);

        let result = check_storage_compatibility(&v1, &v2);

        assert!(result.compatible);
        assert!(result.removed_keys.is_empty());
        assert!(result.type_changed_keys.is_empty());
    }

    // f) storage_compatibility_flags_removed_and_type_changed_keys
    #[test]
    fn storage_compatibility_flags_removed_and_type_changed_keys() {
        let v1 = StorageSchemaSnapshot::new(
            1,
            [("Admin", "Address"), ("Total", "i128"), ("Legacy", "u32")],
        );
        let v2 = StorageSchemaSnapshot::new(2, [("Admin", "Address"), ("Total", "u64")]);

        let result = check_storage_compatibility(&v1, &v2);

        assert!(!result.compatible);
        assert_eq!(result.removed_keys, vec!["Legacy".to_string()]);
        assert_eq!(result.type_changed_keys, vec!["Total".to_string()]);
    }

    // g) previous_version_state_deserializes_under_current_schema
    #[test]
    fn previous_version_state_deserializes_under_current_schema() {
        let v1 = StorageSchemaSnapshot::new(1, [("Admin", "Address"), ("Total", "i128")]);
        let v2 = StorageSchemaSnapshot::new(2, [("Admin", "Address"), ("Total", "i128")]);
        let previous_state = r#"{"Admin":"GABC","Total":42}"#;

        let result = check_previous_state_deserializes(&v1, &v2, previous_state);

        assert!(result.compatible);
        assert!(result.error.is_none());
    }

    // h) previous_version_state_fails_when_key_dropped
    #[test]
    fn previous_version_state_fails_when_key_dropped() {
        let v1 = StorageSchemaSnapshot::new(1, [("Admin", "Address"), ("Total", "i128")]);
        let v2 = StorageSchemaSnapshot::new(2, [("Admin", "Address")]);
        let previous_state = r#"{"Admin":"GABC","Total":42}"#;

        let result = check_previous_state_deserializes(&v1, &v2, previous_state);

        assert!(!result.compatible);
        assert!(result.removed_keys.contains(&"Total".to_string()));
    }

    // i) previous_version_state_fails_on_invalid_json
    #[test]
    fn previous_version_state_fails_on_invalid_json() {
        let v1 = StorageSchemaSnapshot::new(1, [("Admin", "Address")]);
        let v2 = StorageSchemaSnapshot::new(2, [("Admin", "Address")]);

        let result = check_previous_state_deserializes(&v1, &v2, "not json");

        assert!(!result.compatible);
        assert!(result.error.is_some());
    }
}
