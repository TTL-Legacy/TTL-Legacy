# Optimize get_check_in_history_page to Avoid Full Deserialization

## Summary
Changed check-in history storage from a single `Vec<CheckInHistoryEntry>` blob under `DataKey::CheckInHistory(vault_id)` to individual entries stored under `DataKey::CheckInEntry(vault_id, index)` with a ring buffer approach for O(1) writes.

## Changes

### Storage Layout
- **Added** `DataKey::CheckInEntry(u64, u32)` — stores a single `CheckInHistoryEntry`
- **Added** `DataKey::CheckInHistoryHead(u64)` — ring buffer head pointer (only meaningful at 50 entries)
- **Added** `DataKey::CheckInHistoryLen(u64)` — total number of entries stored (capped at 50)
- Old `DataKey::CheckInHistory(u64)` key is deprecated but remains in the enum to avoid orphaned data

### `record_check_in_history` (write path)
- No longer deserializes the full Vec
- Uses a ring buffer: entries are stored at indices 0–49, wrapping around when full
- O(1) write cost (single entry write + two metadata writes)

### `get_check_in_history` (read path, backward compatible)
- Reimplemented to read individual entries in chronological order
- Still returns the full history when needed

### `get_check_in_history_page` (new, paginated)
- Only deserializes the entries for the requested page
- Parameters: `vault_id`, `page`, `page_size`
- Returns empty vec if page is out of bounds
- Entries returned in chronological order (oldest first)

### `predict_expiry` (optimized)
- No longer loads all 50 entries; only reads the first and last entry in the sample window (up to 10 entries)

## Performance
- **Write**: O(1) entry storage instead of O(50) serialization of full Vec
- **Read (paginated)**: Only deserializes `page_size` entries instead of all 50
- **predict_expiry**: Only reads 2 entries instead of all 50

## Testing
- Added 5 new tests covering:
  - First page returns correct number of entries
  - Second page returns remaining entries
  - Out-of-bounds page returns empty
  - Full 50-entry ring buffer pagination
  - Entries are in chronological order

Closes #873
