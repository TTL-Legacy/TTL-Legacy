# Beneficiary Floors

A beneficiary floor sets the minimum amount a beneficiary should receive. If the estate cannot cover configured floors, distribution completes with a reported shortfall instead of panicking.

## Functions

- `set_floor(caller, beneficiary, floor)` sets a positive minimum payout and emits `FloorSet`.
- `apply_floors(distributions, total_available)` raises payouts below configured floors while funds remain, returns `(floored_map, total_shortfall)`, and emits `FloorEnforced`.

## Events

| Event | Fields |
|---|---|
| `FloorSet` | `beneficiary`, `floor` |
| `FloorEnforced` | `beneficiary`, `original`, `floor`, `shortfall` |
