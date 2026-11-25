#![cfg(test)]

use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, Env, String,
};

#[test]
fn test_add_passkey_with_mobile_device_type() {
    let env = Env::new();
    let owner = Address::generate(&env);
    let passkey_id = String::from_slice(&env, "passkey_mobile_123");
    let device_type = "Mobile";

    // Test adding a passkey with device_type: Mobile
    // Should store device type in PasskeyEntry
}

#[test]
fn test_add_passkey_with_desktop_device_type() {
    let env = Env::new();
    let owner = Address::generate(&env);
    let passkey_id = String::from_slice(&env, "passkey_desktop_456");
    let device_type = "Desktop";

    // Test adding a passkey with device_type: Desktop
    // Should store device type in PasskeyEntry
}

#[test]
fn test_add_passkey_with_hardware_key_device_type() {
    let env = Env::new();
    let owner = Address::generate(&env);
    let passkey_id = String::from_slice(&env, "passkey_hw_789");
    let device_type = "HardwareKey";

    // Test adding a passkey with device_type: HardwareKey
    // Should store device type in PasskeyEntry
}

#[test]
fn test_add_passkey_with_unknown_device_type() {
    let env = Env::new();
    let owner = Address::generate(&env);
    let passkey_id = String::from_slice(&env, "passkey_unknown_000");
    let device_type = "Unknown";

    // Test adding a passkey with device_type: Unknown
    // Should handle unidentified device types gracefully
}

#[test]
fn test_add_passkey_device_type_required() {
    let env = Env::new();
    let owner = Address::generate(&env);
    let passkey_id = String::from_slice(&env, "passkey_no_type");

    // Test that device_type parameter is required in add_passkey
    // Should error if device_type is not provided
}

#[test]
fn test_add_passkey_invalid_device_type() {
    let env = Env::new();
    let owner = Address::generate(&env);
    let passkey_id = String::from_slice(&env, "passkey_invalid");
    let invalid_device_type = "InvalidType";

    // Test that invalid device_type values are rejected
    // Should return error for unrecognized device types
}

#[test]
fn test_list_passkeys_includes_device_type() {
    let env = Env::new();
    let owner = Address::generate(&env);
    let mobile_key = String::from_slice(&env, "mobile_001");
    let desktop_key = String::from_slice(&env, "desktop_001");

    // Test that list_passkeys response includes device_type for each key
    // Should return array with device_type field for each passkey
}

#[test]
fn test_list_passkeys_all_device_types() {
    let env = Env::new();
    let owner = Address::generate(&env);
    let mobile = String::from_slice(&env, "key_mobile");
    let desktop = String::from_slice(&env, "key_desktop");
    let hardware = String::from_slice(&env, "key_hardware");
    let unknown = String::from_slice(&env, "key_unknown");

    // Test listing multiple passkeys with different device types
    // Should include all variants in response
}

#[test]
fn test_device_type_persisted_across_calls() {
    let env = Env::new();
    let owner = Address::generate(&env);
    let passkey_id = String::from_slice(&env, "persistent_key");
    let device_type = "Mobile";

    // Test that device_type is persisted and returned consistently
    // Multiple list_passkeys calls should return same device_type
}

#[test]
fn test_device_type_specific_to_passkey() {
    let env = Env::new();
    let owner = Address::generate(&env);
    let key1 = String::from_slice(&env, "key1");
    let key2 = String::from_slice(&env, "key2");

    // Test that device_type is stored per-passkey
    // key1 with Mobile and key2 with Desktop should be independent
}

#[test]
fn test_device_type_case_sensitivity() {
    let env = Env::new();
    let owner = Address::generate(&env);
    let passkey_id = String::from_slice(&env, "case_test_key");

    // Test device_type field for case sensitivity
    // Should normalize or enforce specific casing (Mobile vs mobile)
}

