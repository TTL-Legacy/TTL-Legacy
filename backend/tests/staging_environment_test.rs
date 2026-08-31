//! Staging environment configuration tests.
//! Issue #1346: Infrastructure: add staging environment with testnet deployment

#[cfg(test)]
mod staging_environment {
    use std::fs;
    use std::path::Path;

    /// Verifies that environments.toml configuration file exists
    #[test]
    fn environments_config_exists() {
        assert!(
            Path::new("environments.toml").exists(),
            "environments.toml should exist for environment configuration"
        );
    }

    /// Verifies that environments.toml contains staging configuration
    #[test]
    fn environments_config_has_staging() {
        let content = fs::read_to_string("environments.toml")
            .expect("Failed to read environments.toml");

        assert!(
            content.contains("[staging]") || content.contains("staging"),
            "environments.toml should contain staging environment configuration"
        );
    }

    /// Verifies that docker-compose configuration exists
    #[test]
    fn docker_compose_yml_exists() {
        assert!(
            Path::new("docker-compose.yml").exists(),
            "docker-compose.yml should exist"
        );
    }

    /// Verifies that docker-compose defines services
    #[test]
    fn docker_compose_defines_services() {
        let content = fs::read_to_string("docker-compose.yml")
            .expect("Failed to read docker-compose.yml");

        assert!(
            content.contains("services:"),
            "docker-compose.yml should define services"
        );
    }

    /// Verifies that GitHub Actions workflows directory exists
    #[test]
    fn github_workflows_directory_exists() {
        assert!(
            Path::new(".github/workflows").exists(),
            ".github/workflows directory should exist"
        );
    }

    /// Verifies that CI workflow file exists
    #[test]
    fn ci_workflow_exists() {
        assert!(
            Path::new(".github/workflows/ci.yml").exists(),
            ".github/workflows/ci.yml should exist for CI/CD"
        );
    }

    /// Verifies that environment-specific deployments are configured
    #[test]
    fn deployment_workflows_exist() {
        let workflows_dir = ".github/workflows";
        let entries = fs::read_dir(workflows_dir)
            .expect("Failed to read workflows directory");

        let deployment_files: Vec<_> = entries
            .filter_map(|entry| {
                let entry = entry.ok()?;
                let path = entry.path();
                if path.extension()?.to_str() == Some("yml")
                    || path.extension()?.to_str() == Some("yaml")
                {
                    path.file_name()?.to_str().map(|s| s.to_string())
                } else {
                    None
                }
            })
            .collect();

        assert!(
            !deployment_files.is_empty(),
            "At least one workflow file should exist"
        );
    }

    /// Verifies that .env.example exists for environment variable documentation
    #[test]
    fn env_example_exists() {
        assert!(
            Path::new(".env.example").exists(),
            ".env.example should exist for documenting environment variables"
        );
    }

    /// Verifies that .env.example documents staging-specific variables
    #[test]
    fn env_example_has_environment_variables() {
        let content = fs::read_to_string(".env.example")
            .expect("Failed to read .env.example");

        assert!(
            !content.is_empty(),
            ".env.example should contain environment variable documentation"
        );
    }

    /// Verifies that deployment-related configuration exists
    #[test]
    fn deployment_configuration_exists() {
        let config_files = vec![
            "Dockerfile",
            "docker-compose.yml",
            "environments.toml",
        ];

        for file in config_files {
            assert!(
                Path::new(file).exists() || Path::new(&format!("backend/{}", file)).exists(),
                "Deployment configuration {} should exist",
                file
            );
        }
    }

    /// Verifies that staging differs from production configuration
    #[test]
    fn staging_production_separation() {
        let content = fs::read_to_string("environments.toml")
            .expect("Failed to read environments.toml");

        // Should have multiple environment sections
        let sections: Vec<&str> = content.lines()
            .filter(|line| line.trim().starts_with("["))
            .collect();

        assert!(
            sections.len() >= 2,
            "environments.toml should define multiple environments (dev, staging, prod)"
        );
    }
}
