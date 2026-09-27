//! Cargo-deny license and advisory verification tests
//! Issue #1572: Enhancement: add cargo-deny for license and advisory checks

#[cfg(test)]
mod cargo_deny {
    use std::fs;
    use std::path::Path;

    /// Verifies that deny.toml exists in the project root
    #[test]
    fn deny_toml_exists() {
        let deny_path = "deny.toml";
        assert!(
            Path::new(deny_path).exists(),
            "deny.toml should exist in project root"
        );
    }

    /// Verifies that deny.toml has advisories section enabled
    #[test]
    fn deny_toml_has_advisories_section() {
        let content = fs::read_to_string("deny.toml")
            .expect("Failed to read deny.toml");

        assert!(
            content.contains("[advisories]"),
            "deny.toml should have [advisories] section"
        );
    }

    /// Verifies that deny.toml has licenses section configured
    #[test]
    fn deny_toml_has_licenses_section() {
        let content = fs::read_to_string("deny.toml")
            .expect("Failed to read deny.toml");

        assert!(
            content.contains("[licenses]"),
            "deny.toml should have [licenses] section"
        );
    }

    /// Verifies that deny.toml has bans section configured
    #[test]
    fn deny_toml_has_bans_section() {
        let content = fs::read_to_string("deny.toml")
            .expect("Failed to read deny.toml");

        assert!(
            content.contains("[bans]"),
            "deny.toml should have [bans] section"
        );
    }

    /// Verifies that CI workflow includes cargo-deny check
    #[test]
    fn ci_workflow_has_cargo_deny_check() {
        let ci_path = ".github/workflows/ci.yml";
        let content = fs::read_to_string(ci_path)
            .expect("Failed to read CI workflow file");

        assert!(
            content.contains("cargo-deny") || content.contains("cargo deny"),
            "CI workflow should include cargo-deny check"
        );
    }

    /// Verifies that advisories denial list is configured
    #[test]
    fn deny_toml_has_advisories_configured() {
        let content = fs::read_to_string("deny.toml")
            .expect("Failed to read deny.toml");

        let advisories_section = content.split("[advisories]")
            .nth(1)
            .expect("Could not find [advisories] section");

        assert!(
            advisories_section.contains("db-path") || advisories_section.contains("vulnerability"),
            "advisories section should be properly configured"
        );
    }

    /// Verifies that allowed licenses list is configured
    #[test]
    fn deny_toml_has_allowed_licenses() {
        let content = fs::read_to_string("deny.toml")
            .expect("Failed to read deny.toml");

        let licenses_section = content.split("[licenses]")
            .nth(1)
            .expect("Could not find [licenses] section");

        assert!(
            licenses_section.contains("allow") || licenses_section.contains("deny"),
            "licenses section should specify allowed or denied licenses"
        );
    }
}
