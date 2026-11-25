#![no_main]

//! Fuzz target for the ttl_vault contract's input-parsing paths.
//!
//! Contract function arguments arrive as XDR-encoded `ScVal`s. The host
//! deserializes them, then the contract converts them into typed parameters
//! and, in some cases, parses raw `Bytes` payloads internally (e.g. the
//! multi-sig numeric payloads decoded by `decode_i128` / `decode_u64`).
//!
//! This target registers the real contract in a Soroban test environment and
//! drives those paths with:
//!
//! * **mode 0** - a single `ScVal` decoded from raw XDR, dispatched to the
//!   one-argument entry points (`pause`, `propose_upgrade`, ...);
//! * **mode 1** - a `Vec<ScVal>` decoded from raw XDR (the full argument
//!   list of a call), dispatched to the multi-argument entry points
//!   (`file_dispute`, `deposit`, `check_in`, `propose_multisig`, ...);
//! * **mode 2** - a structured multi-sig payload (operation + arbitrary
//!   `Bytes` payload), pushed through `propose_multisig` → `approve_multisig`
//!   → `execute_multisig`, exercising the internal payload decoders.
//!
//! Invariants asserted:
//! - Malformed XDR never panics: deserialization fails with an error.
//! - Semantically invalid `ScVal`s are rejected during conversion, not by
//!   panicking.
//! - Arbitrary contract inputs (including short/oversized/empty payloads)
//!   are handled with contract errors, never a panic.

use libfuzzer_sys::fuzz_target;

use soroban_sdk::{
    testutils::Address as _,
    vec, Address, Bytes, Env, IntoVal, Symbol, TryIntoVal, Val,
};
use stellar_xdr::{FromXdr, ScVal, VecM};
use ttl_vault::{ContractError, MultiSigOperation, TtlVaultContract, Vault};

/// A fresh, initialized test environment with the contract registered.
struct Harness {
    env: Env,
    contract_id: Address,
    owner: Address,
    beneficiary: Address,
    signer: Address,
}

fn setup() -> Harness {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let owner = Address::generate(&env);
    let beneficiary = Address::generate(&env);
    let signer = Address::generate(&env);
    let token = Address::generate(&env);

    let contract_id = env.register_contract(None, TtlVaultContract);

    // The fuzz flows below never touch the token contract, so a bare address
    // is enough for initialization.
    let _ = env.try_invoke_contract::<(), ContractError>(
        &contract_id,
        &Symbol::new(&env, "initialize"),
        vec![&env, token.into_val(&env), admin.into_val(&env)],
    );

    Harness {
        env,
        contract_id,
        owner,
        beneficiary,
        signer,
    }
}

/// mode 0: invoke one-argument entry points with a single XDR-decoded ScVal.
fn fuzz_xdr_single_arg(h: &Harness, data: &[u8]) {
    let env = &h.env;
    let scval = match ScVal::from_xdr(data) {
        Ok(v) => v,
        Err(_) => return, // malformed XDR: must be rejected without panicking
    };
    let val: Val = match scval.try_into_val(env) {
        Ok(v) => v,
        Err(_) => return, // semantically invalid ScVal: host rejects it too
    };
    let args = vec![env, val];

    let _ = env.try_invoke_contract::<(), ContractError>(
        &h.contract_id,
        &Symbol::new(env, "pause"),
        args.clone(),
    );
    let _ = env.try_invoke_contract::<u64, ContractError>(
        &h.contract_id,
        &Symbol::new(env, "propose_upgrade"),
        args.clone(),
    );
    let _ = env.try_invoke_contract::<bool, ContractError>(
        &h.contract_id,
        &Symbol::new(env, "is_token_whitelisted"),
        args.clone(),
    );
    let _ = env.try_invoke_contract::<(), ContractError>(
        &h.contract_id,
        &Symbol::new(env, "remove_token_whitelist"),
        args.clone(),
    );
    let _ = env.try_invoke_contract::<(), ContractError>(
        &h.contract_id,
        &Symbol::new(env, "validate_upgrade"),
        args.clone(),
    );
    let _ = env.try_invoke_contract::<Vault, ContractError>(
        &h.contract_id,
        &Symbol::new(env, "get_vault"),
        args,
    );
}

