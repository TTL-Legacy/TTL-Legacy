# Beneficiary Caps

A beneficiary cap sets the maximum amount that beneficiary can receive. Amount above the cap is redistributed to uncapped beneficiaries with remaining room; any amount that still cannot be placed is returned as excess.

## Functions

- `set_cap(caller, beneficiary, cap)` sets a positive maximum payout and emits `CapSet`.
- `apply_caps(distributions)` enforces caps, redistributes excess where possible, returns `(capped_map, remaining_excess)`, and emits `CapEnforced` for capped beneficiaries.

## Events

| Event | Fields |
|---|---|
| `CapSet` | `beneficiary`, `cap` |
| `CapEnforced` | `beneficiary`, `requested`, `capped_at`, `excess` |
