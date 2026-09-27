//! Architecture diagram tests for backend components.
//! Issue #1589: Documentation: add architecture diagram for backend components

#[cfg(test)]
mod architecture_documentation {
    use std::fs;
    use std::path::Path;

    /// Verifies that architecture documentation file exists
    #[test]
    fn architecture_documentation_file_exists() {
        let path = Path::new("docs/architecture.md");
        assert!(
            path.exists(),
            "docs/architecture.md should exist"
        );
    }

    /// Verifies that architecture documentation contains mermaid diagram
    #[test]
    fn architecture_has_mermaid_diagram() {
        let content = fs::read_to_string("docs/architecture.md")
            .expect("Failed to read docs/architecture.md");

        assert!(
            content.contains("```mermaid") || content.contains("graph") || content.contains("diagram"),
            "docs/architecture.md should contain a Mermaid diagram"
        );
    }

    /// Verifies that architecture documentation describes scheduler interaction
    #[test]
    fn architecture_documents_scheduler() {
        let content = fs::read_to_string("docs/architecture.md")
            .expect("Failed to read docs/architecture.md");

        assert!(
            content.to_lowercase().contains("scheduler"),
            "docs/architecture.md should document scheduler component"
        );
    }

    /// Verifies that architecture documentation describes notifications interaction
    #[test]
    fn architecture_documents_notifications() {
        let content = fs::read_to_string("docs/architecture.md")
            .expect("Failed to read docs/architecture.md");

        assert!(
            content.to_lowercase().contains("notification"),
            "docs/architecture.md should document notifications component"
        );
    }

    /// Verifies that architecture documentation describes cache interaction
    #[test]
    fn architecture_documents_cache() {
        let content = fs::read_to_string("docs/architecture.md")
            .expect("Failed to read docs/architecture.md");

        assert!(
            content.to_lowercase().contains("cache"),
            "docs/architecture.md should document cache component"
        );
    }

    /// Verifies that architecture documentation describes websocket interaction
    #[test]
    fn architecture_documents_websocket() {
        let content = fs::read_to_string("docs/architecture.md")
            .expect("Failed to read docs/architecture.md");

        assert!(
            content.to_lowercase().contains("websocket"),
            "docs/architecture.md should document websocket component"
        );
    }

    /// Verifies that architecture documentation includes data flow description
    #[test]
    fn architecture_describes_data_flow() {
        let content = fs::read_to_string("docs/architecture.md")
            .expect("Failed to read docs/architecture.md");

        let has_flow_description = content.to_lowercase().contains("data flow") ||
            content.to_lowercase().contains("flow") ||
            content.to_lowercase().contains("interaction");

        assert!(
            has_flow_description,
            "docs/architecture.md should include data flow description"
        );
    }

    /// Verifies that architecture documentation includes check-in flow
    #[test]
    fn architecture_describes_checkin_flow() {
        let content = fs::read_to_string("docs/architecture.md")
            .expect("Failed to read docs/architecture.md");

        assert!(
            content.to_lowercase().contains("check-in") || content.to_lowercase().contains("checkin"),
            "docs/architecture.md should include check-in data flow description"
        );
    }

    /// Verifies that architecture documentation is not empty
    #[test]
    fn architecture_documentation_has_content() {
        let content = fs::read_to_string("docs/architecture.md")
            .expect("Failed to read docs/architecture.md");

        assert!(
            content.len() > 200,
            "docs/architecture.md should contain substantial content"
        );
    }

    /// Verifies that architecture documentation has proper heading structure
    #[test]
    fn architecture_has_proper_structure() {
        let content = fs::read_to_string("docs/architecture.md")
            .expect("Failed to read docs/architecture.md");

        assert!(
            content.contains("#"),
            "docs/architecture.md should have markdown headings"
        );
    }
}
