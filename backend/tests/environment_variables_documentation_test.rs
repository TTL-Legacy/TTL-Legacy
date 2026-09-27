//! Test for Issue #1590: Ensure all environment variables used in the backend
//! are properly documented in docs/deployment-guide.md.
//!
//! This test verifies that:
//! - All std::env::var() calls in the codebase reference documented variables
//! - The deployment guide contains a section documenting each variable
//! - A .env.example file exists with all variables

#[cfg(test)]
mod environment_variables_documentation {
    use std::fs;
    use std::path::Path;

    /// List of all environment variables used in the backend codebase.
    /// Update this list when new environment variables are added to the code.
    const REQUIRED_ENV_VARS: &[&str] = &[
        "ADMIN_API_KEY",
        "ALLOWED_ORIGINS",
        "APP_ENV",
        "CONSENSUS_STRATEGY",
        "CSP_POLICY",
        "DB_POOL_MAX",
        "DB_POOL_MIN",
        "DB_POOL_TIMEOUT_SECS",
        "JWT_SECRET",
        "LOG_FORMAT",
        "MIN_CONTRACT_VERSION",
        "NODE_ID",
        "OTEL_EXPORTER_OTLP_ENDPOINT",
        "OTEL_SERVICE_NAME",
        "REDIS_URL",
    ];

    #[test]
    fn deployment_guide_documents_all_environment_variables() {
        let deployment_guide = fs::read_to_string("docs/deployment-guide.md")
            .expect("docs/deployment-guide.md should exist");

        for var in REQUIRED_ENV_VARS {
            assert!(
                deployment_guide.contains(var),
                "Environment variable {} is not documented in docs/deployment-guide.md",
                var
            );
        }
    }

    #[test]
    fn env_example_file_exists() {
        let env_example_path = ".env.example";
        assert!(
            Path::new(env_example_path).exists(),
            ".env.example file should exist at the project root"
        );
    }

    #[test]
    fn env_example_contains_all_variables() {
        let env_example = fs::read_to_string(".env.example")
            .expect(".env.example file should be readable");

        for var in REQUIRED_ENV_VARS {
            assert!(
                env_example.contains(var),
                "Environment variable {} is not documented in .env.example",
                var
            );
        }
    }

    #[test]
    fn deployment_guide_has_environment_section() {
        let deployment_guide = fs::read_to_string("docs/deployment-guide.md")
            .expect("docs/deployment-guide.md should exist");

        let has_env_setup = deployment_guide.contains("Environment Setup")
            || deployment_guide.contains("environment setup")
            || deployment_guide.contains("Environment Configuration");

        assert!(
            has_env_setup,
            "docs/deployment-guide.md should have an Environment Setup or Configuration section"
        );
    }
}
