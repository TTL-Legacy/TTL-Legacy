/// In-memory vault cache with TTL-based expiry.
///
/// Caches the results of expensive vault state lookups (`get_vault`,
/// `get_ttl_remaining`, `get_vault_summary`) for up to `TTL_SECS` seconds.
/// Cache entries are invalidated automatically on expiry or explicitly via
/// `invalidate`.
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::models::{Vault, VaultSummary};

/// Default cache time-to-live: 5 minutes.
pub const TTL_SECS: u64 = 300;

// ── Cache entry ───────────────────────────────────────────────────────────────

struct CacheEntry<T> {
    value: T,
    inserted_at: Instant,
    ttl: Duration,
}

impl<T> CacheEntry<T> {
    fn new(value: T, ttl: Duration) -> Self {
        Self {
            value,
            inserted_at: Instant::now(),
            ttl,
        }
    }

    fn is_expired(&self) -> bool {
        self.inserted_at.elapsed() >= self.ttl
    }
}

// ── Per-vault cached data ─────────────────────────────────────────────────────

struct VaultCacheEntries {
    vault: Option<CacheEntry<Vault>>,
    ttl_remaining: Option<CacheEntry<Option<u64>>>,
    summary: Option<CacheEntry<VaultSummary>>,
}

impl VaultCacheEntries {
    fn new() -> Self {
        Self {
            vault: None,
            ttl_remaining: None,
            summary: None,
        }
    }
}

// ── Public cache type ─────────────────────────────────────────────────────────

/// Thread-safe in-memory cache keyed by `vault_id` (String).
pub struct VaultCache {
    inner: Mutex<HashMap<String, VaultCacheEntries>>,
    ttl: Duration,
}

impl VaultCache {
    /// Create a new cache with the default 5-minute TTL.
    pub fn new() -> Self {
        Self::with_ttl(Duration::from_secs(TTL_SECS))
    }

    /// Create a cache with a custom TTL (useful for tests).
    pub fn with_ttl(ttl: Duration) -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
            ttl,
        }
    }

    // ── get_vault ─────────────────────────────────────────────────────────────

    /// Return the cached `Vault` for `vault_id`, if present and not expired.
    pub fn get_vault(&self, vault_id: &str) -> Option<Vault> {
        let mut map = self.inner.lock().unwrap();
        if let Some(entries) = map.get_mut(vault_id) {
            if let Some(entry) = &entries.vault {
                if !entry.is_expired() {
                    return Some(entry.value.clone());
                }
            }
            // Expired — clear it.
            entries.vault = None;
        }
        None
    }

    /// Insert or update the cached `Vault` for `vault_id`.
    pub fn set_vault(&self, vault_id: &str, vault: Vault) {
        let mut map = self.inner.lock().unwrap();
        let entries = map
            .entry(vault_id.to_string())
            .or_insert_with(VaultCacheEntries::new);
        entries.vault = Some(CacheEntry::new(vault, self.ttl));
    }

    // ── get_ttl_remaining ─────────────────────────────────────────────────────

    /// Return the cached TTL-remaining value for `vault_id`, if present and not
    /// expired.
    pub fn get_ttl_remaining(&self, vault_id: &str) -> Option<Option<u64>> {
        let mut map = self.inner.lock().unwrap();
        if let Some(entries) = map.get_mut(vault_id) {
            if let Some(entry) = &entries.ttl_remaining {
                if !entry.is_expired() {
                    return Some(entry.value);
                }
            }
            entries.ttl_remaining = None;
        }
        None
    }

    /// Insert or update the cached TTL-remaining value for `vault_id`.
    pub fn set_ttl_remaining(&self, vault_id: &str, ttl_remaining: Option<u64>) {
        let mut map = self.inner.lock().unwrap();
        let entries = map
            .entry(vault_id.to_string())
            .or_insert_with(VaultCacheEntries::new);
        entries.ttl_remaining = Some(CacheEntry::new(ttl_remaining, self.ttl));
    }

    // ── get_vault_summary ─────────────────────────────────────────────────────

    /// Return the cached `VaultSummary` for `vault_id`, if present and not
    /// expired.
    pub fn get_vault_summary(&self, vault_id: &str) -> Option<VaultSummary> {
        let mut map = self.inner.lock().unwrap();
        if let Some(entries) = map.get_mut(vault_id) {
            if let Some(entry) = &entries.summary {
                if !entry.is_expired() {
                    return Some(entry.value.clone());
                }
            }
            entries.summary = None;
        }
        None
    }

    /// Insert or update the cached `VaultSummary` for `vault_id`.
    pub fn set_vault_summary(&self, vault_id: &str, summary: VaultSummary) {
        let mut map = self.inner.lock().unwrap();
        let entries = map
            .entry(vault_id.to_string())
            .or_insert_with(VaultCacheEntries::new);
        entries.summary = Some(CacheEntry::new(summary, self.ttl));
    }

    // ── Invalidation ──────────────────────────────────────────────────────────

    /// Remove all cached entries for `vault_id`.  Call this after a check-in
    /// or any state-change event so that subsequent reads see fresh data.
    pub fn invalidate(&self, vault_id: &str) {
        let mut map = self.inner.lock().unwrap();
        map.remove(vault_id);
    }

    /// Remove all entries from the cache.
    pub fn invalidate_all(&self) {
        let mut map = self.inner.lock().unwrap();
        map.clear();
    }

    /// Return how many vault IDs currently have at least one live (non-expired)
    /// entry in the cache.
    pub fn live_entry_count(&self) -> usize {
        let map = self.inner.lock().unwrap();
        map.values()
            .filter(|e| {
                e.vault.as_ref().map_or(false, |v| !v.is_expired())
                    || e.ttl_remaining.as_ref().map_or(false, |v| !v.is_expired())
                    || e.summary.as_ref().map_or(false, |v| !v.is_expired())
            })
            .count()
    }
}

