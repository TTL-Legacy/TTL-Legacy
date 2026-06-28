# ZK Verifier Contract: Stub Implementation & Migration Path

## Overview

The `zk_verifier` contract in TTL-Legacy is currently a **stub implementation** designed to validate the architectural integration and provide a path toward full zero-knowledge proof verification in future versions.

This document explains:
1. Why it is a stub
2. Current security properties and limitations
3. What a real ZK implementation requires
4. Roadmap for full ZK support

---

## Current Stub Implementation

### What the Stub Does

The stub `zk_verifier` contract provides:

- **Oracle attestation registration**: Trusted oracles can register and publish attestations
- **Proof/claim validation**: Performs basic input validation and sentinel checks
- **Event emission**: Publishes verification results to the network
- **SHA-256 hashing**: Stores digest-based proofs to minimize on-chain storage

### Signature of Key Functions

```rust
/// Initialize the contract with an admin address.
pub fn initialize(env: Env, admin: Address)

/// Register a trusted oracle. Admin only.
pub fn register_oracle(env: Env, oracle: Address)

/// Revoke a trusted oracle. Admin only.
pub fn revoke_oracle(env: Env, oracle: Address)

/// Returns whether the given address is a registered oracle.
pub fn is_oracle(env: Env, oracle: Address) -> bool

/// An oracle publishes an attestation that `proof` is valid for `claim`.
pub fn attest(env: Env, oracle: Address, proof: Bytes, claim: Bytes)

/// Verifies a zero-knowledge proof against a claim using oracle attestation.
pub fn verify_claim(env: Env, proof: Bytes, claim: Bytes) -> bool
```

### Current Verification Logic

The `verify_claim` function:

1. **Validates input bounds**:
   - Rejects empty or oversized proofs (max 4 KB)
   - Rejects empty or oversized claims (max 1 KB)

2. **Applies sentinel check**:
   - Returns `false` if proof is exactly `0x00` (known-invalid sentinel)
   - Returns `true` for all other non-empty proofs

3. **Computes claim hash**:
   - SHA-256 digest of the claim bytes
   - Published in a `vfy_claim` event for off-chain indexing

4. **Emits event**:
   ```
   vfy_claim: (result: bool, claim_hash: BytesN<32>)
   ```

### Security Limitations of the Stub

⚠️ **The stub is NOT suitable for production use where actual proof verification matters.**

| Aspect | Stub Behavior | Security Impact |
|--------|---------------|-----------------|
| **Proof Verification** | Accepts all non-0x00 proofs as valid | Anyone can forge valid "proofs" |
| **Cryptographic Soundness** | None (sentinel-based only) | Claims can be falsely validated |
| **Oracle Trust Model** | Attestations stored but not used | Malicious oracles can bypass checks |
| **Claim Authentication** | Stored as SHA-256 digest only | No binding between claim and proof |
| **Replay Protection** | None | Same proof/claim can be reused indefinitely |

---

## Why It Is a Stub

### Technical Reasons

1. **Soroban Host Functions Unavailable**: Full ZK verification (e.g., Groth16) requires:
   - Pairing operations (elliptic curve)
   - Field arithmetic (BN128, BLS12-381)
   - Custom host functions not yet exposed in Soroban v20.x

2. **Storage Constraints**: On-chain ZK proofs are large:
   - Groth16 proof: ~288 bytes (manageable)
   - Verification key: ~3-5 KB (significant)
   - Vkey storage per proof set becomes prohibitive

3. **Performance**: Cryptographic operations on-chain are expensive:
   - BN128 pairing: ~50-100ms per proof
   - Contract execution cost scales with complexity
   - Threshold pricing makes frequent verification costly

4. **Trusted Setup Requirement**: Groth16 needs a trusted setup ceremony:
   - Parameters must be securely generated
   - Cannot be deployed without external coordination
   - Increases complexity and trust assumptions

### Design Philosophy

The stub allows:

- ✅ Architecture validation (prove the contract model works)
- ✅ Oracle integration testing (validate the attestation flow)
- ✅ Event emission patterns (ensure off-chain indexing works)
- ✅ Future migration (clear upgrade path to real ZK)

---

## What a Real ZK Implementation Requires

### 1. Cryptographic Backend

**Current Limitation**: Soroban lacks pairing-friendly elliptic curve host functions.

**Required**: At least one of:

- **Groth16** (BN128 or BLS12-381):
  - Most compact proofs (~288 bytes)
  - Fastest verification (single pairing check)
  - Widely supported in ZK frameworks

- **PLONK** (Generic elliptic curves):
  - Longer proofs (~5-10 KB)
  - Flexible setup ceremony
  - Better for multiple proof systems

- **Bulletproofs**:
  - Smaller proofs than PLONK
  - Transparent setup (no trusted ceremony)
  - Slower verification

### 2. Host Functions Needed

Soroban must expose (or TTL-Legacy must implement as a precompile):

```rust
/// Verify a Groth16 proof
fn groth16_verify(
    vkey: BytesN<4096>,        // Verification key
    proof: BytesN<288>,         // Groth16 proof
    pub_inputs: Vec<u256>       // Public inputs
) -> bool

/// Verify a PLONK proof  
fn plonk_verify(
    vkey: Bytes,                // Verification key (variable size)
    proof: Bytes,               // PLONK proof
    pub_inputs: Vec<Scalar>     // Public inputs
) -> bool
```

### 3. Trusted Setup (if using Groth16)

**Setup Ceremony Output**:

```
Common Reference String (CRS)
├── Proving Key (prover side, not stored on-chain)
├── Verification Key (stored on-chain, immutable)
└── Toxic Waste (destroyed, never stored)
```

