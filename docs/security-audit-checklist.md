# Security Audit Checklist

Use this checklist before every release and as a guide for external auditors. Each item should be marked ✅ (pass), ❌ (fail), or N/A.

Related documents: [Threat Model & Security](security.md) · [Security Policy](../SECURITY.md)

---

## 1. Authentication & Authorization

- [ ] Every owner action calls `owner.require_auth()`
- [ ] Every admin action calls `admin.require_auth()`
- [ ] `initialize()` rejects a second call (`AlreadyInitialized`)
- [ ] `propose_admin` / `accept_admin` two-step transfer is enforced
- [ ] Passkey hash is validated before accepting a check-in
- [ ] Backup codes are single-use and marked `used = true` after consumption
- [ ] Beneficiary cannot trigger release before TTL expiry

## 2. Reentrancy

- [ ] All state mutations (balance, status) are written **before** token transfers
- [ ] `trigger_release` sets `vault.status = Released` before calling `token.transfer`
- [ ] `claim_vested_installment` decrements balance before transferring
- [ ] No external calls are made between reading and writing vault state

## 3. Integer Arithmetic

- [ ] All balance additions use `checked_add` or `saturating_add` to prevent overflow
- [ ] BPS distribution sums to exactly 10 000 before saving beneficiaries
- [ ] Last-beneficiary rounding absorbs remainder (no dust left in vault)
- [ ] `vault_ttl_ledgers` uses `saturating_mul` / `saturating_div`
- [ ] Vesting `per_installment` calculation handles zero `num_installments`

## 4. TTL Management

- [ ] `save_vault` always calls `extend_ttl` with the correct ledger count
- [ ] `check_in` rejects if the new deadline would exceed `max_ttl_seconds`
- [ ] `create_vault` sets TTL proportional to `check_in_interval` (2× buffer)
- [ ] Instance storage TTL is extended on every state-mutating call
- [ ] `ping_expiry` emits a warning event when TTL < `EXPIRY_WARNING_THRESHOLD`
- [ ] Archived vault state can be restored via `restore_vault` before `trigger_release`

## 5. Access Control — Vault Operations

- [ ] `deposit` / `withdraw` reject if vault is paused or released
- [ ] `withdraw` enforces `vault.balance >= amount`
- [ ] `update_beneficiary` rejects `owner == new_beneficiary`
- [ ] `set_beneficiaries` rejects owner appearing in the list
- [ ] `cancel_vault` is owner-only and only allowed while `Locked`
- [ ] `pause_vault` / `resume_vault` are owner-only

## 6. Contract-Level Pause

- [ ] `assert_not_paused` is called at the top of every state-mutating function
- [ ] Paused state blocks `deposit`, `withdraw`, `check_in`, `trigger_release`
- [ ] Admin cannot access or redirect vault funds while paused
- [ ] Unpause restores full functionality without data loss

## 7. Soroban-Specific Checks

- [ ] No `panic!` / `unwrap` in production paths — all errors use `panic_with_error!`
- [ ] `load_vault` panics with `VaultNotFound` rather than returning a default
- [ ] Persistent storage keys are unique per vault ID (no key collisions)
- [ ] `MAX_METADATA_LEN`, `MAX_CUSTOM_METADATA_LEN` are enforced before storage writes
- [ ] Host function budget (CPU / memory) is not exhausted in worst-case loops
- [ ] Ledger entry size limits are respected for `Vec<BeneficiaryEntry>` and metadata

## 8. Token Handling

- [ ] Only whitelisted token addresses are accepted in `create_vault`
- [ ] `token.transfer` return value is not silently ignored
- [ ] Contract never holds more balance than the sum of all vault balances
- [ ] XLM token address is validated at `initialize` time

## 9. Beneficiary & Vesting

- [ ] Vesting schedule `total_amount` matches vault balance at schedule creation
- [ ] `claim_vested_installment` is only callable after `trigger_release`
- [ ] Installment index cannot overflow `u32`
- [ ] Declined beneficiary blocks `trigger_release` (`InvalidBeneficiary`)
- [ ] Dispute status `Filed` blocks release until resolved

## 10. Upgrade & Versioning

- [ ] Contract version is stored and readable via `get_version`
- [ ] Any upgrade path preserves existing vault data layout
- [ ] Breaking storage key changes are documented and migration tested

---

## Audit Sign-Off

| Auditor | Date | Findings | Status |
|---------|------|----------|--------|
| (internal review) | | | Pending |
| (external auditor) | | | Not started |

> **Note**: No mainnet deployment should occur without a completed external audit. See [SECURITY.md](../SECURITY.md) for the full security policy.
