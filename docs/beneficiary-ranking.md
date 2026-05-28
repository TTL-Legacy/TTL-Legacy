# Beneficiary Ranking

Beneficiaries can be assigned a numeric priority. Lower priority values are served first.

## Functions

- `set_rank(caller, beneficiary, priority)` assigns or updates a beneficiary priority and emits `RankingSet`.
- `distribute_by_rank(total_amount)` distributes in ascending priority order and emits `DistributedByRank` for each payout.

## Events

| Event | Fields |
|---|---|
| `RankingSet` | `beneficiary`, `priority` |
| `DistributedByRank` | `beneficiary`, `priority`, `amount` |
