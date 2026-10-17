use super::*;
use crate::cache::VaultCache;
use crate::models::VaultSummary;

pub fn search_vaults(store: &VaultStore, query: &SearchQuery) -> SearchResult {
    let vaults = store.lock().unwrap();
    let limit = query.limit.unwrap_or(20).clamp(1, 100);

    let mut filtered: Vec<Vault> = vaults
        .values()
        .filter(|v| {
            if let Some(ref owner) = query.owner {
                if v.owner != *owner {
                    return false;
                }
            }
            if let Some(ref beneficiary) = query.beneficiary {
                if v.beneficiary != *beneficiary {
                    return false;
                }
            }
            if let Some(ref status) = query.status {
                if v.status != *status {
                    return false;
                }
            }
            if let Some(after) = query.created_after {
                if v.created_at < after {
                    return false;
                }
            }
            if let Some(before) = query.created_before {
                if v.created_at > before {
                    return false;
                }
            }
            true
        })
        .cloned()
        .collect();

    // Stable ordering by vault ID so cursors are deterministic.
    filtered.sort_by(|a, b| a.id.cmp(&b.id));

    // Apply cursor: skip everything up to and including the cursor vault ID.
    if let Some(ref cursor) = query.cursor {
        let pos = filtered.iter().position(|v| &v.id == cursor);
        if let Some(idx) = pos {
            filtered = filtered.into_iter().skip(idx + 1).collect();
        }
    }

    let mut paginated: Vec<Vault> = filtered.into_iter().take(limit as usize + 1).collect();
    let next_cursor = if paginated.len() > limit as usize {
        paginated.pop().map(|v| v.id.clone())
    } else {
        None
    };

    SearchResult {
        vaults: paginated,
        next_cursor,
        limit,
    }
}

pub fn get_vault_history(event_store: &EventStore, vault_id: &str) -> Vec<VaultEvent> {
    event_store
        .lock()
        .unwrap()
        .iter()
        .filter(|e| e.vault_id == vault_id)
        .cloned()
        .collect()
}

pub fn get_vault_audit_log(audit_store: &AuditStore, vault_id: &str) -> Vec<AuditEntry> {
    audit_store
        .lock()
        .unwrap()
        .iter()
        .filter(|a| {
            a.details
                .get("vault_id")
                .map_or(false, |v| v.as_str() == Some(vault_id))
        })
        .cloned()
        .collect()
}

// ── Task 1: Analytics ────────────────────────────────────────────────────────

pub fn compute_vault_analytics(store: &VaultStore) -> crate::models::VaultAnalytics {
    use crate::models::{TimeSeriesPoint, VaultAnalytics, VaultStatus};
    use std::collections::BTreeMap;

    let vaults = store.lock().unwrap();
    let total_vaults = vaults.len() as u64;
    let active_vaults = vaults
        .values()
        .filter(|v| v.status == VaultStatus::Active)
        .count() as u64;
    let released_vaults = vaults
        .values()
        .filter(|v| v.status == VaultStatus::Released)
        .count() as u64;

    let avg_ttl = if total_vaults > 0 {
        vaults
            .values()
            .map(|v| v.check_in_interval as f64)
            .sum::<f64>()
            / total_vaults as f64
    } else {
        0.0
    };

    let release_rate = if total_vaults > 0 {
        released_vaults as f64 / total_vaults as f64
    } else {
        0.0
    };

    // Build daily time-series bucketed by creation date
    let mut created_by_day: BTreeMap<String, u64> = BTreeMap::new();
    let mut released_by_day: BTreeMap<String, u64> = BTreeMap::new();
    for v in vaults.values() {
        let day = v.created_at.format("%Y-%m-%d").to_string();
        *created_by_day.entry(day.clone()).or_insert(0) += 1;
        if v.status == VaultStatus::Released {
            *released_by_day.entry(day).or_insert(0) += 1;
        }
    }

    let all_days: std::collections::BTreeSet<String> = created_by_day
        .keys()
        .chain(released_by_day.keys())
        .cloned()
        .collect();

    let time_series = all_days
        .into_iter()
        .map(|date| TimeSeriesPoint {
            vaults_created: *created_by_day.get(&date).unwrap_or(&0),
            vaults_released: *released_by_day.get(&date).unwrap_or(&0),
            date,
        })
        .collect();

    VaultAnalytics {
        total_vaults,
        active_vaults,
        average_ttl_seconds: avg_ttl,
        release_rate,
        time_series,
    }
}

// ── Task 2: Backup & Recovery ─────────────────────────────────────────────────

