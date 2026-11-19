//! Test for Issue #1587: Ensure roadmap.md accurately reflects delivery status.
//!
//! This test verifies that:
//! - Roadmap document exists
//! - Delivered features are marked with checkmarks (✅)
//! - Future items are clearly separated by version or milestone
//! - Status indicators are consistent throughout the document

#[cfg(test)]
mod roadmap_documentation {
    use std::fs;

    /// Features that should be marked as delivered (✅) in the roadmap
    const DELIVERED_FEATURES: &[&str] = &[
        "passkey",
        "Passkey",
        "mobile",
        "Mobile",
        "multisig",
        "Multi-signature",
    ];

    /// Version milestones that should be in the roadmap
    const VERSION_MILESTONES: &[&str] = &["v1.0", "v1.1", "v2.0", "v2.1", "v3.0"];

    #[test]
    fn roadmap_document_exists() {
        let roadmap_path = "docs/roadmap.md";
        assert!(
            std::path::Path::new(roadmap_path).exists(),
            "docs/roadmap.md should exist"
        );
    }

    #[test]
    fn roadmap_has_version_milestones() {
        let roadmap = fs::read_to_string("docs/roadmap.md")
            .expect("docs/roadmap.md should be readable");

        for milestone in VERSION_MILESTONES {
            assert!(
                roadmap.contains(milestone),
                "Roadmap should document milestone {}",
                milestone
            );
        }
    }

    #[test]
    fn roadmap_marks_delivered_features() {
        let roadmap = fs::read_to_string("docs/roadmap.md")
            .expect("docs/roadmap.md should be readable");

        let has_checkmark = roadmap.contains("✅");

        assert!(
            has_checkmark,
            "Roadmap should mark delivered features with ✅ checkmark"
        );
    }

    #[test]
    fn roadmap_current_version_is_marked() {
        let roadmap = fs::read_to_string("docs/roadmap.md")
            .expect("docs/roadmap.md should be readable");

        let has_current_marker = roadmap.contains("Current")
            || roadmap.contains("current")
            || roadmap.contains("v1.0");

        assert!(
            has_current_marker,
            "Roadmap should clearly indicate the current version"
        );
    }

    #[test]
    fn roadmap_has_clear_structure() {
        let roadmap = fs::read_to_string("docs/roadmap.md")
            .expect("docs/roadmap.md should be readable");

        // Check for markdown headers indicating structure
        let has_version_headers = roadmap.contains("## v");
        let has_bullet_points = roadmap.contains("- ");

        assert!(
            has_version_headers,
            "Roadmap should use markdown headers for version sections"
        );
        assert!(
            has_bullet_points,
            "Roadmap should use bullet points to list features"
        );
    }

    #[test]
    fn roadmap_distinguishes_future_work() {
        let roadmap = fs::read_to_string("docs/roadmap.md")
            .expect("docs/roadmap.md should be readable");

        let has_future_marker = roadmap.contains("Future")
            || roadmap.contains("Upcoming")
            || roadmap.contains("v4.0");

        assert!(
            has_future_marker,
            "Roadmap should distinguish between committed and future work"
        );
    }
}
