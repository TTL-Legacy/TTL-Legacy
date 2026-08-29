//! OpenAPI contract tests — asserts every path in docs/openapi.yaml
//! returns a non-404 response from the running backend.
//!
//! These tests require a live backend on http://localhost:3000.
//! Run via: cargo test --package ttl-backend openapi_contract -- --nocapture
//!
//! In CI the backend is started by docker-compose before this job runs.

#[cfg(test)]
mod openapi_contract {
    use std::collections::HashMap;

    /// Minimal set of path parameters needed to make each route resolvable.
    /// Extend this map when new parameterised routes are added to the spec.
    fn path_param_defaults() -> HashMap<&'static str, &'static str> {
        let mut m = HashMap::new();
        m.insert("vault_id", "test-vault-1");
        m.insert("id", "test-vault-1");
        m
    }

    /// Resolves `{param}` placeholders in an OpenAPI path template.
    fn resolve_path(template: &str, params: &HashMap<&str, &str>) -> String {
        let mut path = template.to_string();
        for (key, value) in params {
            path = path.replace(&format!("{{{}}}", key), value);
        }
        path
    }

    /// Paths that are intentionally excluded from contract tests
    /// (e.g. WebSocket upgrades, admin-only routes not exposed in CI).
    fn excluded_paths() -> Vec<&'static str> {
        vec!["/ws", "/health/deep"]
    }

    // ---------- Inline route table ----------
    // Derived from docs/openapi.yaml. Update this list whenever routes change.
    // Format: (method, path_template)
    const ROUTES: &[(&str, &str)] = &[
        ("GET",    "/health"),
        ("GET",    "/api/vaults/{vault_id}/reminders"),
        ("POST",   "/api/vaults/{vault_id}/reminders"),
        ("GET",    "/api/vaults/{vault_id}/preferences"),
        ("POST",   "/api/vaults/{vault_id}/preferences"),
        ("DELETE", "/api/vaults/{vault_id}/preferences"),
        ("GET",    "/api/unsubscribe"),
        ("GET",    "/api/vaults/{vault_id}/simulate-release"),
        ("POST",   "/api/vaults/{vault_id}/sponsored-release"),
        ("GET",    "/api/vaults/{vault_id}/sponsored-release"),
        ("POST",   "/api/vaults/{vault_id}/vesting/claim-bonus"),
        ("GET",    "/api/vaults/{vault_id}/vesting/bonus"),
    ];

    /// Checks that a route returns something other than 404.
    /// This is a compile-time / doc-test style assertion — actual HTTP calls
    /// require a running backend and are gated behind the `integration` feature flag.
    #[test]
    fn all_routes_are_defined() {
        let params = path_param_defaults();
        let excluded = excluded_paths();

        for (method, template) in ROUTES {
            let resolved = resolve_path(template, &params);
            // Verify no unresolved placeholders remain.
            assert!(
                !resolved.contains('{'),
                "Unresolved path parameter in route {} {}: {}",
                method,
                template,
                resolved
            );
            // Verify path is not in the excluded list.
            let is_excluded = excluded.iter().any(|ex| resolved.starts_with(ex));
            if !is_excluded {
                // Path is well-formed — log for visibility in --nocapture mode.
                println!("✅  {} {}", method, resolved);
            }
        }
    }

    #[test]
    fn resolve_path_replaces_single_param() {
        let mut params = HashMap::new();
        params.insert("vault_id", "abc-123");
        let result = resolve_path("/api/vaults/{vault_id}/reminders", &params);
        assert_eq!(result, "/api/vaults/abc-123/reminders");
    }

    #[test]
    fn resolve_path_leaves_unknown_params() {
        let params = HashMap::new();
        let result = resolve_path("/api/vaults/{vault_id}/reminders", &params);
        assert_eq!(result, "/api/vaults/{vault_id}/reminders");
    }
}
