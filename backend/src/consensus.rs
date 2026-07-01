use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

/// A versioned cache entry shared across backend nodes.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CacheEntry {
    pub key: String,
    pub value: String,
    pub node_id: String,
    pub updated_at: DateTime<Utc>,
    pub version: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictStrategy {
    LastWriteWins,
    Voting,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct ConflictDetail {
    pub key: String,
    pub local: Option<CacheEntry>,
    pub remote: Option<CacheEntry>,
    pub winner: Option<CacheEntry>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct ConsensusReport {
    pub consistent: bool,
    pub node_id: String,
    pub strategy: ConflictStrategy,
    pub conflicts: Vec<ConflictDetail>,
    pub conflicts_resolved: usize,
    pub keys_checked: usize,
}

pub trait CacheBackend: Send + Sync {
    fn get_entry(&self, key: &str) -> Result<Option<CacheEntry>, String>;
    fn set_entry(&self, entry: &CacheEntry) -> Result<(), String>;
    fn all_entries(&self) -> Result<Vec<CacheEntry>, String>;
}

/// In-memory backend used for tests and single-node deployments without Redis.
#[derive(Default)]
pub struct InMemoryBackend {
    entries: Mutex<HashMap<String, CacheEntry>>,
}

impl InMemoryBackend {
    pub fn new() -> Self {
        Self::default()
    }
}

impl CacheBackend for InMemoryBackend {
    fn get_entry(&self, key: &str) -> Result<Option<CacheEntry>, String> {
        Ok(self.entries.lock().unwrap().get(key).cloned())
    }

    fn set_entry(&self, entry: &CacheEntry) -> Result<(), String> {
        self.entries
            .lock()
            .unwrap()
            .insert(entry.key.clone(), entry.clone());
        Ok(())
    }

    fn all_entries(&self) -> Result<Vec<CacheEntry>, String> {
        Ok(self.entries.lock().unwrap().values().cloned().collect())
    }
}

/// Redis-backed distributed cache for multi-node deployments.
pub struct RedisBackend {
    client: redis::Client,
    prefix: String,
}

impl RedisBackend {
    pub fn new(redis_url: &str) -> Result<Self, String> {
        let client = redis::Client::open(redis_url).map_err(|e| e.to_string())?;
        Ok(Self {
            client,
            prefix: "ttl:consensus:".to_string(),
        })
    }

    fn redis_key(&self, key: &str) -> String {
        format!("{}{}", self.prefix, key)
    }
}

impl CacheBackend for RedisBackend {
    fn get_entry(&self, key: &str) -> Result<Option<CacheEntry>, String> {
        let mut conn = self.client.get_connection().map_err(|e| e.to_string())?;
        let raw: Option<String> = redis::cmd("GET")
            .arg(self.redis_key(key))
            .query(&mut conn)
            .map_err(|e| e.to_string())?;
        raw.map(|json| serde_json::from_str(&json).map_err(|e| e.to_string()))
            .transpose()
    }

    fn set_entry(&self, entry: &CacheEntry) -> Result<(), String> {
        let mut conn = self.client.get_connection().map_err(|e| e.to_string())?;
        let json = serde_json::to_string(entry).map_err(|e| e.to_string())?;
        redis::cmd("SET")
            .arg(self.redis_key(&entry.key))
            .arg(json)
            .query::<()>(&mut conn)
            .map_err(|e| e.to_string())
    }

    fn all_entries(&self) -> Result<Vec<CacheEntry>, String> {
        let mut conn = self.client.get_connection().map_err(|e| e.to_string())?;
        let keys: Vec<String> = redis::cmd("KEYS")
            .arg(format!("{}*", self.prefix))
            .query(&mut conn)
            .map_err(|e| e.to_string())?;

        let mut entries = Vec::with_capacity(keys.len());
        for redis_key in keys {
            let raw: Option<String> = redis::cmd("GET")
                .arg(&redis_key)
                .query(&mut conn)
                .map_err(|e| e.to_string())?;
            if let Some(json) = raw {
                entries.push(serde_json::from_str(&json).map_err(|e| e.to_string())?);
            }
        }
        Ok(entries)
    }
}

/// Per-node cache that mirrors the distributed backend and resolves conflicts.
pub struct NodeCache {
    node_id: String,
    backend: Arc<dyn CacheBackend>,
    local: Mutex<HashMap<String, CacheEntry>>,
    strategy: ConflictStrategy,
}

impl NodeCache {
    pub fn new(
        node_id: impl Into<String>,
        backend: Arc<dyn CacheBackend>,
        strategy: ConflictStrategy,
    ) -> Self {
        Self {
            node_id: node_id.into(),
            backend,
            local: Mutex::new(HashMap::new()),
            strategy,
        }
    }

    pub fn node_id(&self) -> &str {
        &self.node_id
    }

    pub fn strategy(&self) -> ConflictStrategy {
        self.strategy
    }

    pub fn from_env() -> Arc<Self> {
        let node_id = std::env::var("NODE_ID")
            .unwrap_or_else(|_| uuid::Uuid::new_v4().to_string());
        let strategy = match std::env::var("CONSENSUS_STRATEGY")
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str()
        {
            "voting" => ConflictStrategy::Voting,
            _ => ConflictStrategy::LastWriteWins,
        };

        let backend: Arc<dyn CacheBackend> =
            if let Ok(redis_url) = std::env::var("REDIS_URL") {
                Arc::new(
                    RedisBackend::new(&redis_url)
                        .unwrap_or_else(|e| panic!("failed to connect to Redis: {e}")),
                )
            } else {
                Arc::new(InMemoryBackend::new())
            };

        Arc::new(Self::new(node_id, backend, strategy))
    }

    pub fn put(&self, key: &str, value: &str) -> Result<CacheEntry, String> {
        let entry = self.make_entry(key, value);
        self.backend.set_entry(&entry)?;
        self.local.lock().unwrap().insert(key.to_string(), entry.clone());
        Ok(entry)
    }

    pub fn get(&self, key: &str) -> Result<Option<CacheEntry>, String> {
        self.backend.get_entry(key)
    }

    pub fn sync_local_from_remote(&self) -> Result<(), String> {
        let entries = self.backend.all_entries()?;
        let mut local = self.local.lock().unwrap();
        local.clear();
        for entry in entries {
            local.insert(entry.key.clone(), entry);
        }
        Ok(())
    }

    /// Compare local cache against the distributed backend and resolve conflicts.
    pub fn check_and_resolve(&self) -> Result<ConsensusReport, String> {
        let remote_entries: HashMap<String, CacheEntry> = self
            .backend
            .all_entries()?
            .into_iter()
            .map(|entry| (entry.key.clone(), entry))
            .collect();

        let local_snapshot = self.local.lock().unwrap().clone();
        let all_keys: HashSet<String> = local_snapshot
            .keys()
            .chain(remote_entries.keys())
            .cloned()
            .collect();

        let mut conflicts = Vec::new();
        let mut conflicts_resolved = 0;

        for key in &all_keys {
            let local_entry = local_snapshot.get(key);
            let remote_entry = remote_entries.get(key);

            if entries_equal(local_entry, remote_entry) {
                continue;
            }

            let winner = resolve_conflict(local_entry, remote_entry, self.strategy);
            conflicts.push(ConflictDetail {
                key: key.clone(),
                local: local_entry.cloned(),
                remote: remote_entry.cloned(),
                winner: winner.clone(),
            });

            if let Some(resolved) = winner {
                self.backend.set_entry(&resolved)?;
                self.local
                    .lock()
                    .unwrap()
                    .insert(key.clone(), resolved);
                conflicts_resolved += 1;
            }
        }

        let consistent = conflicts.is_empty();
        Ok(ConsensusReport {
            consistent,
            node_id: self.node_id.clone(),
            strategy: self.strategy,
            conflicts,
            conflicts_resolved,
            keys_checked: all_keys.len(),
        })
    }

    fn make_entry(&self, key: &str, value: &str) -> CacheEntry {
        let updated_at = Utc::now();
        CacheEntry {
            key: key.to_string(),
            value: value.to_string(),
            node_id: self.node_id.clone(),
            updated_at,
            version: updated_at.timestamp_millis() as u64,
        }
    }

    #[cfg(test)]
    pub fn set_local_entry(&self, entry: CacheEntry) {
        self.local
            .lock()
            .unwrap()
            .insert(entry.key.clone(), entry);
    }
}

fn entries_equal(a: Option<&CacheEntry>, b: Option<&CacheEntry>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(left), Some(right)) => left == right,
        _ => false,
    }
}

