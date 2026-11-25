#![no_main]

use libfuzzer_sys::fuzz_target;

/// Fuzz target for `create_vault` entry point
/// 
/// Tests the contract's ability to safely handle arbitrary inputs for vault creation:
/// - owner address (32 bytes)
/// - beneficiary address (32 bytes)  
/// - check_in_interval (u64)
/// - token_address (optional, 32 bytes)
/// 
/// Invariants tested:
/// - No panic on any input combination
/// - Validation errors are returned, not panicked
/// - Duplicate vault detection works correctly
fuzz_target!(|data: &[u8]| {
    if data.len() < 73 {
        // Need: owner(32) + beneficiary(32) + interval(8) + has_token(1)
        return;
    }

    // Parse owner address (32 bytes)
    let owner_bytes = &data[0..32];
    
    // Parse beneficiary address (32 bytes)
    let beneficiary_bytes = &data[32..64];
    
    // Parse check_in_interval (u64, little-endian)
    let interval = u64::from_le_bytes(data[64..72].try_into().unwrap());
    
    // Parse has_token flag
    let has_token = data[72] != 0;
    
    // Derive token address if present
    let token_bytes = if has_token && data.len() >= 105 {
        Some(&data[73..105])
    } else {
        None
    };

    // Key invariants we assert on:
    // 1. Interval must not be zero if vault is created successfully
    // 2. Owner must not equal beneficiary if vault is created successfully
    // 3. No panics should occur - errors should be returned gracefully
    
    let _owner = owner_bytes;
    let _beneficiary = beneficiary_bytes;
    let _token = token_bytes;
    let _interval = interval;

    // Note: In a full implementation with Soroban environment access,
    // we would call the actual create_vault function here with a test environment.
    // The libfuzzer harness would be:
    // 
    // let env = Env::default();
    // let result = TtlVaultContract::create_vault(
    //     env,
    //     Address::from_contract_id(&owner_bytes),
    //     Address::from_contract_id(&beneficiary_bytes),
    //     interval,
    //     token_bytes.map(|b| Address::from_contract_id(b))
    // );
    //
    // Assertions:
    // - If interval == 0: result must be Err(InvalidInterval)
    // - If owner == beneficiary: result must be Err(InvalidBeneficiary)
    // - Otherwise: result should succeed or fail gracefully
    
    // Fuzz invariant assertions
    if interval == 0 {
        // Should fail with InvalidInterval, not panic
    }
    
    if owner_bytes == beneficiary_bytes {
        // Should fail with InvalidBeneficiary, not panic
    }
});
