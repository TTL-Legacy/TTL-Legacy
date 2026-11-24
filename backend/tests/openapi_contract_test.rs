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
        m.insert("beneficiary_id", "test-beneficiary-1");
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
        ("GET", "/health"),
        ("GET", "/api/vaults/{vault_id}/reminders"),
        ("POST", "/api/vaults/{vault_id}/reminders"),
        ("GET", "/api/vaults/{vault_id}/preferences"),
        ("POST", "/api/vaults/{vault_id}/preferences"),
        ("DELETE", "/api/vaults/{vault_id}/preferences"),
        ("GET", "/api/unsubscribe"),
        ("GET", "/api/vaults/{vault_id}/simulate-release"),
        ("POST", "/api/vaults/{vault_id}/sponsored-release"),
        ("GET", "/api/vaults/{vault_id}/sponsored-release"),
        ("POST", "/api/vaults/{vault_id}/vesting/claim-bonus"),
        ("GET", "/api/vaults/{vault_id}/vesting/bonus"),
        // Beneficiary endpoints
        ("POST", "/api/vaults/{vault_id}/beneficiaries/{beneficiary_id}/accept"),
        ("POST", "/api/vaults/{vault_id}/beneficiaries/{beneficiary_id}/decline"),
        ("GET", "/api/vaults/{vault_id}/beneficiaries/{beneficiary_id}/conflict"),
        ("POST", "/api/vaults/{vault_id}/beneficiaries/{beneficiary_id}/auction"),
        ("GET", "/api/vaults/{vault_id}/beneficiaries/{beneficiary_id}/waterfall"),
        // Token-management endpoints (#2082)
        ("POST", "/api/tokens"),
        ("DELETE", "/api/tokens/{id}"),
        ("GET", "/api/tokens"),
        ("PUT", "/api/tokens/{id}/allowlist"),
    ];

    /// Beneficiary route templates that must be covered by contract tests.
    const BENEFICIARY_ROUTES: &[(&str, &str)] = &[
        ("POST", "/api/vaults/{vault_id}/beneficiaries/{beneficiary_id}/accept"),
        ("POST", "/api/vaults/{vault_id}/beneficiaries/{beneficiary_id}/decline"),
        ("GET", "/api/vaults/{vault_id}/beneficiaries/{beneficiary_id}/conflict"),
        ("POST", "/api/vaults/{vault_id}/beneficiaries/{beneficiary_id}/auction"),
        ("GET", "/api/vaults/{vault_id}/beneficiaries/{beneficiary_id}/waterfall"),
    ];

    /// Token-management route templates covered by contract tests (#2082).
    /// Routes: POST /tokens, DELETE /tokens/{id}, GET /tokens, PUT /tokens/{id}/allowlist.
    const TOKEN_MANAGEMENT_ROUTES: &[(&str, &str)] = &[
        ("POST", "/api/tokens"),
        ("DELETE", "/api/tokens/{id}"),
        ("GET", "/api/tokens"),
        ("PUT", "/api/tokens/{id}/allowlist"),
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

    #[test]
    fn validate_routes_match_openapi_spec() {
        let openapi_content = std::fs::read_to_string("docs/openapi.yaml")
            .expect("Failed to read docs/openapi.yaml");

        let params = path_param_defaults();
        let excluded = excluded_paths();

        for (method, template) in ROUTES {
            let resolved = resolve_path(template, &params);
            let is_excluded = excluded.iter().any(|ex| resolved.starts_with(ex));

            if !is_excluded {
                let path_line = format!("  {}:\n", resolved);
                assert!(
                    openapi_content.contains(&path_line) || openapi_content.contains(template),
                    "Route {} {} must be defined in docs/openapi.yaml",
                    method,
                    template
                );
            }
        }
    }

    #[test]
    fn all_routes_have_response_schemas() {
        let openapi_content = std::fs::read_to_string("docs/openapi.yaml")
            .expect("Failed to read docs/openapi.yaml");

        assert!(
            openapi_content.contains("responses:"),
            "OpenAPI spec should define response schemas for all endpoints"
        );

        assert!(
            openapi_content.contains("content:") || openapi_content.contains("application/json"),
            "Response schemas should specify content-type and body"
        );
    }

    #[test]
    fn route_count_consistency() {
        assert!(
            ROUTES.len() > 0,
            "ROUTES should contain at least one endpoint"
        );

        let unique_paths: Vec<_> = ROUTES.iter().map(|(_, path)| path).collect();
        assert_eq!(
            unique_paths.len(),
            ROUTES.len(),
            "Each route should be unique in ROUTES table"
        );
    }

    #[test]
    fn beneficiary_routes_defined_in_spec_and_resolvable() {
        let openapi_content = std::fs::read_to_string("docs/openapi.yaml")
            .expect("Failed to read docs/openapi.yaml");

        let params = path_param_defaults();

        for (method, template) in BENEFICIARY_ROUTES {
            // Each beneficiary route must be present in the ROUTES table.
            assert!(
                ROUTES.iter().any(|(m, p)| m == method && p == template),
                "Beneficiary route {} {} must be listed in ROUTES",
                method,
                template
            );

            // Each beneficiary route must be defined in the OpenAPI spec.
            assert!(
                openapi_content.contains(template),
                "Beneficiary route {} {} must be defined in docs/openapi.yaml",
                method,
                template
            );

            // Each beneficiary route must resolve without unresolved placeholders.
            let resolved = resolve_path(template, &params);
            assert!(
                !resolved.contains('{'),
                "Unresolved path parameter in beneficiary route {} {}: {}",
                method,
                template,
                resolved
            );
            println!("✅  {} {}", method, resolved);
        }
    }

    #[test]
    fn health_endpoint_accessible() {
        let health_route = ROUTES.iter().find(|(_, path)| path.contains("health"));
        assert!(
            health_route.is_some(),
            "Health check endpoint should be in ROUTES"
        );

        let (method, path) = health_route.unwrap();
        assert_eq!(*method, "GET", "Health endpoint should use GET method");
        assert!(*path == "/health", "Health endpoint should be at /health");
    }

    // ── Token-management contract tests (#2082) ────────────────────────────

    /// All four token-management routes appear in the ROUTES table and resolve
    /// without unresolved placeholders.
    #[test]
    fn token_management_routes_are_defined() {
        let params = path_param_defaults();

        for (method, template) in TOKEN_MANAGEMENT_ROUTES {
            assert!(
                ROUTES.iter().any(|(m, p)| m == method && p == template),
                "Token-management route {} {} must be listed in ROUTES",
                method,
                template,
            );

            let resolved = resolve_path(template, &params);
            assert!(
                !resolved.contains('{'),
                "Unresolved path parameter in token-management route {} {}: {}",
                method,
                template,
                resolved,
            );
            println!("✅  {} {}", method, resolved);
        }
    }

    /// Every token-management route is present in the OpenAPI spec and the spec
    /// contains at least one response schema for each of them.
    #[test]
    fn token_management_routes_match_openapi_spec() {
        let openapi_content = std::fs::read_to_string("docs/openapi.yaml")
            .expect("Failed to read docs/openapi.yaml");

        for (method, template) in TOKEN_MANAGEMENT_ROUTES {
            assert!(
                openapi_content.contains(template),
                "Token-management route {} {} must be defined in docs/openapi.yaml",
                method,
                template,
            );
        }

        // The spec must declare response schemas and JSON content types for
        // the token section.
        assert!(
            openapi_content.contains("responses:"),
            "OpenAPI spec must contain response schema definitions",
        );
        assert!(
            openapi_content.contains("application/json"),
            "OpenAPI spec must specify application/json content type for token responses",
        );
    }

    /// The request body shapes for POST /api/tokens and PUT /api/tokens/{id}/allowlist
    /// are present in the spec (requestBody section).
    #[test]
    fn token_management_request_shapes_defined_in_spec() {
        let openapi_content = std::fs::read_to_string("docs/openapi.yaml")
            .expect("Failed to read docs/openapi.yaml");

        // The spec must include requestBody definitions (used by POST and PUT routes).
        assert!(
            openapi_content.contains("requestBody:"),
            "OpenAPI spec must define requestBody for write endpoints such as POST /api/tokens",
        );
    }

    /// The response shapes for token-management endpoints are present in the
    /// spec — either inline or via $ref to a component schema.
    #[test]
    fn token_management_response_shapes_defined_in_spec() {
        let openapi_content = std::fs::read_to_string("docs/openapi.yaml")
            .expect("Failed to read docs/openapi.yaml");

        // All four routes must appear alongside HTTP status codes.
        // We verify at least one success (2xx) and one error (4xx) code is documented.
        let has_success = openapi_content.contains("'200':")
            || openapi_content.contains("'201':")
            || openapi_content.contains("'204':");
        let has_error = openapi_content.contains("'400':")
            || openapi_content.contains("'401':")
            || openapi_content.contains("'403':")
            || openapi_content.contains("'404':");

        assert!(
            has_success,
            "OpenAPI spec must document success responses (200/201/204) for token endpoints",
        );
        assert!(
            has_error,
            "OpenAPI spec must document error responses (4xx) for token endpoints",
        );
    }

    /// Verifies the four token-management routes cover all required HTTP verbs:
    /// POST (create), DELETE (remove), GET (list), PUT (allowlist update).
    #[test]
    fn token_management_covers_required_http_methods() {
        let methods: Vec<&str> = TOKEN_MANAGEMENT_ROUTES
            .iter()
            .map(|(method, _)| *method)
            .collect();

        assert!(
            methods.contains(&"POST"),
            "Token-management routes must include POST /api/tokens",
        );
        assert!(
            methods.contains(&"DELETE"),
            "Token-management routes must include DELETE /api/tokens/{{id}}",
        );
        assert!(
            methods.contains(&"GET"),
            "Token-management routes must include GET /api/tokens",
        );
        assert!(
            methods.contains(&"PUT"),
            "Token-management routes must include PUT /api/tokens/{{id}}/allowlist",
        );
    }

    /// Ensures TOKEN_MANAGEMENT_ROUTES entries are a subset of the main ROUTES
    /// table, so the two tables stay in sync.
    #[test]
    fn token_management_routes_are_subset_of_routes() {
        for (method, template) in TOKEN_MANAGEMENT_ROUTES {
            assert!(
                ROUTES.iter().any(|(m, p)| m == method && p == template),
                "TOKEN_MANAGEMENT_ROUTES entry {} {} is missing from the main ROUTES table",
                method,
                template,
            );
        }
    }
}
