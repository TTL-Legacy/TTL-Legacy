/// Contract version verification module.
///
/// Verifies the deployed Soroban contract's version meets the minimum required
/// by this backend build. Called once on startup, before the server
/// begins accepting requests.
///
/// This module defines the version check logic and result structure. The actual
/// contract interaction (RPC call to get_contract_version) is mocked in tests
/// and implemented via the contract client in production.

use std::fmt;

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
        Ok(version) => VersionCheckResult {
            compatible: version >= min_required_version,
            contract_version: Some(version),
            min_required_version,
            error: None,
        },
        Err(e) => VersionCheckResult {
            compatible: false,
            contract_version: None,
            min_required_version,
            error: Some(format!("Unable to reach contract to verify version: {}", e)),
        },
    }
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

        let result = check_contract_version(
            || async { Ok(contract_version) },
            min_required,
        )
        .await;

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

        let result = check_contract_version(
            || async { Ok(contract_version) },
            min_required,
        )
        .await;

        assert!(result.compatible);
        assert_eq!(result.contract_version, Some(1));
        assert_eq!(result.min_required_version, 1);
        assert!(result.error.is_none());
    }

    // c) returns_compatible_false_when_version_below_minimum
    #[tokio::test]
    async fn returns_compatible_false_when_version_below_minimum() {
        let min_required = 2u32;
        let contract_version = 1u32;

        let result = check_contract_version(
            || async { Ok(contract_version) },
            min_required,
        )
        .await;

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
        assert!(result.contract_version.is_none());
        assert_eq!(result.min_required_version, 1);
        assert!(result.error.is_some());
        assert!(result.error.as_ref().unwrap().contains("Unable to reach contract"));
    }

    // e) min_contract_version_parses_from_env
    #[test]
    fn min_contract_version_parses_from_env() {
        let min_version = parse_min_contract_version(Some("3".to_string()));
        assert_eq!(min_version, 3);
    }

    // f) min_contract_version_defaults_to_1_when_unset
    #[test]
    fn min_contract_version_defaults_to_1_when_unset() {
        let min_version_none = parse_min_contract_version(None);
        assert_eq!(min_version_none, 1);

        let min_version_empty = parse_min_contract_version(Some(String::new()));
        assert_eq!(min_version_empty, 1);
    }

    // g) version_check_result_display_shows_error_when_unreachable
    #[test]
    fn version_check_result_display_shows_error_when_unreachable() {
        let result = VersionCheckResult {
            compatible: false,
            contract_version: None,
            min_required_version: 1,
            error: Some("connection refused".to_string()),
        };
        let display = result.to_string();
        assert!(display.contains("Version check error"));
        assert!(display.contains("connection refused"));
    }

    // h) version_check_result_display_shows_versions_when_compatible
    #[test]
    fn version_check_result_display_shows_versions_when_compatible() {
        let result = VersionCheckResult {
            compatible: true,
            contract_version: Some(2),
            min_required_version: 1,
            error: None,
        };
        let display = result.to_string();
        assert!(display.contains("Contract version 2"));
        assert!(display.contains("minimum required: 1"));
    }
}
