# Protocol Constants

`caps.rs` and `floors.rs` define per-beneficiary distribution limits. `constants.rs` defines protocol-wide hard limits and defaults. This page lists every constant, its current value, the rationale behind it, and which contract functions enforce it.

---

## Time and Ledger Constants (`constants.rs`)

| Constant | Value | Description |
| :--- | :--- | :--- |
| `LEDGER_SECOND` | `5` s | Approximate Stellar mainnet ledger close time. Used to convert seconds to ledgers for storage TTL calculations. |
| `SECONDS_PER_MINUTE` | `60` | Convenience multiplier. |
| `SECONDS_PER_HOUR` | `3 600` | Convenience multiplier. |
| `SECONDS_PER_DAY` | `86 400` | Convenience multiplier. |
| `DAYS_PER_YEAR` | `365` | Used in the simplified financial-year calculation for `DEFAULT_MAX_TTL_SECONDS`. |

---

## Vault TTL Constants

| Constant | Value | Description |
| :--- | :--- | :--- |
| `DEFAULT_MAX_TTL_SECONDS` | `315 360 000` s (10 years) | Upper bound on vault TTL when no admin override is set. Prevents an astronomically large interval from making automatic release practically unreachable. Enforced by `set_max_ttl_seconds` / `get_max_ttl_seconds`. |
| `MAX_PERSISTENT_TTL` | `3 110 400` ledgers (~180 days) | Soroban's hard ceiling on a persistent storage entry's TTL. Vault storage extension is capped at this value. Applied in `vault_ttl_ledgers()`. |
| `VAULT_TTL_LEDGERS` | `200 000` ledgers (~11.6 days) | Default persistent storage TTL floor for vault entries. Used unless a vault's check-in interval calls for a larger value. Applied in every `extend_ttl` call on vault state. |
| `VAULT_TTL_THRESHOLD` | `1 000` ledgers (~83 min) | Minimum remaining TTL before a persistent entry is eligible for extension. Guards against redundant extension calls. Used throughout storage write paths. |
| `INSTANCE_TTL_LEDGERS` | `200 000` ledgers (~11.6 days) | TTL assigned to instance storage entries. Extended on every state-mutating call to keep the contract instance alive. |
| `INSTANCE_TTL_THRESHOLD` | `1 000` ledgers (~83 min) | Minimum remaining TTL before instance storage is eligible for extension. |

---

## Check-In Interval Constants

| Constant | Value | Description |
| :--- | :--- | :--- |
| `MIN_CHECK_IN_INTERVAL` | `3 600` s (1 hour) | Absolute minimum check-in interval. Prevents abuse of the TTL extension mechanism with unreasonably short intervals. Enforced in `create_vault` and `update_check_in_interval`. |
| `MAX_CHECK_IN_INTERVAL` | `315 360 000` s (10 years) | Unconditional ceiling — independent of the admin-configured `MaxCheckInInterval`. Prevents overflow in TTL arithmetic and ensures automatic release remains reachable. Enforced in `create_vault` and `update_check_in_interval`. |
| `DEFAULT_MIN_CHECKIN_COOLDOWN` | `60` s | Minimum time between consecutive check-ins from the same vault (rate limit). Applied in `check_in` and `check_in_as_delegate` before updating `last_check_in`. Overridable at the instance level via `set_min_check_in_interval`. |
| `MAX_ACCELERATE_SECONDS` | `2 592 000` s (30 days) | Maximum number of seconds a single `accelerate_ttl` call may advance the check-in deadline. Prevents a single transaction from skipping months of expected check-ins. |

---

## Hibernation Constants

| Constant | Value | Description |
| :--- | :--- | :--- |
| `MIN_HIBERNATION_SECONDS` | `3 600` s (1 hour) | Minimum duration for a hibernation period. Mirrors `MIN_CHECK_IN_INTERVAL`; a hibernation shorter than the shortest allowed check-in interval provides no meaningful relief. Enforced in `enter_hibernation`. |
| `MAX_HIBERNATION_SECONDS` | `15 552 000` s (180 days) | Maximum hibernation duration. Capped at Soroban's persistent-entry TTL ceiling so storage extension can never fail during hibernation. Enforced in `enter_hibernation`. |

---

## Storage Sizing Constants

| Constant | Value | Description |
| :--- | :--- | :--- |
| `DEPOSIT_HISTORY_CAP` | `100` entries | Maximum number of deposit history entries kept per vault. Oldest entries are dropped first to bound on-chain storage. Applied in `deposit`. |
| `MAX_SNAPSHOTS_PER_VAULT` | `1 000` | Maximum number of vault state snapshots. Enforced in `create_vault_snapshot`. |
| `MAX_WITHDRAWAL_HISTORY` | `50` entries | Per-vault withdrawal event ring buffer size. Older entries are silently evicted. Applied in `withdraw`. |
| `MAX_BATCH_STATUS_SIZE` | `100` | Maximum vault IDs accepted by `get_vault_batch_status` per call. Guards against excessive computation in a single invocation. |
| `MAX_VAULT_BATCH_SIZE` | `50` | Maximum vault IDs accepted by `get_vault_batch` per call. |
| `MAX_VESTING_SCHEDULES` | `20` | Maximum number of vesting schedules attached to a single vault. Enforced in `add_vesting_schedule`. |
| `MAX_BENEFICIARIES` | `20` | Maximum number of beneficiaries per vault. Enforced in `set_beneficiaries` and `create_vault_with_splits`. |

---

## Metadata Length Constants

