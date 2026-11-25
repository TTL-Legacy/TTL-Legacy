# Beneficiary Waterfall and Tiering

This guide explains how to configure **beneficiary tiers** and **waterfall fallback** for multi-beneficiary plans. It covers how beneficiaries are ranked, how same-priority tiers split an allocation, how the waterfall resolves shortfalls, and the events emitted when configuration or fallback behavior changes.

For related topics, see:

- [Beneficiary Ranking](./beneficiary-ranking.md) — how beneficiaries are ordered and ranked.
- [Beneficiary Matching](./beneficiary-matching.md) — how beneficiaries are matched to allocations.

---

## Overview

A plan can have multiple beneficiaries. When the available balance is not enough to satisfy every beneficiary at their full allocation, the contract must decide **who gets paid first** and **how the remainder is distributed**. This is handled by two cooperating mechanisms:

1. **Tiering** — beneficiaries are grouped into ordered tiers. Lower-priority numbers are served first.
2. **Waterfall fallback** — when the remaining balance cannot cover a tier in full, the waterfall determines how the shortfall is distributed among the beneficiaries in that tier.

Both mechanisms are configured through the beneficiary registry and are observable through two events:

- `BeneficiaryTierSetEvent` — emitted when a beneficiary's tier/priority is configured.
- `BeneficiaryWaterfallEvent` — emitted when waterfall fallback is applied.

---

## Tier Configuration

Each beneficiary is assigned a **priority** (also referred to as its tier rank). The priority is a numeric value where **lower numbers are served first**.

### Setting a rank / priority

Use `set_rank` (or the equivalent priority setter) on the beneficiary registry to assign a beneficiary's tier:

```rust
// Beneficiary A is in the highest-priority tier.
registry.set_rank(&env, &beneficiary_a, 0);

// Beneficiary B is in the next tier down.
registry.set_rank(&env, &beneficiary_b, 1);

// Beneficiary C is served last.
registry.set_rank(&env, &beneficiary_c, 2);
```

Rules to keep in mind:

