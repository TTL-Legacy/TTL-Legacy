//! Test for Issue #1589: Ensure architecture.md has comprehensive component diagrams.
//!
//! This test verifies that:
//! - Architecture documentation exists
//! - Component diagram is present
//! - Data flow diagrams are documented (check-in and release flows)
//! - Architecture describes interactions between scheduler, notifications, cache, and websocket

#[cfg(test)]
mod architecture_documentation {
    use std::fs;

    #[test]
    fn architecture_document_exists() {
        let arch_path = "docs/architecture.md";
        assert!(
            std::path::Path::new(arch_path).exists(),
            "docs/architecture.md should exist"
        );
    }

    #[test]
    fn architecture_has_component_diagram() {
        let architecture = fs::read_to_string("docs/architecture.md")
            .expect("docs/architecture.md should be readable");

        let has_component_diagram = architecture.contains("Component Diagram")
            || (architecture.contains("graph TD") || architecture.contains("graph LR"));

        assert!(
            has_component_diagram,
            "Architecture should contain a component diagram showing system interactions"
        );
    }

    #[test]
    fn architecture_has_data_flow_diagrams() {
        let architecture = fs::read_to_string("docs/architecture.md")
            .expect("docs/architecture.md should be readable");

        let has_check_in_flow = architecture.contains("Check-in")
            || architecture.contains("check-in")
            || architecture.contains("check in");

        let has_release_flow = architecture.contains("Release")
            || architecture.contains("release");

        assert!(
            has_check_in_flow,
            "Architecture should document the check-in flow"
        );
        assert!(
            has_release_flow,
            "Architecture should document the release flow"
        );
    }

    #[test]
    fn architecture_describes_key_components() {
        let architecture = fs::read_to_string("docs/architecture.md")
            .expect("docs/architecture.md should be readable");

        // These are key components mentioned in issue #1589
        let has_scheduler = architecture.contains("scheduler") || architecture.contains("Scheduler");
        let has_notifications = architecture.contains("notification") || architecture.contains("Notification");
        let has_cache = architecture.contains("cache") || architecture.contains("Cache");
        let has_websocket = architecture.contains("websocket") || architecture.contains("WebSocket");

        // At least 2 of these 4 should be documented
        let documented_count = [has_scheduler, has_notifications, has_cache, has_websocket]
            .iter()
            .filter(|&&x| x)
            .count();

        assert!(
            documented_count >= 2,
            "Architecture should describe interactions between key components (scheduler, notifications, cache, websocket)"
        );
    }

    #[test]
    fn architecture_has_technology_stack_section() {
        let architecture = fs::read_to_string("docs/architecture.md")
            .expect("docs/architecture.md should be readable");

        assert!(
            architecture.contains("Technology Stack") || architecture.contains("technology stack"),
            "Architecture should document the technology stack used"
        );
    }

    #[test]
    fn architecture_has_mermaid_diagrams() {
        let architecture = fs::read_to_string("docs/architecture.md")
            .expect("docs/architecture.md should be readable");

        let mermaid_count = architecture.matches("```mermaid").count();

        assert!(
            mermaid_count >= 2,
            "Architecture should contain at least 2 Mermaid diagrams (component and data flow)"
        );
    }
}
