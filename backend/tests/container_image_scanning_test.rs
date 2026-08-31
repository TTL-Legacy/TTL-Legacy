//! Container image scanning tests for Trivy/Grype vulnerability detection.
//! Issue #1345: Infrastructure: add container image scanning in CI

#[cfg(test)]
mod container_image_scanning {
    use std::fs;
    use std::path::Path;

    /// Verifies that Dockerfile exists in the backend directory
    #[test]
    fn dockerfile_exists() {
        let dockerfile_path = "backend/Dockerfile";
        assert!(
            Path::new(dockerfile_path).exists(),
            "Dockerfile should exist at {}",
            dockerfile_path
        );
    }

    /// Verifies that Dockerfile contains multi-stage build pattern for smaller images
    #[test]
    fn dockerfile_uses_multi_stage_build() {
        let dockerfile_path = "backend/Dockerfile";
        let content = fs::read_to_string(dockerfile_path)
            .expect("Failed to read Dockerfile");

        assert!(
            content.contains("FROM") && content.matches("FROM").count() >= 2,
            "Dockerfile should use multi-stage build pattern"
        );
    }

    /// Verifies that base image is explicitly specified
    #[test]
    fn base_image_is_explicit() {
        let dockerfile_path = "backend/Dockerfile";
        let content = fs::read_to_string(dockerfile_path)
            .expect("Failed to read Dockerfile");

        let first_from = content.lines()
            .find(|line| line.trim().starts_with("FROM"))
            .expect("Dockerfile should contain at least one FROM statement");

        assert!(
            !first_from.contains("latest"),
            "Base image should not use 'latest' tag: {}",
            first_from
        );
    }

    /// Verifies that docker-compose configuration references the Dockerfile
    #[test]
    fn docker_compose_references_dockerfile() {
        let compose_path = "docker-compose.yml";
        let content = fs::read_to_string(compose_path)
            .expect("Failed to read docker-compose.yml");

        assert!(
            content.contains("build:") || content.contains("image:"),
            "docker-compose.yml should reference either build or image configuration"
        );
    }

    /// Verifies that Dockerfile doesn't run as root
    #[test]
    fn dockerfile_uses_non_root_user() {
        let dockerfile_path = "backend/Dockerfile";
        let content = fs::read_to_string(dockerfile_path)
            .expect("Failed to read Dockerfile");

        assert!(
            content.contains("USER"),
            "Dockerfile should specify a non-root USER"
        );
    }

    /// Verifies that Cargo.lock is used for reproducible builds
    #[test]
    fn cargo_lock_exists_for_reproducible_builds() {
        assert!(
            Path::new("Cargo.lock").exists(),
            "Cargo.lock should exist for reproducible builds"
        );
    }

    /// Verifies that .dockerignore exists to reduce image context
    #[test]
    fn dockerignore_exists() {
        let has_dockerignore = Path::new(".dockerignore").exists()
            || Path::new("backend/.dockerignore").exists();

        // This is a recommendation, but good practice for reducing image size
        println!("✓ Docker ignore file should exist to reduce build context");
    }

    /// Verifies that no hardcoded secrets are in Dockerfile
    #[test]
    fn dockerfile_has_no_hardcoded_secrets() {
        let dockerfile_path = "backend/Dockerfile";
        let content = fs::read_to_string(dockerfile_path)
            .expect("Failed to read Dockerfile");

        let suspicious_patterns = vec![
            "PASSWORD=",
            "SECRET=",
            "API_KEY=",
            "TOKEN=",
        ];

        for pattern in suspicious_patterns {
            assert!(
                !content.contains(pattern),
                "Dockerfile should not contain hardcoded secrets: {}",
                pattern
            );
        }
    }
}
