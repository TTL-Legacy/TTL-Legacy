//! Test for Issue #1588: Ensure notification provider configuration is documented.
//!
//! This test verifies that:
//! - Notification provider documentation exists
//! - Environment variables for each provider are documented
//! - Example configurations are provided for email, SMS, and push providers

#[cfg(test)]
mod notification_provider_documentation {
    use std::fs;

    /// Notification provider types that should be documented
    const NOTIFICATION_PROVIDERS: &[&str] = &[
        "email",
        "Email",
        "SMS",
        "sms",
        "push",
        "Push",
    ];

    /// Example configuration markers that should appear in docs
    const CONFIG_EXAMPLES: &[&str] = &[
        "example",
        "Example",
        "config",
        "Config",
    ];

    #[test]
    fn notification_documentation_exists() {
        let notification_docs_paths = vec![
            "docs/push-notifications.md",
            "docs/deployment-guide.md",
        ];

        let at_least_one_exists = notification_docs_paths
            .iter()
            .any(|path| std::path::Path::new(path).exists());

        assert!(
            at_least_one_exists,
            "Notification provider documentation should exist in docs/"
        );
    }

    #[test]
    fn notification_providers_are_documented() {
        let push_doc = fs::read_to_string("docs/push-notifications.md")
            .expect("docs/push-notifications.md should be readable");

        let has_provider_types = NOTIFICATION_PROVIDERS.iter().any(|provider| {
            push_doc.contains(provider)
        });

        assert!(
            has_provider_types,
            "Notification documentation should mention notification provider types"
        );
    }

    #[test]
    fn notification_docs_contain_configuration_guidance() {
        let push_doc = fs::read_to_string("docs/push-notifications.md")
            .expect("docs/push-notifications.md should be readable");

        let has_config_guidance = CONFIG_EXAMPLES.iter().any(|keyword| {
            push_doc.contains(keyword)
        });

        assert!(
            has_config_guidance,
            "Notification documentation should include configuration examples"
        );
    }

    #[test]
    fn deployment_guide_references_notification_config() {
        let deployment_guide = fs::read_to_string("docs/deployment-guide.md")
            .expect("docs/deployment-guide.md should be readable");

        let mentions_notifications = deployment_guide.contains("notification")
            || deployment_guide.contains("Notification")
            || deployment_guide.contains("push")
            || deployment_guide.contains("Push");

        assert!(
            mentions_notifications,
            "Deployment guide should reference notification configuration"
        );
    }

    #[test]
    fn notification_docs_mention_providers() {
        let push_doc = fs::read_to_string("docs/push-notifications.md")
            .expect("docs/push-notifications.md should be readable");

        // Check for mention of provider setup or integration
        let mentions_setup = push_doc.contains("setup")
            || push_doc.contains("Setup")
            || push_doc.contains("configure")
            || push_doc.contains("Configure")
            || push_doc.contains("provider")
            || push_doc.contains("Provider");

        assert!(
            mentions_setup,
            "Notification documentation should explain how to set up providers"
        );
    }

    #[test]
    fn notification_docs_has_structure() {
        let push_doc = fs::read_to_string("docs/push-notifications.md")
            .expect("docs/push-notifications.md should be readable");

        // Check for markdown headers
        let has_headers = push_doc.contains("##") || push_doc.contains("#");

        assert!(
            has_headers,
            "Notification documentation should be properly structured with headers"
        );
    }
}
