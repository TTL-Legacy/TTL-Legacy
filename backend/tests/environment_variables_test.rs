//! Environment variables documentation tests for backend deployment.
//! Issue #1590: Documentation: document every environment variable used by backend

#[cfg(test)]
mod environment_variables_documentation {
    use std::fs;
    use std::path::Path;

    /// Verifies that deployment guide documentation exists
    #[test]
    fn deployment_guide_file_exists() {
        let path = Path::new("docs/deployment-guide.md");
        assert!(
            path.exists(),
            "docs/deployment-guide.md should exist"
        );
    }

    /// Verifies that .env.example file exists
    #[test]
    fn env_example_file_exists() {
        let path = Path::new(".env.example");
        let alt_path = Path::new("backend/.env.example");

        let exists = path.exists() || alt_path.exists();
        assert!(
            exists,
            ".env.example should exist in project root or backend directory"
        );
    }

    /// Verifies that deployment guide includes environment variables table
    #[test]
    fn deployment_guide_documents_environment_variables() {
        let content = fs::read_to_string("docs/deployment-guide.md")
            .expect("Failed to read docs/deployment-guide.md");

        let has_env_section = content.to_lowercase().contains("environment") ||
            content.to_lowercase().contains("variable") ||
            content.to_lowercase().contains("env");

        assert!(
            has_env_section,
            "docs/deployment-guide.md should document environment variables"
        );
    }

    /// Verifies that .env.example is not empty
    #[test]
    fn env_example_has_content() {
        let content = get_env_example_content();
        assert!(
            !content.trim().is_empty(),
            ".env.example should not be empty"
        );
    }

    /// Verifies that .env.example includes common environment variables
    #[test]
    fn env_example_includes_database_config() {
        let content = get_env_example_content();

        let has_database = content.contains("DATABASE") ||
            content.contains("DB_") ||
            content.contains("POSTGRES");

        assert!(
            has_database,
            ".env.example should include database configuration variables"
        );
    }

    /// Verifies that .env.example includes MIN_CONTRACT_VERSION
    #[test]
    fn env_example_includes_min_contract_version() {
        let content = get_env_example_content();

        assert!(
            content.contains("MIN_CONTRACT_VERSION"),
            ".env.example should include MIN_CONTRACT_VERSION"
        );
    }

    /// Verifies that .env.example includes OTEL configuration
    #[test]
    fn env_example_includes_otel_configuration() {
        let content = get_env_example_content();

        assert!(
            content.contains("OTEL_EXPORTER_OTLP_ENDPOINT"),
            ".env.example should include OTEL_EXPORTER_OTLP_ENDPOINT"
        );
    }

    /// Verifies that deployment guide describes variable purposes
    #[test]
    fn deployment_guide_describes_variable_purposes() {
        let content = fs::read_to_string("docs/deployment-guide.md")
            .expect("Failed to read docs/deployment-guide.md");

        let has_descriptions = content.contains("|") || // table format
            content.to_lowercase().contains("description") ||
            content.to_lowercase().contains("purpose");

        assert!(
            has_descriptions,
            "docs/deployment-guide.md should describe purposes of environment variables"
        );
    }

    /// Verifies that deployment guide includes values for required variables
    #[test]
    fn deployment_guide_specifies_required_variables() {
        let content = fs::read_to_string("docs/deployment-guide.md")
            .expect("Failed to read docs/deployment-guide.md");

        let has_required = content.to_lowercase().contains("required") ||
            content.to_lowercase().contains("mandatory") ||
            content.to_lowercase().contains("must");

        assert!(
            has_required,
            "docs/deployment-guide.md should specify which variables are required"
        );
    }

    /// Verifies that .env.example includes example values
    #[test]
    fn env_example_includes_sample_values() {
        let content = get_env_example_content();

        let has_values = content.contains("=") &&
            content.lines().filter(|l| l.contains("=")).count() > 3;

        assert!(
            has_values,
            ".env.example should include sample values for variables"
        );
    }

    /// Verifies that deployment guide is comprehensive
    #[test]
    fn deployment_guide_is_comprehensive() {
        let content = fs::read_to_string("docs/deployment-guide.md")
            .expect("Failed to read docs/deployment-guide.md");

        assert!(
            content.len() > 500,
            "docs/deployment-guide.md should be comprehensive"
        );
    }

    /// Verifies that environment variables are properly formatted
    #[test]
    fn env_variables_follow_conventions() {
        let content = get_env_example_content();

        let lines: Vec<&str> = content.lines()
            .filter(|l| !l.trim().is_empty() && !l.trim().starts_with("#"))
            .collect();

        assert!(
            !lines.is_empty(),
            ".env.example should have non-commented lines"
        );

        for line in lines {
            if !line.starts_with("#") && !line.trim().is_empty() {
                assert!(
                    line.contains("="),
                    "Each variable line should follow KEY=VALUE format: {}",
                    line
                );
            }
        }
    }

    /// Verifies that deployment guide includes configuration table
    #[test]
    fn deployment_guide_includes_configuration_table() {
        let content = fs::read_to_string("docs/deployment-guide.md")
            .expect("Failed to read docs/deployment-guide.md");

        let has_table = content.contains("|") && content.contains("---");

        assert!(
            has_table,
            "docs/deployment-guide.md should include a configuration table"
        );
    }

    // Helper function to read .env.example content
    fn get_env_example_content() -> String {
        let path = Path::new(".env.example");
        let alt_path = Path::new("backend/.env.example");

        if path.exists() {
            fs::read_to_string(path)
                .expect("Failed to read .env.example")
        } else if alt_path.exists() {
            fs::read_to_string(alt_path)
                .expect("Failed to read backend/.env.example")
        } else {
            panic!(".env.example not found");
        }
    }
}