impl Default for VaultCache {
    fn default() -> Self {
        Self::new()
    }
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Vault, VaultStatus, VaultSummary};
    use chrono::Utc;

    fn make_vault(id: &str) -> Vault {
        Vault {
            id: id.to_string(),
            owner: "owner1".to_string(),
            beneficiary: "ben1".to_string(),
            balance: 1000,
            check_in_interval: 86400,
            last_check_in: Utc::now(),
            created_at: Utc::now(),
            status: VaultStatus::Active,
            ttl_remaining: Some(86400),
        }
    }

    fn make_summary(vault_id: &str) -> VaultSummary {
        VaultSummary {
            vault_id: vault_id.to_string(),
            owner: "owner1".to_string(),
            status: VaultStatus::Active,
            ttl_remaining: Some(86400),
            balance: 1000,
        }
    }

    // ── get_vault / set_vault ─────────────────────────────────────────────────

    #[test]
    fn test_get_vault_miss_on_empty_cache() {
        let cache = VaultCache::new();
        assert!(cache.get_vault("v1").is_none());
    }

    #[test]
    fn test_set_and_get_vault() {
        let cache = VaultCache::new();
        let vault = make_vault("v1");
        cache.set_vault("v1", vault.clone());
        let result = cache.get_vault("v1");
        assert!(result.is_some());
        assert_eq!(result.unwrap().id, "v1");
    }

    #[test]
    fn test_vault_cache_expires_after_ttl() {
        let cache = VaultCache::with_ttl(Duration::from_millis(1));
        cache.set_vault("v1", make_vault("v1"));
        // Sleep just long enough for the entry to expire.
        std::thread::sleep(Duration::from_millis(5));
        assert!(cache.get_vault("v1").is_none());
    }

    #[test]
    fn test_vault_cache_updated_value_is_returned() {
        let cache = VaultCache::new();
        let mut vault = make_vault("v1");
        cache.set_vault("v1", vault.clone());
        vault.balance = 9999;
        cache.set_vault("v1", vault.clone());
        let result = cache.get_vault("v1").unwrap();
        assert_eq!(result.balance, 9999);
    }

    // ── get_ttl_remaining / set_ttl_remaining ─────────────────────────────────

    #[test]
    fn test_get_ttl_remaining_miss_on_empty_cache() {
        let cache = VaultCache::new();
        assert!(cache.get_ttl_remaining("v1").is_none());
    }

    #[test]
    fn test_set_and_get_ttl_remaining_some() {
        let cache = VaultCache::new();
        cache.set_ttl_remaining("v1", Some(3600));
        let result = cache.get_ttl_remaining("v1");
        assert_eq!(result, Some(Some(3600)));
    }

    #[test]
    fn test_set_and_get_ttl_remaining_none() {
        let cache = VaultCache::new();
        cache.set_ttl_remaining("v1", None);
        let result = cache.get_ttl_remaining("v1");
        assert_eq!(result, Some(None));
    }

    #[test]
    fn test_ttl_remaining_expires_after_ttl() {
        let cache = VaultCache::with_ttl(Duration::from_millis(1));
        cache.set_ttl_remaining("v1", Some(100));
        std::thread::sleep(Duration::from_millis(5));
        assert!(cache.get_ttl_remaining("v1").is_none());
    }

    // ── get_vault_summary / set_vault_summary ─────────────────────────────────

    #[test]
    fn test_get_vault_summary_miss_on_empty_cache() {
        let cache = VaultCache::new();
        assert!(cache.get_vault_summary("v1").is_none());
    }

    #[test]
    fn test_set_and_get_vault_summary() {
        let cache = VaultCache::new();
        let summary = make_summary("v1");
        cache.set_vault_summary("v1", summary.clone());
        let result = cache.get_vault_summary("v1");
        assert!(result.is_some());
        assert_eq!(result.unwrap().vault_id, "v1");
    }

    #[test]
    fn test_vault_summary_expires_after_ttl() {
        let cache = VaultCache::with_ttl(Duration::from_millis(1));
        cache.set_vault_summary("v1", make_summary("v1"));
        std::thread::sleep(Duration::from_millis(5));
        assert!(cache.get_vault_summary("v1").is_none());
    }

    // ── invalidation ─────────────────────────────────────────────────────────

    #[test]
    fn test_invalidate_removes_all_entries_for_vault() {
        let cache = VaultCache::new();
        cache.set_vault("v1", make_vault("v1"));
        cache.set_ttl_remaining("v1", Some(100));
        cache.set_vault_summary("v1", make_summary("v1"));

        cache.invalidate("v1");

        assert!(cache.get_vault("v1").is_none());
        assert!(cache.get_ttl_remaining("v1").is_none());
        assert!(cache.get_vault_summary("v1").is_none());
    }

    #[test]
    fn test_invalidate_does_not_affect_other_vaults() {
        let cache = VaultCache::new();
        cache.set_vault("v1", make_vault("v1"));
        cache.set_vault("v2", make_vault("v2"));

        cache.invalidate("v1");

        assert!(cache.get_vault("v1").is_none());
        assert!(cache.get_vault("v2").is_some());
    }

    #[test]
    fn test_invalidate_all_clears_entire_cache() {
        let cache = VaultCache::new();
        cache.set_vault("v1", make_vault("v1"));
        cache.set_vault("v2", make_vault("v2"));

        cache.invalidate_all();

        assert!(cache.get_vault("v1").is_none());
        assert!(cache.get_vault("v2").is_none());
    }

    // ── cache consistency ─────────────────────────────────────────────────────

    #[test]
    fn test_cache_consistency_after_state_change() {
        // Simulate a check-in event: cache is populated, state changes,
        // invalidate is called, and fresh data is written.
        let cache = VaultCache::new();
        let vault = make_vault("v1");
        cache.set_vault("v1", vault);
        cache.set_ttl_remaining("v1", Some(86400));
        cache.set_vault_summary("v1", make_summary("v1"));

        // Simulate check-in / state change → invalidate stale data.
        cache.invalidate("v1");

        // Write updated values (as the handler would after fetching fresh data).
        let mut updated_vault = make_vault("v1");
        updated_vault.ttl_remaining = Some(86400 * 2);
        cache.set_vault("v1", updated_vault.clone());
        cache.set_ttl_remaining("v1", Some(86400 * 2));

        let cached = cache.get_vault("v1").unwrap();
        assert_eq!(cached.ttl_remaining, Some(86400 * 2));

        let cached_ttl = cache.get_ttl_remaining("v1").unwrap();
        assert_eq!(cached_ttl, Some(86400 * 2));
    }

    #[test]
    fn test_independent_vaults_do_not_interfere() {
        let cache = VaultCache::new();
        cache.set_vault("v1", make_vault("v1"));
        cache.set_vault("v2", make_vault("v2"));
        cache.set_ttl_remaining("v1", Some(100));
        cache.set_ttl_remaining("v2", Some(200));

        assert_eq!(cache.get_ttl_remaining("v1"), Some(Some(100)));
        assert_eq!(cache.get_ttl_remaining("v2"), Some(Some(200)));
    }

    // ── live_entry_count ─────────────────────────────────────────────────────

    #[test]
    fn test_live_entry_count_empty() {
        let cache = VaultCache::new();
        assert_eq!(cache.live_entry_count(), 0);
    }

    #[test]
    fn test_live_entry_count_with_entries() {
        let cache = VaultCache::new();
        cache.set_vault("v1", make_vault("v1"));
        cache.set_vault("v2", make_vault("v2"));
        assert_eq!(cache.live_entry_count(), 2);
    }

    #[test]
    fn test_live_entry_count_decrements_after_invalidation() {
        let cache = VaultCache::new();
        cache.set_vault("v1", make_vault("v1"));
        cache.set_vault("v2", make_vault("v2"));
        cache.invalidate("v1");
        assert_eq!(cache.live_entry_count(), 1);
    }

    #[test]
    fn test_live_entry_count_zero_after_expiry() {
        let cache = VaultCache::with_ttl(Duration::from_millis(1));
        cache.set_vault("v1", make_vault("v1"));
        std::thread::sleep(Duration::from_millis(5));
        assert_eq!(cache.live_entry_count(), 0);
    }
}