**Ceremony Participants**: Minimum 3-5 independent parties running the MPC protocol.

**TTL-Legacy Integration**:

- Verification keys stored in contract
- New key registration requires multi-sig approval
- Audit trail for all key changes

### 4. Circuit Implementation

**What the circuit must prove**:

```rust
// Example: Prove vault ownership without revealing private key
circuit prove_vault_access(
    owner_secret: Field,           // Hidden input
    vault_salt: Field,             // Public input
    commitment: Field              // Public commitment
) {
    // Verify: hash(owner_secret, vault_salt) == commitment
    let derived = poseidon_hash([owner_secret, vault_salt]);
    assert(derived == commitment);
}
```

### 5. Performance Envelope

**Target metrics for production**:

| Metric | Groth16 | PLONK | Bulletproofs |
|--------|---------|-------|--------------|
| **Proof Size** | 288 B | 5-10 KB | 2-4 KB |
| **On-Chain Verification** | ~5 ms | ~50 ms | ~500 ms |
| **Setup Time** | Hours (ceremony) | Minutes | Transparent |
| **Prover Time** | ~100 ms | ~500 ms | ~5 s |
| **Memory** | ~1 GB | ~2 GB | ~500 MB |

---

## Roadmap for Full ZK Implementation

### Phase 1: Foundation (Soroban v21.x - Q4 2026)

**Deliverables**:

- [ ] Soroban exposes scalar multiplication for BN128
- [ ] TTL-Legacy implements Miller-Rabin for fast BN128 pairing
- [ ] Verification key registration framework
- [ ] Test circuit with Groth16 (via circom + snarkjs)

**Effort**: 3-4 weeks

### Phase 2: Groth16 Verifier (Soroban v22.x - Q1 2027)

**Deliverables**:

- [ ] Full Groth16 verifier in Rust (without host functions, naive)
- [ ] Integration with TTL-Legacy contract
- [ ] Trusted setup ceremony documentation
- [ ] Example: vault access proof

**Effort**: 4-6 weeks

**Status**: Groth16 without optimized host functions is slow (~50-100 ms per proof).

### Phase 3: Optimized Verification (Soroban v23.x - Q2 2027)

**Deliverables**:

- [ ] Native BN128 pairing in Soroban
- [ ] Fast Groth16 verifier (~5-10 ms per proof)
- [ ] PLONK verifier as alternative
- [ ] Proof batching support (verify N proofs in one transaction)

**Effort**: 6-8 weeks

### Phase 4: Productionization (Q3 2027)

**Deliverables**:

- [ ] Security audit of verifier code
- [ ] Trusted setup ceremony execution
- [ ] Circuit library for common proving tasks
- [ ] CLI tools for proof generation
- [ ] Off-chain prover integration

**Effort**: 8-10 weeks

**Timeline**: ~6 months from Phase 1 start.

---

## Using the Stub Today

### For Testing

The stub is suitable for:

1. **Architecture validation**: Confirm the contract model integrates correctly
2. **Oracle testing**: Verify the attestation workflow
3. **Event indexing**: Test off-chain listening
4. **Integration tests**: Mock ZK verification for workflow testing

### Example Test Flow

```rust
#[test]
fn test_oracle_attestation_flow() {
    let env = Env::default();
    let contract = ZkVerifierContract::new(&env);
    
    // 1. Initialize with admin
    let admin = Address::generate(&env);
    contract.initialize(admin.clone());
    
    // 2. Register oracle
    let oracle = Address::generate(&env);
    contract.register_oracle(oracle.clone());
    
    // 3. Oracle attests
    let proof = Bytes::from_slice(&env, &[1, 2, 3]);
    let claim = Bytes::from_slice(&env, &[4, 5, 6]);
    contract.attest(oracle.clone(), proof.clone(), claim.clone());
    
    // 4. Verify claim
    let result = contract.verify_claim(proof, claim);
    assert!(result);  // Stub always returns true for non-0x00 proofs
}
```

### For Production

**Do NOT use the stub for**:

- ❌ Actual proof verification security
- ❌ Authorization decisions based on proofs
- ❌ Financial transactions gated by ZK
- ❌ Any security-critical logic

---

## Error Codes

| Code | Name | Description |
|------|------|-------------|
| 1 | EmptyProof | Proof bytes are empty |
| 2 | EmptyClaim | Claim bytes are empty |
| 3 | ProofTooLarge | Proof exceeds 4 KB |
| 4 | ClaimTooLarge | Claim exceeds 1 KB |
| 5 | AlreadyInitialized | Contract already initialized |
| 6 | OracleNotFound | Oracle address not registered |
| 7 | Unauthorized | Caller is not authorized |

---

## References

- [Groth16: Succinct Non-Interactive Zero Knowledge for a von Neumann Architecture](https://eprint.iacr.org/2016/260)
- [PLONK: Permutations over Lagrange-bases for Oecumenical Noninteractive arguments of Knowledge](https://eprint.iacr.org/2019/953)
- [Bulletproofs: Short Proofs for Confidential Transactions and More](https://eprint.iacr.org/2017/1066)
- [Stellar Soroban Documentation](https://developers.stellar.org/docs/learn/soroban)
- [circom: Circuit Compiler](https://github.com/iden3/circom)
- [snarkjs: ZK Proof Generation](https://github.com/iden3/snarkjs)

---

## Migration Notes

When Soroban adds native ZK support or TTL-Legacy implements a verifier:

1. **Update** `verify_claim` implementation (drop sentinel check)
2. **Add** verification key management endpoints
3. **Emit** circuit-specific events
4. **Maintain** backward compatibility where possible
5. **Deprecate** oracle attestation flow (if not needed)

The stub contract serves as a forward-compatible skeleton for this transition.
