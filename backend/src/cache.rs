/// In-memory vault cache with TTL-based expiry.
///
/// Caches the results of expensive vault state lookups (`get_vault`,
/// `get_ttl_remaining`, `get_vault_summary`) for up to `TTL_SECS` seconds.
/// Cache entries are invalidated automatically on expiry or explicitly via
/// `invalidate`.
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
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

// ── Cache metrics ─────────────────────────────────────────────────────────────

/// Snapshot of cache hit/miss counters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CacheMetrics {
    pub hits: u64,
    pub misses: u64,
    pub invalidations: u64,
}

impl CacheMetrics {
    /// Total number of lookups recorded (hits + misses).
    pub fn lookups(&self) -> u64 {
        self.hits + self.misses
    }

    /// Hit ratio in the range `0.0..=1.0`; `0.0` when no lookups occurred.
    pub fn hit_ratio(&self) -> f64 {
        let lookups = self.lookups();
        if lookups == 0 {
            0.0
        } else {
            self.hits as f64 / lookups as f64
        }
    }
}

// ── Public cache type ─────────────────────────────────────────────────────────

/// Thread-safe in-memory cache keyed by `vault_id` (String).
pub struct VaultCache {
    inner: Mutex<HashMap<String, VaultCacheEntries>>,
    ttl: Duration,
    hits: AtomicU64,
    misses: AtomicU64,
    invalidations: AtomicU64,
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
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
            invalidations: AtomicU64::new(0),
        }
    }

    // ── Metrics ───────────────────────────────────────────────────────────────

    fn record_hit(&self) {
        self.hits.fetch_add(1, Ordering::Relaxed);
    }

    fn record_miss(&self) {
        self.misses.fetch_add(1, Ordering::Relaxed);
    }

    /// Return a snapshot of the current hit/miss/invalidation counters.
    pub fn metrics(&self) -> CacheMetrics {
        CacheMetrics {
            hits: self.hits.load(Ordering::Relaxed),
            misses: self.misses.load(Ordering::Relaxed),
            invalidations: self.invalidations.load(Ordering::Relaxed),
        }
    }

    /// Reset all counters to zero.
    pub fn reset_metrics(&self) {
        self.hits.store(0, Ordering::Relaxed);
        self.misses.store(0, Ordering::Relaxed);
        self.invalidations.store(0, Ordering::Relaxed);
    }

    // ── get_vault ─────────────────────────────────────────────────────────────

    /// Return the cached `Vault` for `vault_id`, if present and not expired.
    pub fn get_vault(&self, vault_id: &str) -> Option<Vault> {
        let mut map = self.inner.lock().unwrap();
        if let Some(entries) = map.get_mut(vault_id) {
            if let Some(entry) = &entries.vault {
                if !entry.is_expired() {
                    let value = entry.value.clone();
                    drop(map);
                    self.record_hit();
                    return Some(value);
                }
            }
            // Expired — clear it.
            entries.vault = None;
        }
        drop(map);
        self.record_miss();
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
                    let value = entry.value;
                    drop(map);
                    self.record_hit();
                    return Some(value);
                }
            }
            entries.ttl_remaining = None;
        }
        drop(map);
        self.record_miss();
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
                    let value = entry.value.clone();
                    drop(map);
                    self.record_hit();
                    return Some(value);
                }
            }
            entries.summary = None;
        }
        drop(map);
        self.record_miss();
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
        drop(map);
        self.invalidations.fetch_add(1, Ordering::Relaxed);
    }

    /// Invalidate cache entries in response to an indexed contract event.
    ///
    /// Check-ins, withdrawals and releases mutate vault state, so any cached data
    /// for the affected vault must be dropped to avoid serving stale values.
    /// Unknown event kinds are ignored.  TTL expiry remains in place as a fallback.
    pub fn invalidate_on_event(&self, event_kind: &str, vault_id: &str) {
        match event_kind {
            "check_in" | "withdrawal" | "release" => self.invalidate(vault_id),
            _ => {}
        }
    }

    /// Remove all entries from the cache.
    pub fn invalidate_all(&self) {
        let mut map = self.inner.lock().unwrap();
        map.clear();
        drop(map);
        self.invalidations.fetch_add(1, Ordering::Relaxed);
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
        vault.balance = 2000;
        cache.set_vault("v1", vault);
        assert_eq!(cache.get_vault("v1").unwrap().balance, 2000);
    }

    // ── Invalidation ──────────────────────────────────────────────────────────

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
    fn test_invalidate_on_check_in_event() {
        let cache = VaultCache::new();
        cache.set_vault("v1", make_vault("v1"));
        cache.set_vault_summary("v1", make_summary("v1"));
        assert!(cache.get_vault("v1").is_some());

        // A check-in event for v1 must drop the stale entries.
        cache.invalidate_on_event("check_in", "v1");

        assert!(cache.get_vault("v1").is_none());
        assert!(cache.get_vault_summary("v1").is_none());
    }

    #[test]
    fn test_invalidate_on_withdrawal_event() {
        let cache = VaultCache::new();
        cache.set_vault("v1", make_vault("v1"));
        cache.invalidate_on_event("withdrawal", "v1");
        assert!(cache.get_vault("v1").is_none());
    }

    #[test]
    fn test_invalidate_on_event_ignores_unrelated_kinds() {
        let cache = VaultCache::new();
        cache.set_vault("v1", make_vault("v1"));
        cache.invalidate_on_event("deposit", "v1");
        assert!(cache.get_vault("v1").is_some());
    }

    #[test]
    fn test_invalidate_on_event_only_affects_target_vault() {
        let cache = VaultCache::new();
        cache.set_vault("v1", make_vault("v1"));
        cache.set_vault("v2", make_vault("v2"));
        cache.invalidate_on_event("check_in", "v1");
        assert!(cache.get_vault("v1").is_none());
        assert!(cache.get_vault("v2").is_some());
    }

    // ── Metrics ───────────────────────────────────────────────────────────────

    #[test]
    fn test_metrics_track_hits_and_misses() {
        let cache = VaultCache::new();
        cache.set_vault("v1", make_vault("v1"));
        assert!(cache.get_vault("v1").is_some()); // hit
        assert!(cache.get_vault("missing").is_none()); // miss
        let metrics = cache.metrics();
        assert_eq!(metrics.hits, 1);
        assert_eq!(metrics.misses, 1);
        assert_eq!(metrics.lookups(), 2);
        assert!((metrics.hit_ratio() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_metrics_count_invalidations() {
        let cache = VaultCache::new();
        cache.set_vault("v1", make_vault("v1"));
        cache.invalidate_on_event("check_in", "v1");
        assert_eq!(cache.metrics().invalidations, 1);
    }

    #[test]
    fn test_reset_metrics() {
        let cache = VaultCache::new();
        cache.set_vault("v1", make_vault("v1"));
        let _ = cache.get_vault("v1");
        cache.reset_metrics();
        let metrics = cache.metrics();
        assert_eq!(metrics.hits, 0);
        assert_eq!(metrics.misses, 0);
        assert_eq!(metrics.invalidations, 0);
    }
}