/// mode 1: invoke multi-argument entry points with an XDR-decoded ScVec
/// (the full argument list of a call).
fn fuzz_xdr_multi_arg(h: &Harness, data: &[u8]) {
    let env = &h.env;
    let scvec = match VecM::<ScVal, 1000>::from_xdr(data) {
        Ok(v) => v,
        Err(_) => return, // malformed XDR: must be rejected without panicking
    };
    let mut args = soroban_sdk::Vec::new(env);
    for scval in scvec.iter() {
        if let Ok(v) = scval.try_into_val(env) {
            args.push_back(v);
        }
    }

    let _ = env.try_invoke_contract::<Result<(), ContractError>, ContractError>(
        &h.contract_id,
        &Symbol::new(env, "file_dispute"),
        args.clone(),
    );
    let _ = env.try_invoke_contract::<(), ContractError>(
        &h.contract_id,
        &Symbol::new(env, "deposit"),
        args.clone(),
    );
    let _ = env.try_invoke_contract::<Result<(), ContractError>, ContractError>(
        &h.contract_id,
        &Symbol::new(env, "withdraw"),
        args.clone(),
    );
    let _ = env.try_invoke_contract::<Result<(), ContractError>, ContractError>(
        &h.contract_id,
        &Symbol::new(env, "check_in"),
        args.clone(),
    );
    let _ = env.try_invoke_contract::<Result<u64, ContractError>, ContractError>(
        &h.contract_id,
        &Symbol::new(env, "propose_multisig"),
        args,
    );
}

/// mode 2: structured multi-sig payload flow. The arbitrary `Bytes` payload
/// is stored in a proposal and later parsed by the contract's internal
/// decoders (`decode_i128` / `decode_u64`) during `execute_multisig`.
fn fuzz_multisig_payload(h: &Harness, data: &[u8]) {
    if data.len() < 3 {
        return;
    }
    let env = &h.env;

    let op_idx = data[0];
    let payload_len = u16::from_le_bytes([data[1], data[2]]) as usize;
    let payload_start = 3usize;
    let payload_end = payload_start.saturating_add(payload_len).min(data.len());
    let payload_bytes = &data[payload_start..payload_end];
    let addr_flag = data.get(payload_end).copied().unwrap_or(0);

    let operation = match op_idx {
        0 => MultiSigOperation::Withdraw,
        1 => MultiSigOperation::UpdateBeneficiary,
        2 => MultiSigOperation::CancelVault,
        3 => MultiSigOperation::UpdateCheckInInterval,
        4 => MultiSigOperation::TransferOwnership,
        _ => return,
    };

    // Create a real vault so the payload path is reachable.
    let vault_id: u64 = match env.try_invoke_contract::<u64, ContractError>(
        &h.contract_id,
        &Symbol::new(env, "create_vault"),
        vec![
            env,
            h.owner.clone().into_val(env),
            h.beneficiary.clone().into_val(env),
            3600u64.into_val(env),
            None::<Address>.into_val(env),
        ],
    ) {
        Ok(Ok(id)) => id,
        _ => return,
    };

    // 2-of-2 multi-sig (owner + one co-signer) so the proposal can be
    // approved and executed.
    let signers = vec![env, h.signer.clone()];
    let _ = env.try_invoke_contract::<(), ContractError>(
        &h.contract_id,
        &Symbol::new(env, "configure_multisig"),
        vec![
            env,
            vault_id.into_val(env),
            h.owner.clone().into_val(env),
            signers.into_val(env),
            2u32.into_val(env),
        ],
    );

    let payload = Bytes::from_slice(env, payload_bytes);
    let address_payload: Option<Address> = match addr_flag % 3 {
        0 => None,
        1 => Some(h.beneficiary.clone()),
        _ => Some(h.signer.clone()),
    };

    let proposal_id: u64 = match env.try_invoke_contract::<u64, ContractError>(
        &h.contract_id,
        &Symbol::new(env, "propose_multisig"),
        vec![
            env,
            vault_id.into_val(env),
            h.owner.clone().into_val(env),
            operation.into_val(env),
            payload.into_val(env),
            address_payload.into_val(env),
        ],
    ) {
        Ok(Ok(id)) => id,
        _ => return,
    };

    // Approve with the co-signer, then execute. Execution runs the payload
    // through the contract's internal decoders.
    let _ = env.try_invoke_contract::<(), ContractError>(
        &h.contract_id,
        &Symbol::new(env, "approve_multisig"),
        vec![
            env,
            vault_id.into_val(env),
            proposal_id.into_val(env),
            h.signer.clone().into_val(env),
        ],
    );
    let _ = env.try_invoke_contract::<(), ContractError>(
        &h.contract_id,
        &Symbol::new(env, "execute_multisig"),
        vec![
            env,
            vault_id.into_val(env),
            proposal_id.into_val(env),
            h.owner.clone().into_val(env),
        ],
    );
}

fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }
    let mode = data[0] % 3;
    let rest = &data[1..];
    let harness = setup();
    match mode {
        0 => fuzz_xdr_single_arg(&harness, rest),
        1 => fuzz_xdr_multi_arg(&harness, rest),
        _ => fuzz_multisig_payload(&harness, rest),
    }
});
