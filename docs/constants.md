# Constants in `caps.rs` and `floors.rs`

This page lists what `contracts/ttl_vault/src/caps.rs` and `contracts/ttl_vault/src/floors.rs` define, read from the code. For what caps and floors mean to a beneficiary, see [beneficiary-caps.md](beneficiary-caps.md) and [beneficiary-floors.md](beneficiary-floors.md).

## What these files do not contain

Neither file defines a fixed numeric limit. A cap or a floor is a per-beneficiary amount that the caller of `set_cap` or `set_floor` chooses at run time. The protocol-wide numeric limits (maximum beneficiaries, maximum TTL, field lengths, dispute windows and so on) are in `contracts/ttl_vault/src/constants.rs`, which this page does not cover.

## Constants

The only `const` items in the two files are event topics:

| Constant | File | Value | Used by |
|---|---|---|---|
| `CAP_SET_TOPIC` | `caps.rs` | `symbol_short!("cap_set")` | `set_cap` publishes a `CapSetEvent` under this topic |
| `CAP_ENFORCED_TOPIC` | `caps.rs` | `symbol_short!("cap_enf")` | `apply_caps` publishes a `CapEnforcedEvent` under this topic, once for each beneficiary whose amount was reduced |
| `FLOOR_SET_TOPIC` | `floors.rs` | `symbol_short!("floor_set")` | `set_floor` publishes a `FloorSetEvent` under this topic |
| `FLOOR_ENFORCED_TOPIC` | `floors.rs` | `symbol_short!("floor_enf")` | `apply_floors` publishes a `FloorEnforcedEvent` under this topic when a payout was below its floor or could not reach it |

The code gives no reason for these values. They are short topic names, within the 9-character limit of `symbol_short!`.

## Storage keys

| Key | File | Holds |
|---|---|---|
| `CapsKey::BeneficiaryCaps` | `caps.rs` | one persistent `Map<Address, i128>` of caps, for all beneficiaries |
| `FloorsKey::BeneficiaryFloors` | `floors.rs` | one persistent `Map<Address, i128>` of floors, for all beneficiaries |

## Values the code implies

| Value | Where | Effect |
|---|---|---|
| A cap must be greater than 0 | `set_cap` | `cap <= 0` panics with `ContractError::InvalidAmount` |
| A floor must be greater than 0 | `set_floor` | `floor <= 0` panics with `ContractError::InvalidAmount` |
| No cap set for a beneficiary | `apply_caps`, `redistribute_excess` | treated as `i128::MAX`, so the amount is never reduced |
| No floor set for a beneficiary | `apply_floors` | treated as `0`, so the amount is never raised |
| A negative `total_available` | `apply_floors` | read as `0` (`total_available.max(0)`) |

Both setters call `caller.require_auth()`. Neither file checks that the caller is an admin or an owner; any check of that kind would have to be made by whoever calls them.

## Which contract functions enforce them

None. `lib.rs` has no `mod caps;` or `mod floors;`, and no other file calls `set_cap`, `apply_caps`, `set_floor` or `apply_floors`, so these two files are not compiled into the contract today. Their unit tests do not run either. One of them expects the panic message `cap must be positive`, while `set_cap` panics with `ContractError::InvalidAmount`; the same goes for `floor must be positive` in `floors.rs`.

The two other documents describe `set_cap`, `apply_caps`, `set_floor` and `apply_floors` as functions of the contract. If they are meant to be part of it, the modules still have to be added and called from the release path. Once that is done, this page should name the functions that call them.
