#![no_main]

//! Fuzz target for the XDR input-deserialization path of contract calls.
//!
//! Soroban contract arguments are XDR-encoded `ScVal`s. When a transaction is
//! submitted, the host deserializes the raw bytes into `ScVal` / `ScVec`
//! before the contract function is invoked. This target feeds arbitrary
//! bytes through exactly that deserialization and asserts that malformed
//! XDR is rejected with an error rather than a panic, and that any value
//! that does deserialize survives a lossless encode/decode round trip.
//!
//! It also fuzzes the contract-internal payload parsers used by the multi-sig
//! path (`decode_i128` / `decode_u64`), which interpret raw `Bytes` payloads
//! as little-endian integers.

use libfuzzer_sys::fuzz_target;
use stellar_xdr::{FromXdr, ScVal, ToXdr, VecM};

/// Mirrors `ttl_vault::TtlVaultContract::decode_i128` (multi-sig payloads):
/// a 16-byte little-endian i128; payloads shorter than 16 bytes are rejected
/// with an error instead of panicking.
fn decode_i128(payload: &[u8]) -> Result<i128, ()> {
    if payload.len() < 16 {
        return Err(());
    }
    let mut buf = [0u8; 16];
    buf.copy_from_slice(&payload[..16]);
    Ok(i128::from_le_bytes(buf))
}

/// Mirrors `ttl_vault::TtlVaultContract::decode_u64` (multi-sig payloads):
/// an 8-byte little-endian u64; payloads shorter than 8 bytes are rejected
/// with an error instead of panicking.
fn decode_u64(payload: &[u8]) -> Result<u64, ()> {
    if payload.len() < 8 {
        return Err(());
    }
    let mut buf = [0u8; 8];
    buf.copy_from_slice(&payload[..8]);
    Ok(u64::from_le_bytes(buf))
}

fuzz_target!(|data: &[u8]| {
    // 1. Single contract argument: raw XDR -> ScVal.
    match ScVal::from_xdr(data) {
        Ok(scval) => {
            // Re-encoding a successfully decoded value must succeed and
            // decode back to the same value (lossless round trip).
            let encoded = scval.to_xdr().expect("re-encoding a valid ScVal must not fail");
            let decoded =
                ScVal::from_xdr(&encoded).expect("decoding a re-encoded ScVal must not fail");
            assert_eq!(decoded, scval, "ScVal XDR decode -> encode -> decode must be stable");
        }
        Err(_) => {
            // Malformed XDR must be rejected gracefully (no panic).
        }
    }

    // 2. Full argument list: raw XDR -> Vec<ScVal>, the form the host passes
    //    to contract functions with multiple arguments.
    match VecM::<ScVal, 1000>::from_xdr(data) {
        Ok(scvec) => {
            let encoded = scvec
                .to_xdr()
                .expect("re-encoding a valid ScVec must not fail");
            let decoded = VecM::<ScVal, 1000>::from_xdr(&encoded)
                .expect("decoding a re-encoded ScVec must not fail");
            assert_eq!(decoded, scvec, "ScVec XDR decode -> encode -> decode must be stable");
        }
        Err(_) => {
            // Malformed XDR must be rejected gracefully (no panic).
        }
    }

    // 3. Contract-internal payload parsing (multi-sig numeric payloads).
    //    Arbitrary payload bytes must never panic the decoders.
    let _ = decode_i128(data);
    let _ = decode_u64(data);
});