pub fn store_backup(backup_store: &BackupStore, backup: crate::models::VaultBackup) {
    backup_store
        .lock()
        .unwrap()
        .insert(backup.backup_id.clone(), backup);
}

pub fn get_backup(
    backup_store: &BackupStore,
    backup_id: &str,
) -> Option<crate::models::VaultBackup> {
    backup_store.lock().unwrap().get(backup_id).cloned()
}

// ── Task 3: Sharing ───────────────────────────────────────────────────────────

pub fn add_vault_share(share_store: &ShareStore, share: crate::models::VaultShare) {
    share_store.lock().unwrap().push(share);
}

pub fn get_vault_shares(
    share_store: &ShareStore,
    vault_id: &str,
) -> Vec<crate::models::VaultShare> {
    share_store
        .lock()
        .unwrap()
        .iter()
        .filter(|s| s.vault_id == vault_id)
        .cloned()
        .collect()
}

// ── Share token persistence ──────────────────────────────────────────────────

pub fn add_share_token(store: &ShareTokenStore, token: ShareToken) {
    store.lock().unwrap().insert(token.token.clone(), token);
}

pub fn get_share_token(store: &ShareTokenStore, token: &str) -> Option<ShareToken> {
    store.lock().unwrap().get(token).cloned()
}

pub fn get_vault_share_tokens(store: &ShareTokenStore, vault_id: &str) -> Vec<ShareToken> {
    store
        .lock()
        .unwrap()
        .values()
        .filter(|t| t.vault_id == vault_id)
        .cloned()
        .collect()
}

pub fn revoke_share_token(store: &ShareTokenStore, token: &str) -> Option<ShareToken> {
    let mut lock = store.lock().unwrap();
    if let Some(t) = lock.get_mut(token) {
        t.revoked = true;
        Some(t.clone())
    } else {
        None
    }
}

impl Db {
    /// Insert or replace a vault in the in-memory store.
    pub fn insert_vault(&self, vault: crate::models::Vault) {
        self.vault_store
            .lock()
            .unwrap()
            .insert(vault.id.clone(), vault);
    }

    /// Retrieve a vault from the in-memory store by string ID.
    pub fn get_vault(&self, vault_id: &str) -> Option<crate::models::Vault> {
        self.vault_store.lock().unwrap().get(vault_id).cloned()
    }
}

// ── Cache-aware vault accessors ───────────────────────────────────────────────

/// Retrieve a `Vault` from the in-memory store, consulting the cache first.
///
/// On a cache miss the vault is fetched from `store`, inserted into the cache
/// and then returned. Returns `None` if the vault does not exist in the store.
pub fn get_vault_cached(
    store: &VaultStore,
    cache: &VaultCache,
    vault_id: &str,
) -> Option<crate::models::Vault> {
    if let Some(v) = cache.get_vault(vault_id) {
        return Some(v);
    }
    let vault = store.lock().unwrap().get(vault_id).cloned()?;
    cache.set_vault(vault_id, vault.clone());
    Some(vault)
}

/// Retrieve the TTL-remaining value for a vault, consulting the cache first.
///
/// Returns `None` if the vault does not exist in the store.
pub fn get_ttl_remaining_cached(
    store: &VaultStore,
    cache: &VaultCache,
    vault_id: &str,
) -> Option<Option<u64>> {
    if let Some(ttl) = cache.get_ttl_remaining(vault_id) {
        return Some(ttl);
    }
    let vault = store.lock().unwrap().get(vault_id).cloned()?;
    let ttl = vault.ttl_remaining;
    cache.set_ttl_remaining(vault_id, ttl);
    Some(ttl)
}

/// Retrieve a lightweight `VaultSummary` for a vault, consulting the cache
/// first.
///
/// Returns `None` if the vault does not exist in the store.
pub fn get_vault_summary_cached(
    store: &VaultStore,
    cache: &VaultCache,
    vault_id: &str,
) -> Option<VaultSummary> {
    if let Some(s) = cache.get_vault_summary(vault_id) {
        return Some(s);
    }
    let vault = store.lock().unwrap().get(vault_id).cloned()?;
    let summary = VaultSummary::from(&vault);
    cache.set_vault_summary(vault_id, summary.clone());
    Some(summary)
}

/// Invalidate all cached entries for `vault_id`.  Must be called whenever
/// a check-in or state-change event modifies vault state.
pub fn invalidate_vault_cache(cache: &VaultCache, vault_id: &str) {
    cache.invalidate(vault_id);
}
