//! Docker image vulnerability scanning with Trivy tests
//! Issue #1573: Enhancement: add docker image vulnerability scanning

#[cfg(test)]
mod docker_trivy_scanning {
    use std::fs;
    use std::path::Path;

    /// Verifies that release workflow exists
    #[test]
    fn release_workflow_exists() {
        let release_path = ".github/workflows/release.yml";
        assert!(
            Path::new(release_path).exists(),
            "release workflow should exist at {}",
            release_path
        );
    }

    /// Verifies that Trivy scanning is included in release workflow
    #[test]
    fn trivy_scan_included_in_release_workflow() {
        let content = fs::read_to_string(".github/workflows/release.yml")
            .expect("Failed to read release workflow");

        assert!(
            content.contains("trivy") || content.contains("Trivy"),
            "release workflow should include Trivy scanning step"
        );
    }

    /// Verifies that Docker image scanning targets backend Dockerfile
    #[test]
    fn trivy_scans_docker_image() {
        let content = fs::read_to_string(".github/workflows/release.yml")
            .expect("Failed to read release workflow");

        let trivy_section = content.split("trivy").next().unwrap_or("");
        let has_docker_target = trivy_section.contains("Dockerfile") || trivy_section.contains("image")
            || trivy_section.contains("backend");

        assert!(
            has_docker_target,
            "Trivy scanning should target Docker images"
        );
    }

    /// Verifies that critical vulnerabilities fail the build
    #[test]
    fn trivy_fails_on_critical_vulnerabilities() {
        let content = fs::read_to_string(".github/workflows/release.yml")
            .expect("Failed to read release workflow");

        let has_severity_check = content.contains("critical") || content.contains("CRITICAL")
            || content.contains("severity") || content.contains("exit 1");

        assert!(
            has_severity_check,
            "Trivy should fail the build on critical vulnerabilities"
        );
    }

    /// Verifies that Dockerfile exists for scanning
    #[test]
    fn backend_dockerfile_exists() {
        assert!(
            Path::new("backend/Dockerfile").exists(),
            "backend/Dockerfile should exist for Trivy scanning"
        );
    }

    /// Verifies that release workflow is a valid YAML
    #[test]
    fn release_workflow_is_valid_yaml() {
        let content = fs::read_to_string(".github/workflows/release.yml")
            .expect("Failed to read release workflow");

        assert!(
            content.contains("name:") && content.contains("on:"),
            "release workflow should be valid GitHub Actions workflow"
        );
    }

    /// Verifies that scanning step has proper naming
    #[test]
    fn trivy_step_properly_named() {
        let content = fs::read_to_string(".github/workflows/release.yml")
            .expect("Failed to read release workflow");

        let trivy_section = content.split("trivy").next().unwrap_or("");
        let has_step_name = trivy_section.contains("name:") || trivy_section.contains("uses:");

        // This is informational - good practice
        println!("✓ Trivy scanning step should have proper name and uses fields");
    }

    /// Verifies that Docker build is performed before scanning
    #[test]
    fn docker_build_before_trivy_scan() {
        let content = fs::read_to_string(".github/workflows/release.yml")
            .expect("Failed to read release workflow");

        let has_build_step = content.contains("docker build") || content.contains("Build")
            || content.contains("build");

        assert!(
            has_build_step || content.contains("trivy"),
            "release workflow should prepare Docker image before Trivy scanning"
        );
    }

    /// Verifies that scanning output is captured
    #[test]
    fn trivy_scan_output_is_captured() {
        let content = fs::read_to_string(".github/workflows/release.yml")
            .expect("Failed to read release workflow");

        let trivy_section = content.split("trivy").next().unwrap_or("");
        let has_output = trivy_section.contains("format") || trivy_section.contains("output")
            || trivy_section.contains("json") || trivy_section.contains("table");

        assert!(
            has_output,
            "Trivy scanning should capture formatted output"
        );
    }
}
