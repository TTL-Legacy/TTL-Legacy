//! Notification provider configuration tests for backend documentation.
//! Issue #1588: Documentation: document notification provider configuration

#[cfg(test)]
mod notification_provider_documentation {
    use std::fs;
    use std::path::Path;

    /// Verifies that notification provider documentation exists
    #[test]
    fn notification_provider_doc_file_exists() {
        let path = Path::new("docs/notification-providers.md");
        let alt_path = Path::new("docs/notifications.md");

        let exists = path.exists() || alt_path.exists();
        assert!(
            exists,
            "Notification provider documentation should exist at docs/notification-providers.md or docs/notifications.md"
        );
    }

    /// Verifies that documentation describes email provider configuration
    #[test]
    fn documents_email_provider_configuration() {
        let content = get_notification_docs();

        assert!(
            content.to_lowercase().contains("email"),
            "Should document email provider configuration"
        );
    }

    /// Verifies that documentation describes SMS provider configuration
    #[test]
    fn documents_sms_provider_configuration() {
        let content = get_notification_docs();

        assert!(
            content.to_lowercase().contains("sms"),
            "Should document SMS provider configuration"
        );
    }

    /// Verifies that documentation describes push notification configuration
    #[test]
    fn documents_push_notification_configuration() {
        let content = get_notification_docs();

        assert!(
            content.to_lowercase().contains("push"),
            "Should document push notification provider configuration"
        );
    }

    /// Verifies that documentation includes environment variable examples
    #[test]
    fn includes_environment_variable_examples() {
        let content = get_notification_docs();

        assert!(
            content.to_lowercase().contains("env") ||
            content.contains("$") ||
            content.contains("PROVIDER"),
            "Should include environment variable examples"
        );
    }

    /// Verifies that documentation includes example configurations
    #[test]
    fn includes_example_configurations() {
        let content = get_notification_docs();

        let has_examples = content.contains("```") ||
            content.to_lowercase().contains("example") ||
            content.to_lowercase().contains("sample");

        assert!(
            has_examples,
            "Should include example configurations"
        );
    }

    /// Verifies that documentation describes how to enable providers
    #[test]
    fn describes_enabling_providers() {
        let content = get_notification_docs();

        let has_enable_guide = content.to_lowercase().contains("enable") ||
            content.to_lowercase().contains("configure") ||
            content.to_lowercase().contains("setup");

        assert!(
            has_enable_guide,
            "Should describe how to enable/configure providers"
        );
    }

    /// Verifies that documentation is comprehensive
    #[test]
    fn documentation_is_comprehensive() {
        let content = get_notification_docs();

        assert!(
            content.len() > 300,
            "Notification provider documentation should be comprehensive"
        );
    }

    /// Verifies that documentation has proper markdown structure
    #[test]
    fn has_proper_markdown_structure() {
        let content = get_notification_docs();

        assert!(
            content.contains("#"),
            "Should have markdown headings"
        );
    }

    /// Verifies that documentation lists required fields per provider
    #[test]
    fn lists_required_configuration_fields() {
        let content = get_notification_docs();

        let has_field_list = content.to_lowercase().contains("required") ||
            content.to_lowercase().contains("field") ||
            content.to_lowercase().contains("parameter") ||
            content.contains("-") ||
            content.contains("*");

        assert!(
            has_field_list,
            "Should list required configuration fields for each provider"
        );
    }

    /// Verifies that documentation describes provider selection
    #[test]
    fn describes_provider_selection() {
        let content = get_notification_docs();

        let has_selection_guide = content.to_lowercase().contains("select") ||
            content.to_lowercase().contains("choose") ||
            content.to_lowercase().contains("default");

        assert!(
            has_selection_guide,
            "Should describe how to select notification providers"
        );
    }

    // Helper function to read notification documentation
    fn get_notification_docs() -> String {
        let path = Path::new("docs/notification-providers.md");
        let alt_path = Path::new("docs/notifications.md");

        if path.exists() {
            fs::read_to_string(path)
                .expect("Failed to read docs/notification-providers.md")
        } else if alt_path.exists() {
            fs::read_to_string(alt_path)
                .expect("Failed to read docs/notifications.md")
        } else {
            panic!("Notification provider documentation not found");
        }
    }
}