- **Lower priority value = served earlier.** Priority `0` is served before priority `1`, which is served before priority `2`, and so on.
- Multiple beneficiaries may share the **same priority**. They form a single tier and are served together (see [Same-Priority Tier Splitting](#same-priority-tier-splitting)).
- Priorities do not need to be contiguous. `0`, `5`, and `10` are valid tiers; the contract serves them in ascending order.

### Example: three-tier plan

```rust
// Tier 0: primary beneficiaries
registry.set_rank(&env, &alice, 0);
registry.set_rank(&env, &bob, 0);

// Tier 1: secondary beneficiaries
registry.set_rank(&env, &carol, 1);

// Tier 2: residual beneficiary
registry.set_rank(&env, &dave, 2);
```

When the plan is settled, the contract processes tier `0` first (Alice and Bob), then tier `1` (Carol), then tier `2` (Dave), stopping as soon as the available balance is exhausted.

---

## Same-Priority Tier Splitting

Beneficiaries that share a priority form a single tier. Within a tier, the allocation is split according to each beneficiary's **`allocation_bps`** (basis points, where `10_000` bps = 100%).

### How splitting works

1. The contract computes the total allocation for the tier as the sum of each member's `allocation_bps`.
2. The tier's share of the available balance is distributed proportionally to each member's `allocation_bps`.
3. If the tier's combined `allocation_bps` exceeds `10_000`, the tier is over-allocated and the contract will only distribute up to the available balance for that tier (see [Waterfall Fallback](#waterfall-fallback)).

### Example: splitting a tier

```rust
// Both beneficiaries share priority 0, so they form one tier.
registry.set_rank(&env, &alice, 0);
registry.set_rank(&env, &bob, 0);

// Alice receives 60% of the tier, Bob receives 40%.
registry.set_allocation_bps(&env, &alice, 6_000);
registry.set_allocation_bps(&env, &bob, 4_000);
```

If the tier is allocated `1_000` units of balance:

| Beneficiary | `allocation_bps` | Share of tier | Amount |
|-------------|------------------|---------------|--------|
| Alice       | `6_000`          | 60%           | `600`  |
| Bob         | `4_000`          | 40%           | `400`  |

### Rounding

Proportional splits may not divide evenly. The contract distributes the remainder deterministically so that the sum of distributed amounts never exceeds the tier's available balance. Any undistributed remainder stays available for lower-priority tiers.

---

## Waterfall Fallback

Waterfall fallback governs what happens when the **remaining balance is insufficient** to satisfy the current tier at its full allocation.

### Ordering

The waterfall processes tiers in ascending priority order:

1. Serve the lowest-priority-number tier first, in full if possible.
2. Deduct the amount served from the remaining balance.
3. Move to the next tier.
4. Repeat until either all tiers are satisfied or the remaining balance reaches zero.

Because lower priority numbers are served first, higher-priority tiers are protected from shortfalls caused by lower-priority tiers.

### Insufficient balance within a tier

When the remaining balance cannot cover a tier in full, the waterfall distributes the remaining balance **across the members of that tier** proportionally to their `allocation_bps`, using the same splitting rules described in [Same-Priority Tier Splitting](#same-priority-tier-splitting). Lower-priority tiers then receive nothing, since the balance has been exhausted.

### Example: shortfall scenario

```rust
// Tier 0
registry.set_rank(&env, &alice, 0);
registry.set_allocation_bps(&env, &alice, 5_000);

// Tier 1
registry.set_rank(&env, &bob, 1);
registry.set_allocation_bps(&env, &bob, 5_000);

// Tier 2
registry.set_rank(&env, &carol, 2);
registry.set_allocation_bps(&env, &carol, 5_000);
```

Suppose the available balance is `800` units:

| Tier | Beneficiary | Requested | Served | Remaining after tier |
|------|-------------|-----------|--------|----------------------|
| 0    | Alice       | `500`     | `500`  | `300`                |
| 1    | Bob         | `500`     | `300`  | `0`                  |
| 2    | Carol       | `500`     | `0`    | `0`                  |

Alice is served in full because she is in the highest-priority tier. Bob receives the remaining `300` (a partial fill). Carol receives nothing because the balance is exhausted before her tier is reached.

### Example: shortfall within a shared tier

```rust
// Tier 0, shared by two beneficiaries
registry.set_rank(&env, &alice, 0);
registry.set_rank(&env, &bob, 0);
registry.set_allocation_bps(&env, &alice, 5_000);
registry.set_allocation_bps(&env, &bob, 5_000);
```

With an available balance of `600` units, the tier is underfunded. The waterfall splits the `600` proportionally:

| Beneficiary | `allocation_bps` | Share | Served |
|-------------|------------------|-------|--------|
| Alice       | `5_000`          | 50%   | `300`  |
| Bob         | `5_000`          | 50%   | `300`  |

---

## Events

### `BeneficiaryTierSetEvent`

Emitted whenever a beneficiary's tier/priority is configured (for example, via `set_rank`). Consumers can use this event to track the current tier layout of a plan.

**Payload fields:**

| Field             | Type      | Description                                                        |
|-------------------|-----------|--------------------------------------------------------------------|
| `beneficiary`     | `Address` | The beneficiary whose tier was set.                                |
| `priority`        | `u32`     | The assigned priority/tier. Lower values are served first.         |
| `allocation_bps`  | `u32`     | The beneficiary's allocation in basis points within its tier.      |

Example payload (conceptual):

```text
BeneficiaryTierSetEvent {
    beneficiary:    GABC...XYZ,
    priority:       0,
    allocation_bps: 6_000,
}
```

### `BeneficiaryWaterfallEvent`

Emitted when waterfall fallback is applied — that is, when the remaining balance is insufficient to satisfy a tier at its full allocation and the contract must distribute a shortfall.

**Payload fields:**

| Field              | Type      | Description                                                              |
|--------------------|-----------|--------------------------------------------------------------------------|
| `beneficiary`      | `Address` | The beneficiary affected by the fallback.                                |
| `priority`         | `u32`     | The tier in which the fallback occurred.                                 |
| `requested_amount` | `i128`    | The amount the beneficiary would have received at full allocation.       |
| `served_amount`    | `i128`    | The amount actually served after the waterfall was applied.              |

Example payload (conceptual):

```text
BeneficiaryWaterfallEvent {
    beneficiary:      GABC...XYZ,
    priority:         1,
    requested_amount: 500,
    served_amount:    300,
}
```

A `BeneficiaryWaterfallEvent` is emitted for each beneficiary whose served amount is less than its requested amount. Beneficiaries served in full do not produce a waterfall event.

---

## Configuration Checklist

When setting up tiers and waterfall behavior:

1. **Assign priorities** with `set_rank`. Remember that lower numbers are served first.
2. **Group beneficiaries** into the same priority when they should share a tier.
3. **Set `allocation_bps`** for every beneficiary so the tier can be split correctly. Within a tier, `allocation_bps` values should sum to `10_000` for a fully allocated tier.
4. **Verify ordering** by reviewing the emitted `BeneficiaryTierSetEvent`s.
5. **Test shortfalls** by settling a plan with a balance smaller than the total requested amount and confirming the expected `BeneficiaryWaterfallEvent`s.

---

## See Also

- [Beneficiary Ranking](./beneficiary-ranking.md)
- [Beneficiary Matching](./beneficiary-matching.md)