| Constant | Value | Description |
| :--- | :--- | :--- |
| `MAX_METADATA_LEN` | `256` bytes | Maximum length for the vault metadata string. Enforced in `update_metadata`. |
| `MAX_NAME_LEN` | `64` bytes | Maximum vault name length. |
| `MAX_DESCRIPTION_LEN` | `512` bytes | Maximum vault description length. |
| `MAX_NOTES_LEN` | `1 024` bytes | Maximum vault notes length. Enforced in `set_vault_notes`. |
| `MAX_CUSTOM_METADATA_LEN` | `2 048` bytes | Maximum custom metadata payload (Issue #378). Enforced in `set_custom_metadata`. |
| `MAX_RELEASE_MEMO_LEN` | `256` bytes | Maximum release memo length (Issue #791). Enforced in `set_release_memo`. |

---

## Governance / Timelock Constants

| Constant | Value | Description |
| :--- | :--- | :--- |
| `OWNERSHIP_TRANSFER_TIMELOCK` | `86 400` s (24 hours) | Minimum wait before a pending ownership transfer can be accepted (Issue #1119). Enforced in `accept_ownership`. |
| `OWNERSHIP_TRANSFER_EXPIRY` | `604 800` s (7 days) | Pending ownership transfer expires if not accepted within this window. Enforced in `accept_ownership`. |
| `UPGRADE_TIMELOCK` | `259 200` s (72 hours) | Minimum wait before a staged contract upgrade can be executed (Issue #1120). Enforced in `execute_upgrade`. |
| `BASIS_POINTS_100_PERCENT` | `10 000` | Denominator for all basis-point calculations. Used throughout beneficiary split and fee calculations. |
| `MULTISIG_PROPOSAL_EXPIRY` | `604 800` s (7 days) | Multi-sig proposals expire after this window. Enforced in `approve_multisig_proposal`. |

---

## Conflict Resolution Constants

| Constant | Value | Description |
| :--- | :--- | :--- |
| `DEFAULT_CONFLICT_DISPUTE_WINDOW` | `259 200` s (72 hours) | Default window before a beneficiary conflict can be auto-resolved. Enforced in `auto_resolve_conflict`. |
| `MIN_CONFLICT_DISPUTE_WINDOW` | `3 600` s (1 hour) | Minimum configurable dispute window. Enforced in `set_conflict_dispute_window`. |
| `MAX_CONFLICT_DISPUTE_WINDOW` | `2 592 000` s (30 days) | Maximum configurable dispute window. Enforced in `set_conflict_dispute_window`. |

---

## Oracle Constants

| Constant | Value | Description |
| :--- | :--- | :--- |
| `ORACLE_FRESHNESS_WINDOW` | `300` s (5 minutes) | Maximum age of an oracle attestation timestamp (Issue #1832). The `attest_milestone` function rejects attestations where `now - oracle_timestamp > ORACLE_FRESHNESS_WINDOW` with `OracleAttestationStale` (error 159). |

---

## Caps and Floors (`caps.rs` / `floors.rs`)

Caps and floors are per-beneficiary distribution limits applied during `trigger_release`. They are stored in persistent maps rather than as compile-time constants.

### Cap Behaviour (`caps.rs`)

A cap sets an upper bound on how much a single beneficiary can receive. If the computed distribution for a beneficiary exceeds their cap, the excess is redistributed to other beneficiaries proportionally (weighted by their uncapped allocation). If no beneficiary has remaining room, the excess is returned to the caller as an `i128` residual.

| Function | Description |
| :--- | :--- |
| `set_cap(env, caller, beneficiary, cap)` | Sets the per-beneficiary cap. `caller` must authorise. `cap` must be > 0; zero is rejected with `InvalidAmount`. Emits `cap_set`. |
| `apply_caps(env, distributions)` | Applies all registered caps to a distribution map. Returns `(capped_map, residual_excess)`. Emits `cap_enf` for every beneficiary whose allocation was reduced. |

**Storage key:** `BeneficiaryCaps` (persistent `Map<Address, i128>`).

### Floor Behaviour (`floors.rs`)

A floor sets a minimum guaranteed allocation. If a beneficiary's computed share falls below their floor, the contract raises it to the floor value provided there is enough balance in `total_available`. If the estate is insufficient to reach the floor, the gap is tracked as a `shortfall`.

| Function | Description |
| :--- | :--- |
| `set_floor(env, caller, beneficiary, floor)` | Sets the per-beneficiary floor. `caller` must authorise. `floor` must be > 0; zero is rejected with `InvalidAmount`. Emits `floor_set`. |
| `apply_floors(env, distributions, total_available)` | Applies all registered floors. Returns `(raised_map, total_shortfall)`. Emits `floor_enf` when a floor is triggered. |

**Storage key:** `BeneficiaryFloors` (persistent `Map<Address, i128>`).

### Interaction Between Caps and Floors

Caps and floors are applied sequentially inside `trigger_release_internal`:

1. `apply_floors` raises any under-threshold allocations first.
2. `apply_caps` then clamps over-threshold allocations and redistributes excess.

A floor value higher than a cap for the same beneficiary is not explicitly prevented at the data-model level; the result in that scenario is a shortfall equal to the gap.

### Events

| Topic | Type | Emitted By | Payload |
| :--- | :--- | :--- | :--- |
| `cap_set` | `CapSetEvent` | `set_cap` | `{ beneficiary, cap }` |
| `cap_enf` | `CapEnforcedEvent` | `apply_caps` | `{ beneficiary, requested, capped_at, excess }` |
| `floor_set` | `FloorSetEvent` | `set_floor` | `{ beneficiary, floor }` |
| `floor_enf` | `FloorEnforcedEvent` | `apply_floors` | `{ beneficiary, original, floor, shortfall }` |
