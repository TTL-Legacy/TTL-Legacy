/// Token price oracle integration for vault value estimation.
///
/// Issue #2066 — `TokenPriceOracleConfig` backend support.
///
/// This module provides:
/// * In-memory storage for the global oracle configuration.
/// * A price-cache that the scheduler populates on each refresh tick.
/// * Helper functions used by the route handler to compute
///   `EstimatedVaultValue` responses.
use std::sync::{Arc, RwLock};

use chrono::Utc;

use crate::models::{
    CachedTokenPrice, EstimatedVaultValue, OracleProvider, SetOracleConfigRequest,
    TokenPriceOracleConfig,
};

/// Stroops per token (Stellar native precision).
const STROOPS_PER_TOKEN: f64 = 10_000_000.0;

/// How many multiples of `refresh_interval_seconds` before a price is stale.
const STALE_MULTIPLIER: u64 = 2;

/// Shared oracle state threaded through the application via `Arc`.
#[derive(Debug, Default)]
pub struct OracleState {
    /// Currently active oracle configuration, `None` until first `PUT`.
    pub config: Option<TokenPriceOracleConfig>,
    /// Most recently cached price entries, keyed by `token_id`.
    pub price_cache: Vec<CachedTokenPrice>,
}

pub type SharedOracleState = Arc<RwLock<OracleState>>;

/// Build a new, empty shared oracle state ready to be injected into Actix.
pub fn new_shared_oracle_state() -> SharedOracleState {
    Arc::new(RwLock::new(OracleState::default()))
}

/// Apply a `SetOracleConfigRequest` to the shared oracle state.
///
/// Returns the resulting `TokenPriceOracleConfig` on success, or an error
/// string when the request is invalid (e.g. `refresh_interval_seconds < 10`).
pub fn apply_oracle_config(
    state: &SharedOracleState,
    req: SetOracleConfigRequest,
) -> Result<TokenPriceOracleConfig, String> {
    if req.refresh_interval_seconds < 10 {
        return Err(
            "refresh_interval_seconds must be at least 10".to_owned(),
        );
    }
    if req.endpoint_url.is_empty() {
        return Err("endpoint_url must not be empty".to_owned());
    }
    if req.xlm_feed_id.is_empty() {
        return Err("xlm_feed_id must not be empty".to_owned());
    }

    let config = TokenPriceOracleConfig {
        provider: req.provider,
        endpoint_url: req.endpoint_url,
        refresh_interval_seconds: req.refresh_interval_seconds,
        xlm_feed_id: req.xlm_feed_id,
        enabled: req.enabled,
        updated_at: Utc::now(),
    };

    let mut guard = state.write().map_err(|e| e.to_string())?;
    guard.config = Some(config.clone());
    Ok(config)
}

/// Store a freshly fetched price in the shared cache.
///
/// An existing entry for the same `token_id` is replaced.
pub fn update_price_cache(state: &SharedOracleState, price: CachedTokenPrice) {
    if let Ok(mut guard) = state.write() {
        guard
            .price_cache
            .retain(|p| p.token_id != price.token_id);
        guard.price_cache.push(price);
    }
}

/// Look up the cached price for `token_id`.
pub fn get_cached_price(
    state: &SharedOracleState,
    token_id: &str,
) -> Option<CachedTokenPrice> {
    state
        .read()
        .ok()?
        .price_cache
        .iter()
        .find(|p| p.token_id == token_id)
        .cloned()
}

/// Compute the estimated USD value for a vault.
///
/// `balance_stroops` is the vault's raw on-chain balance.
/// `token_id` is `"XLM"` for native or the SEP-41 contract address.
///
/// Returns `None` when no oracle configuration is present or the price
/// cache is empty for the requested token.
pub fn estimate_vault_value(
    state: &SharedOracleState,
    vault_id: &str,
    balance_stroops: i128,
    token_id: &str,
) -> Option<EstimatedVaultValue> {
    let guard = state.read().ok()?;
    let config = guard.config.as_ref()?;
    let cached = guard
        .price_cache
        .iter()
        .find(|p| p.token_id == token_id)
        .cloned();

    let now = Utc::now();

    let (estimated_value_usd, price_per_token_usd, price_fetched_at, is_stale) =
        if let Some(ref p) = cached {
            let age_seconds = now
                .signed_duration_since(p.fetched_at)
                .num_seconds()
                .unsigned_abs();
            let stale_threshold = config.refresh_interval_seconds * STALE_MULTIPLIER;
            let stale = age_seconds > stale_threshold;

            let token_units = balance_stroops as f64 / STROOPS_PER_TOKEN;
            let value = token_units * p.price_usd;
            (
                Some(value),
                Some(p.price_usd),
                Some(p.fetched_at),
                stale,
            )
        } else {
            (None, None, None, false)
        };

    Some(EstimatedVaultValue {
        vault_id: vault_id.to_owned(),
        balance_stroops,
        estimated_value_usd,
        token_symbol: token_id.to_owned(),
        price_per_token_usd,
        price_fetched_at,
        is_stale,
        estimated_at: now,
    })
}

/// Retrieve the current oracle configuration.
pub fn get_oracle_config(state: &SharedOracleState) -> Option<TokenPriceOracleConfig> {
    state.read().ok()?.config.clone()
}

/// Determine whether the oracle poller should run a refresh tick now.
///
/// Returns `true` when the oracle is enabled and the most recently cached
/// price for `"XLM"` is older than `refresh_interval_seconds`.
pub fn should_refresh(state: &SharedOracleState) -> bool {
    let guard = match state.read() {
        Ok(g) => g,
        Err(_) => return false,
    };
    let config = match guard.config.as_ref() {
        Some(c) if c.enabled => c,
        _ => return false,
    };
    let refresh_secs = config.refresh_interval_seconds;

    guard
        .price_cache
        .iter()
        .find(|p| p.token_id == "XLM")
        .map(|p| {
            let age = Utc::now()
                .signed_duration_since(p.fetched_at)
                .num_seconds()
                .unsigned_abs();
            age >= refresh_secs
        })
        .unwrap_or(true) // no cached entry → refresh immediately
}

/// Simulate a price fetch and update the cache (used by the scheduler).
///
/// In production this function would make an HTTP request to the configured
/// oracle endpoint. Here we return a stub price so the compilation unit is
/// self-contained without an async HTTP client dependency. The actual HTTP
/// integration should be wired up in the scheduler task that calls this.
pub fn simulate_price_fetch(
    state: &SharedOracleState,
    token_id: &str,
    price_usd: f64,
) {
    let provider = state
        .read()
        .ok()
        .and_then(|g| g.config.as_ref().map(|c| c.provider.clone()))
        .unwrap_or(OracleProvider::Redstone);

    update_price_cache(
        state,
        CachedTokenPrice {
            token_id: token_id.to_owned(),
            price_usd,
            fetched_at: Utc::now(),
            provider,
        },
    );
}
