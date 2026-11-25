# Findings: "transfer_from missing allowance check" — issue does not match codebase

**Repository:** https://github.com/Priest-Codes/TTL-Legacy
**Commit inspected:** `f2ccf10` (`main`, full history fetched — 1,567 commits)
**Issue labels:** `bug`, `token-management`
**Outcome:** No code changed. The issue should be re-scoped before work begins.

---

## 1. Issue as written

> `transfer_from` deducts from the token balance but does not verify that
> `caller_allowance(spender, from) >= amount` before proceeding. Add the
> allowance check and return `InsufficientAllowance` if it fails.

The issue assumes three things exist:

| Assumed symbol          | Exists? |
|-------------------------|---------|
| `transfer_from` function | ❌ No |
| `caller_allowance(spender, from)` | ❌ No |
| `InsufficientAllowance` error variant | ❌ No |

## 2. Evidence

| Check | Result |
|-------|--------|
| `grep -rniI "transfer_from\|allowance"` over the whole repo (contracts, backend, frontend, scripts, docs) | Only hit: `docs/security.md:43` (threat-model table text, "Vault balances and token allowances"). No code. |
| `grep` for `spender`, `fn approve`, `fn balance`, share/receipt tokens | No allowance-style ledger anywhere. `approve_*` functions are for multisig, withdrawals, passkey recovery, and votes. |
| `git log --all -S "transfer_from"` | 0 commits — symbol never existed |
| `git log --all -S "InsufficientAllowance"` | 0 commits — symbol never existed |
| GitHub issue search `transfer_from` / `allowance` | 0 results |

So this is **not a regression**, and there is no existing function missing a check.

## 3. How tokens actually move in this codebase

Workspace crates: `contracts/ttl_vault`, `contracts/zk_verifier`, `backend` (plus a separate `contracts/vault` crate with no token logic).

`TtlVaultContract` (`contracts/ttl_vault/src/lib.rs`, ~21k lines) does **not** issue its own token. It holds SEP-41 tokens (XLM/whitelisted tokens like USDC) and moves them only through the standard Soroban client:

```rust
// Deposit (lib.rs ~L2204) — depositor authorises directly; no allowance used
let token_client = token::Client::new(&env, &vault.token_address);
token_client.transfer(&from, &env.current_contract_address(), &amount);

// Withdraw / release / claim (e.g. lib.rs ~L2508, ~L12835)
token_client.transfer(&env.current_contract_address(), &vault.owner, &amount);
```

- Every inbound transfer is `transfer(from, contract, amount)` with `from.require_auth()` enforced by the token contract. There is no `approve` → `transfer_from` pull pattern, so no allowance gets consumed.
- Allowances on the underlying SEP-41 token are enforced by that **token contract** itself (Stellar Asset Contract), not by this repo.
- The nearest "spend on someone's behalf" feature is **beneficiary claim delegation** (`delegate_beneficiary_claim` / `claim_as_delegated_proxy`, ~L12771–L12848). It's time-bounded (`expiry`), not amount-bounded, and already guarded by `proxy.require_auth()`, an expiry check (`DelegationExpired`), a release-status check, and `EmptyVault`.

`ContractError` currently ends at `DepositTooSmall = 163`. Variant `8` is `InsufficientBalance`, and there is no allowance variant.

## 4. Why no code was changed

Doing what the issue asks would mean **designing a new feature** (approve API, allowance storage, expiration semantics, events, a new contract function). That's more than a bug fix. Doing it without maintainer input risks adding API surface the project doesn't want on a security-sensitive contract.

## 5. Suggested ways to re-scope the issue (for maintainers)

1. **Close as invalid / not applicable.** This is the right call if no vault-level spender feature is planned. SEP-41 allowance enforcement already lives in the token contract.
2. **Re-file as a feature request:** "Add SEP-41-style vault allowances". Suggested scope:
   - `approve(env, from, spender, vault_id, amount, expiration_ledger)` — `from.require_auth()`, stored under a new `StorageKey::VaultAllowance(vault_id, from, spender)`
   - `caller_allowance(env, spender, from, vault_id) -> i128` — returns 0 after expiry
   - `transfer_from(env, spender, vault_id, from, to, amount)` — `spender.require_auth()`; check `amount > 0` (`InvalidAmount`), then `caller_allowance(...) >= amount` (`InsufficientAllowance`), then `vault.balance >= amount` (`InsufficientBalance`). Only after all checks pass: decrement the allowance, decrement the balance, transfer, emit an event
   - New error: `InsufficientAllowance = 164`
   - Tests: exact allowance succeeds; allowance short by 1 returns `InsufficientAllowance` with balance unchanged; expired allowance; zero/negative amount; allowance decremented after a partial spend
3. **If the issue meant a different repo/branch** (e.g. a token contract outside this repo), link the correct file and line in the issue.

## 6. Build / test status (actually run on unmodified `main` @ `f2ccf10`)

Toolchain: rustc/cargo 1.100.0 stable, single job, 4 GB swap.

| Command | Result |
|---------|--------|
| `cargo build -p ttl-vault` | ❌ **Fails — 3 pre-existing errors** (unrelated to this issue) |
| `cargo test -p ttl-vault` | ❌ Can't run — the library doesn't compile |
| `cargo test -p zk-verifier` | ❌ Pre-existing test compile errors (`Address::generate` used without the `testutils` trait import; type mismatches) |
| `cargo build --locked -p zk-verifier` | ❌ Committed `Cargo.lock` is stale; cargo needs to add ~45 lines |

Pre-existing `ttl-vault` compile errors:

1. `lib.rs:20830` — `set_beneficiary_confirmation_deadline`: *contract function name is too long: 37, max is 32* (Soroban limit).
2. `lib.rs:8854` — `StorageKey::CheckinEmailToken` not found in `types::StorageKey` (E0599).
3. `lib.rs:8879` — same missing `StorageKey::CheckinEmailToken` variant (E0599).

These would block **any** fix, including a future allowance feature, from passing CI. They should be fixed or tracked as separate issues.

## 7. Files modified / created

| File | Change |
|------|--------|
| *(repository)* | **None** — `git status` clean at `f2ccf10` |
| `ISSUE_FINDINGS_transfer_from_allowance.md` (workspace root, outside repo) | Created — this report |