pub fn resolve_conflict(
    local: Option<&CacheEntry>,
    remote: Option<&CacheEntry>,
    strategy: ConflictStrategy,
) -> Option<CacheEntry> {
    match (local, remote) {
        (None, None) => None,
        (None, Some(remote)) => Some(remote.clone()),
        (Some(local), None) => Some(local.clone()),
        (Some(local), Some(remote)) => match strategy {
            ConflictStrategy::LastWriteWins => Some(pick_latest_by_timestamp(local, remote)),
            ConflictStrategy::Voting => Some(pick_by_voting(local, remote)),
        },
    }
}

fn pick_latest_by_timestamp(local: &CacheEntry, remote: &CacheEntry) -> CacheEntry {
    if local.updated_at >= remote.updated_at {
        local.clone()
    } else {
        remote.clone()
    }
}

fn pick_by_voting(local: &CacheEntry, remote: &CacheEntry) -> CacheEntry {
    if local.version > remote.version {
        local.clone()
    } else if remote.version > local.version {
        remote.clone()
    } else if local.node_id <= remote.node_id {
        local.clone()
    } else {
        remote.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn sample_entry(key: &str, value: &str, node: &str, millis: i64) -> CacheEntry {
        CacheEntry {
            key: key.to_string(),
            value: value.to_string(),
            node_id: node.to_string(),
            updated_at: Utc.timestamp_millis_opt(millis).unwrap(),
            version: millis as u64,
        }
    }

    fn shared_backend() -> Arc<InMemoryBackend> {
        Arc::new(InMemoryBackend::new())
    }

    #[test]
    fn lww_prefers_latest_timestamp() {
        let older = sample_entry("k", "old", "node-a", 1_000);
        let newer = sample_entry("k", "new", "node-b", 2_000);
        let winner = resolve_conflict(
            Some(&older),
            Some(&newer),
            ConflictStrategy::LastWriteWins,
        )
        .unwrap();
        assert_eq!(winner.value, "new");
    }

    #[test]
    fn voting_prefers_higher_version() {
        let low = sample_entry("k", "low", "node-a", 1_000);
        let high = sample_entry("k", "high", "node-b", 2_000);
        let winner =
            resolve_conflict(Some(&low), Some(&high), ConflictStrategy::Voting).unwrap();
        assert_eq!(winner.value, "high");
    }

    #[test]
    fn multi_node_converges_after_consensus_check() {
        let backend: Arc<dyn CacheBackend> = shared_backend();
        let node_a = NodeCache::new("node-a", Arc::clone(&backend), ConflictStrategy::LastWriteWins);
        let node_b = NodeCache::new("node-b", Arc::clone(&backend), ConflictStrategy::LastWriteWins);

        node_a.put("vault:1", "prefs-a").unwrap();
        let remote = node_b.get("vault:1").unwrap().expect("remote entry");
        let stale = sample_entry("vault:1", "stale-local", "node-b", 1);
        node_b.set_local_entry(stale);

        let report = node_b.check_and_resolve().unwrap();
        assert!(!report.consistent);
        assert_eq!(report.conflicts_resolved, 1);

        let resolved = node_b.get("vault:1").unwrap().expect("resolved entry");
        assert_eq!(resolved.value, remote.value);
    }

    #[test]
    fn two_nodes_share_backend_state() {
        let backend: Arc<dyn CacheBackend> = shared_backend();
        let node_a = NodeCache::new("node-a", Arc::clone(&backend), ConflictStrategy::LastWriteWins);
        let node_b = NodeCache::new("node-b", Arc::clone(&backend), ConflictStrategy::LastWriteWins);

        node_a.put("vault:2", "shared").unwrap();
        node_b.sync_local_from_remote().unwrap();

        let report = node_b.check_and_resolve().unwrap();
        assert!(report.consistent);
        assert_eq!(node_b.get("vault:2").unwrap().unwrap().value, "shared");
    }

    #[test]
    fn voting_strategy_resolves_competing_writes() {
        let backend: Arc<dyn CacheBackend> = shared_backend();
        let node_a = NodeCache::new("node-a", Arc::clone(&backend), ConflictStrategy::Voting);
        let node_b = NodeCache::new("node-b", Arc::clone(&backend), ConflictStrategy::Voting);

        let newer = sample_entry("vault:3", "winner", "node-a", 5_000);
        backend.set_entry(&newer).unwrap();
        node_a.set_local_entry(newer.clone());
        node_b.set_local_entry(sample_entry("vault:3", "loser", "node-b", 2_000));

        let report = node_b.check_and_resolve().unwrap();
        assert_eq!(report.conflicts_resolved, 1);
        assert_eq!(
            node_b.get("vault:3").unwrap().unwrap().value,
            "winner"
        );
    }
}