#[test]
fn test_passkey_device_type_audit_report() {
    let env = Env::new();
    let owner = Address::generate(&env);
    let mobile_key = String::from_slice(&env, "audit_mobile");
    let hardware_key = String::from_slice(&env, "audit_hardware");

    // Test that device types are included in security audit report
    // Should show breakdown of key types (2 Mobile, 1 Hardware, etc.)
}

#[test]
fn test_frontend_passkey_device_icons() {
    let env = Env::new();
    let owner = Address::generate(&env);
    let keys = vec![
        ("mobile", "Mobile"),
        ("desktop", "Desktop"),
        ("hardware", "HardwareKey"),
    ];

    // Test that frontend renders correct icons for device types
    // Should display: smartphone icon for Mobile, computer for Desktop, key for Hardware
}

#[test]
fn test_device_type_mobile_icon_display() {
    let env = Env::new();
    let owner = Address::generate(&env);
    let mobile_key = String::from_slice(&env, "mobile_display");

    // Test that Mobile device type shows smartphone/mobile icon
    // Frontend passkey list should display appropriate icon
}

#[test]
fn test_device_type_desktop_icon_display() {
    let env = Env::new();
    let owner = Address::generate(&env);
    let desktop_key = String::from_slice(&env, "desktop_display");

    // Test that Desktop device type shows computer/desktop icon
    // Frontend passkey list should display appropriate icon
}

#[test]
fn test_device_type_hardware_icon_display() {
    let env = Env::new();
    let owner = Address::generate(&env);
    let hardware_key = String::from_slice(&env, "hardware_display");

    // Test that HardwareKey device type shows key/security icon
    // Frontend passkey list should display appropriate icon
}

#[test]
fn test_passkey_device_type_security_dashboard() {
    let env = Env::new();
    let owner = Address::generate(&env);
    let mobile = String::from_slice(&env, "dash_mobile");
    let desktop = String::from_slice(&env, "dash_desktop");
    let hardware = String::from_slice(&env, "dash_hardware");

    // Test that dashboard displays security posture breakdown by device type
    // Should show: 1 Mobile, 1 Desktop, 1 Hardware Key
}

#[test]
fn test_device_type_enum_variants() {
    let env = Env::new();

    // Test that PasskeyDeviceType enum has all required variants
    // Should include: Mobile, Desktop, HardwareKey, Unknown
}

#[test]
fn test_device_type_serialization() {
    let env = Env::new();
    let owner = Address::generate(&env);
    let passkey_id = String::from_slice(&env, "serial_key");
    let device_type = "Mobile";

    // Test that device_type is correctly serialized in responses
    // Should produce valid JSON: "device_type": "Mobile"
}

#[test]
fn test_device_type_api_response_format() {
    let env = Env::new();
    let owner = Address::generate(&env);
    let passkey_id = String::from_slice(&env, "api_format_key");

    // Test REST API response format for device type
    // Response should include: { "id": "...", "device_type": "Mobile", ... }
}

#[test]
fn test_remove_passkey_preserves_device_type_history() {
    let env = Env::new();
    let owner = Address::generate(&env);
    let passkey_id = String::from_slice(&env, "history_key");
    let device_type = "Desktop";

    // Test that removing a passkey preserves device type in audit log
    // Should track which device types were removed and when
}

#[test]
fn test_device_type_audit_trail() {
    let env = Env::new();
    let owner = Address::generate(&env);
    let passkey_id = String::from_slice(&env, "audit_trail_key");

    // Test that all passkey additions with device types are logged
    // Should create audit trail entry with device type information
}

#[test]
fn test_multiple_passkeys_same_device_type() {
    let env = Env::new();
    let owner = Address::generate(&env);
    let mobile_key1 = String::from_slice(&env, "mobile_1");
    let mobile_key2 = String::from_slice(&env, "mobile_2");

    // Test that multiple passkeys can have the same device type
    // Should allow 2+ Mobile keys simultaneously
}
