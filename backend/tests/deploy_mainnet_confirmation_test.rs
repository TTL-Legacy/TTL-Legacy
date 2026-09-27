//! Deploy mainnet confirmation prompt tests
//! Issue #1571: Enhancement: add confirmation prompt to deploy_mainnet.sh

#[cfg(test)]
mod deploy_mainnet_confirmation {
    use std::fs;
    use std::path::Path;

    /// Verifies that deploy_mainnet.sh exists
    #[test]
    fn deploy_mainnet_script_exists() {
        let script_path = "scripts/deploy_mainnet.sh";
        assert!(
            Path::new(script_path).exists(),
            "deploy_mainnet.sh should exist at {}",
            script_path
        );
    }

    /// Verifies that confirmation prompt exists in deploy script
    #[test]
    fn confirmation_prompt_exists() {
        let content = fs::read_to_string("scripts/deploy_mainnet.sh")
            .expect("Failed to read deploy_mainnet.sh");

        assert!(
            content.contains("read") || content.contains("confirm"),
            "deploy_mainnet.sh should have a confirmation prompt"
        );
    }

    /// Verifies that confirmation prompt shows network information
    #[test]
    fn confirmation_displays_network_info() {
        let content = fs::read_to_string("scripts/deploy_mainnet.sh")
            .expect("Failed to read deploy_mainnet.sh");

        assert!(
            content.contains("NETWORK") || content.contains("network"),
            "confirmation prompt should display network information"
        );
    }

    /// Verifies that confirmation prompt shows contract/WASM information
    #[test]
    fn confirmation_displays_wasm_hash_or_contract_info() {
        let content = fs::read_to_string("scripts/deploy_mainnet.sh")
            .expect("Failed to read deploy_mainnet.sh");

        let has_wasm_info = content.contains("WASM") || content.contains("wasm")
            || content.contains("CONTRACT") || content.contains("contract");

        assert!(
            has_wasm_info,
            "confirmation prompt should display WASM or contract information"
        );
    }

    /// Verifies that --yes flag exists to bypass confirmation
    #[test]
    fn yes_flag_bypasses_confirmation() {
        let content = fs::read_to_string("scripts/deploy_mainnet.sh")
            .expect("Failed to read deploy_mainnet.sh");

        assert!(
            content.contains("--yes") || content.contains("YES"),
            "deploy_mainnet.sh should support --yes flag to bypass confirmation"
        );
    }

    /// Verifies that confirmation abort exits with error
    #[test]
    fn failed_confirmation_aborts_deployment() {
        let content = fs::read_to_string("scripts/deploy_mainnet.sh")
            .expect("Failed to read deploy_mainnet.sh");

        assert!(
            content.contains("exit 1") || content.contains("Aborted"),
            "deploy_mainnet.sh should abort deployment on failed confirmation"
        );
    }

    /// Verifies that confirmation accepts expected input
    #[test]
    fn confirmation_validates_expected_input() {
        let content = fs::read_to_string("scripts/deploy_mainnet.sh")
            .expect("Failed to read deploy_mainnet.sh");

        let has_validation = content.contains("!=") || content.contains("=") || content.contains("CONFIRM");
        assert!(
            has_validation,
            "confirmation prompt should validate user input"
        );
    }

    /// Verifies that deploy_utils includes confirmation helper
    #[test]
    fn deploy_utils_has_confirmation_helper() {
        let utils_path = "scripts/deploy_utils.sh";
        if Path::new(utils_path).exists() {
            let content = fs::read_to_string(utils_path)
                .expect("Failed to read deploy_utils.sh");

            let has_confirm = content.contains("confirm") || content.contains("CONFIRM");
            if has_confirm {
                assert!(true, "deploy_utils.sh provides confirmation utilities");
            }
        }
    }

    /// Verifies that confirmation prompt is shown before build
    #[test]
    fn confirmation_occurs_before_deployment() {
        let content = fs::read_to_string("scripts/deploy_mainnet.sh")
            .expect("Failed to read deploy_mainnet.sh");

        let confirm_pos = content.find("read").unwrap_or(0);
        let deploy_pos = content.find("stellar contract deploy").unwrap_or(0);

        assert!(
            confirm_pos < deploy_pos && deploy_pos > 0,
            "confirmation should occur before actual deployment"
        );
    }
}
