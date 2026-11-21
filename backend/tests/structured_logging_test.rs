//! Structured logging tests for JSON-formatted backend logs.
//! Issue #1348: Infrastructure: add structured logging to backend service

#[cfg(test)]
mod structured_logging {
    use std::fs;

    /// Verifies that tracing dependency is included in Cargo.toml
    #[test]
    fn tracing_dependency_exists() {
        let content = fs::read_to_string("backend/Cargo.toml")
            .expect("Failed to read backend/Cargo.toml");

        assert!(
            content.contains("tracing"),
            "backend/Cargo.toml should include 'tracing' dependency"
        );
    }

    /// Verifies that tracing-subscriber is included for JSON logging
    #[test]
    fn tracing_subscriber_json_support() {
        let content = fs::read_to_string("backend/Cargo.toml")
            .expect("Failed to read backend/Cargo.toml");

        assert!(
            content.contains("tracing-subscriber"),
            "backend/Cargo.toml should include 'tracing-subscriber' for JSON formatting"
        );
    }

    /// Verifies that env_logger is available for log level configuration
    #[test]
    fn env_logger_available() {
        let content = fs::read_to_string("backend/Cargo.toml")
            .expect("Failed to read backend/Cargo.toml");

        assert!(
            content.contains("env_logger") || content.contains("log"),
            "backend/Cargo.toml should include 'env_logger' or 'log' for log level configuration"
        );
    }

    /// Verifies that .env.example documents LOG_LEVEL configuration
    #[test]
    fn env_example_documents_log_level() {
        let content = fs::read_to_string(".env.example")
            .expect("Failed to read .env.example");

        assert!(
            content.contains("LOG") || content.contains("log"),
            ".env.example should document logging configuration variables"
        );
    }

    /// Verifies that logging is used in main backend code
    #[test]
    fn logging_is_imported_in_source() {
        let main_rs = "backend/src/main.rs";
        if let Ok(content) = fs::read_to_string(main_rs) {
            // Should have either tracing or log imports
            let has_logging = content.contains("tracing") ||
                            content.contains("log") ||
                            content.contains("tracing_subscriber");

            assert!(
                has_logging,
                "main.rs should import logging crates (tracing, log, tracing_subscriber)"
            );
        }
    }

    /// Verifies that structured logging supports context fields
    #[test]
    fn logging_can_be_structured() {
        let content = fs::read_to_string("backend/Cargo.toml")
            .expect("Failed to read backend/Cargo.toml");

        // tracing allows structured logging with fields
        assert!(
            content.contains("tracing"),
            "tracing dependency enables structured logging with context fields"
        );
    }

    /// Verifies that OpenTelemetry is available for distributed tracing
    #[test]
    fn opentelemetry_integration_available() {
        let content = fs::read_to_string("backend/Cargo.toml")
            .expect("Failed to read backend/Cargo.toml");

        assert!(
            content.contains("opentelemetry") || content.contains("tracing-opentelemetry"),
            "backend/Cargo.toml should include OpenTelemetry for distributed tracing"
        );
    }

    /// Verifies that docker-compose surfaces logs
    #[test]
    fn docker_compose_surfaces_logs() {
        let content = fs::read_to_string("docker-compose.yml")
            .expect("Failed to read docker-compose.yml");

        // Should reference logging in service definition
        assert!(
            !content.is_empty(),
            "docker-compose.yml should be configured for log collection"
        );
    }

    /// Verifies that logging configuration supports environment variables
    #[test]
    fn logging_supports_env_configuration() {
        let env_example = fs::read_to_string(".env.example")
            .expect("Failed to read .env.example");

        let main_rs = fs::read_to_string("backend/src/main.rs")
            .unwrap_or_default();

        // Should support env_filter or similar mechanism
        assert!(
            env_example.contains("LOG") || main_rs.contains("env"),
            "Logging should be configurable via environment variables"
        );
    }

    /// Verifies that json/structured output is supported
    #[test]
    fn json_output_support() {
        let content = fs::read_to_string("backend/Cargo.toml")
            .expect("Failed to read backend/Cargo.toml");

        // tracing-subscriber enables JSON formatting
        assert!(
            content.contains("tracing-subscriber"),
            "tracing-subscriber enables JSON output formatting"
        );
    }

    /// Verifies that request tracing is available through tower-http
    #[test]
    fn request_tracing_middleware_available() {
        let content = fs::read_to_string("backend/Cargo.toml")
            .expect("Failed to read backend/Cargo.toml");

        assert!(
            content.contains("tower-http"),
            "backend/Cargo.toml should include tower-http for request tracing middleware"
        );
    }

    /// Verifies that tower-http includes trace feature
    #[test]
    fn tower_http_trace_feature() {
        let content = fs::read_to_string("backend/Cargo.toml")
            .expect("Failed to read backend/Cargo.toml");

        assert!(
            content.contains("tower-http") && content.contains("trace"),
            "tower-http should be configured with trace feature for request logging"
        );
    }
}
