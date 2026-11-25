# Beneficiary Matching

When a source beneficiary receives a payout, a matched beneficiary can receive a configured percentage of that source payout.

## Functions

- `set_match(caller, source, matched, match_bps)` registers a match pair from 0 to 10000 bps and emits `MatchSet`.
- `compute_matched_amounts(base_distributions)` returns only the extra matched payouts and emits `MatchedDistribution` for each computed match.

## Events

| Event | Fields |
|---|---|
| `MatchSet` | `source`, `matched`, `match_bps` |
| `MatchedDistribution` | `source`, `matched`, `source_amount`, `matched_amount` |
