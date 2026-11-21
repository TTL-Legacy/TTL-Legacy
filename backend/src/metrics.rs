use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::Arc;

/// Default check-in latency histogram bucket boundaries, in milliseconds.
pub const DEFAULT_CHECKIN_LATENCY_BUCKETS_MS: [u64; 6] = [10, 50, 100, 500, 1000, 5000];

/// Environment variable used to override the check-in latency buckets.
pub const CHECKIN_LATENCY_BUCKETS_ENV: &str = "CHECKIN_LATENCY_BUCKETS_MS";

/// Parse a comma-separated list of bucket boundaries (in milliseconds).
///
/// Returns `None` when the input is empty, contains a non-numeric entry, or is
/// not strictly increasing (Prometheus requires ascending bucket boundaries).
pub fn parse_checkin_latency_buckets(raw: &str) -> Option<Vec<u64>> {
    let mut buckets = Vec::new();
    for part in raw.split(',') {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            return None;
        }
        let value: u64 = trimmed.parse().ok()?;
        if let Some(&prev) = buckets.last() {
            if value <= prev {
                return None;
            }
        }
        buckets.push(value);
    }
    if buckets.is_empty() {
        None
    } else {
        Some(buckets)
    }
}

/// Resolve the check-in latency buckets from the environment, falling back to
/// the defaults when the variable is unset or invalid.
pub fn checkin_latency_buckets_from_env() -> Vec<u64> {
    match std::env::var(CHECKIN_LATENCY_BUCKETS_ENV) {
        Ok(raw) => parse_checkin_latency_buckets(&raw).unwrap_or_else(|| {
            eprintln!(
                "warning: invalid {CHECKIN_LATENCY_BUCKETS_ENV}={raw:?}, using defaults"
            );
            DEFAULT_CHECKIN_LATENCY_BUCKETS_MS.to_vec()
        }),
        Err(_) => DEFAULT_CHECKIN_LATENCY_BUCKETS_MS.to_vec(),
    }
}

/// Shared metrics state for the TTL-Legacy backend.
pub struct Metrics {
    /// Check-in latency histogram bucket boundaries in milliseconds (ascending).
    pub checkin_latency_buckets_ms: Vec<u64>,
    /// Per-bucket cumulative counts for check-in latency observations.
    pub checkin_latency_bucket_counts: Vec<AtomicU64>,
    pub checkin_latency_ms_sum: AtomicU64,
    pub checkin_latency_ms_count: AtomicU64,
    pub vaults_total: AtomicU64,
    pub checkins_total: AtomicU64,
    pub releases_total: AtomicU64,
    pub active_vaults: AtomicI64,
    pub request_errors_total: AtomicU64,
    pub http_requests_total: AtomicU64,
    pub contract_paused: AtomicU64,
    pub notification_deliveries_total: AtomicU64,
    pub notification_delivery_failures_total: AtomicU64,
    pub notification_push_success_total: AtomicU64,
    pub notification_push_failure_total: AtomicU64,
    pub notification_email_success_total: AtomicU64,
    pub notification_email_failure_total: AtomicU64,
    pub notification_sms_success_total: AtomicU64,
    pub notification_sms_failure_total: AtomicU64,
    pub notification_delivery_latency_ms_sum: AtomicU64,
    pub notification_delivery_latency_ms_count: AtomicU64,
}

impl Default for Metrics {
    fn default() -> Self {
        Self::with_checkin_latency_buckets(DEFAULT_CHECKIN_LATENCY_BUCKETS_MS.to_vec())
    }
}

impl Metrics {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Build metrics with explicit check-in latency bucket boundaries.
    ///
    /// The provided buckets are sorted and de-duplicated; if the result is
    /// empty the defaults are used.
    pub fn with_checkin_latency_buckets(mut buckets: Vec<u64>) -> Self {
        buckets.sort_unstable();
        buckets.dedup();
        if buckets.is_empty() {
            buckets = DEFAULT_CHECKIN_LATENCY_BUCKETS_MS.to_vec();
        }
        let bucket_counts = buckets.iter().map(|_| AtomicU64::new(0)).collect();
        Self {
            checkin_latency_buckets_ms: buckets,
            checkin_latency_bucket_counts: bucket_counts,
            checkin_latency_ms_sum: AtomicU64::new(0),
            checkin_latency_ms_count: AtomicU64::new(0),
            ..Self::empty()
        }
    }

    /// Build metrics using buckets resolved from the environment.
    pub fn from_env() -> Arc<Self> {
        Arc::new(Self::with_checkin_latency_buckets(
            checkin_latency_buckets_from_env(),
        ))
    }

    /// A `Metrics` value with all counters zeroed and default buckets.
    fn empty() -> Self {
        let buckets = DEFAULT_CHECKIN_LATENCY_BUCKETS_MS.to_vec();
        let bucket_counts = buckets.iter().map(|_| AtomicU64::new(0)).collect();
        Self {
            checkin_latency_buckets_ms: buckets,
            checkin_latency_bucket_counts: bucket_counts,
            checkin_latency_ms_sum: AtomicU64::new(0),
            checkin_latency_ms_count: AtomicU64::new(0),
            vaults_total: AtomicU64::new(0),
            checkins_total: AtomicU64::new(0),
            releases_total: AtomicU64::new(0),
            active_vaults: AtomicI64::new(0),
            request_errors_total: AtomicU64::new(0),
            http_requests_total: AtomicU64::new(0),
            contract_paused: AtomicU64::new(0),
            notification_deliveries_total: AtomicU64::new(0),
            notification_delivery_failures_total: AtomicU64::new(0),
            notification_push_success_total: AtomicU64::new(0),
            notification_push_failure_total: AtomicU64::new(0),
            notification_email_success_total: AtomicU64::new(0),
            notification_email_failure_total: AtomicU64::new(0),
            notification_sms_success_total: AtomicU64::new(0),
            notification_sms_failure_total: AtomicU64::new(0),
            notification_delivery_latency_ms_sum: AtomicU64::new(0),
            notification_delivery_latency_ms_count: AtomicU64::new(0),
            notification_push_latency_ms_sum: AtomicU64::new(0),
            notification_push_latency_ms_count: AtomicU64::new(0),
            notification_email_latency_ms_sum: AtomicU64::new(0),
            notification_email_latency_ms_count: AtomicU64::new(0),
            notification_sms_latency_ms_sum: AtomicU64::new(0),
            notification_sms_latency_ms_count: AtomicU64::new(0),
        }
    }

    /// Record a check-in latency observation (in milliseconds).
    pub fn record_checkin_latency(&self, latency_ms: u64) {
        self.checkin_latency_ms_sum
            .fetch_add(latency_ms, Ordering::Relaxed);
        self.checkin_latency_ms_count
            .fetch_add(1, Ordering::Relaxed);
        for (bound, counter) in self
            .checkin_latency_buckets_ms
            .iter()
            .zip(self.checkin_latency_bucket_counts.iter())
        {
            if latency_ms <= *bound {
                counter.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// Record a successful notification delivery for the given channel.
    pub fn record_notification_success(&self, channel: &str) {
        self.notification_deliveries_total
            .fetch_add(1, Ordering::Relaxed);
        self.notification_channel_success(channel)
            .fetch_add(1, Ordering::Relaxed);
    }

    /// Record a failed notification delivery for the given channel.
    pub fn record_notification_failure(&self, channel: &str) {
        self.notification_delivery_failures_total
            .fetch_add(1, Ordering::Relaxed);
        self.notification_channel_failure(channel)
            .fetch_add(1, Ordering::Relaxed);
    }

    /// Record notification delivery latency (in milliseconds) for the given channel.
    pub fn record_notification_latency(&self, channel: &str, latency_ms: u64) {
        self.notification_delivery_latency_ms_sum
            .fetch_add(latency_ms, Ordering::Relaxed);
        self.notification_delivery_latency_ms_count
            .fetch_add(1, Ordering::Relaxed);
        self.notification_channel_latency_sum(channel)
            .fetch_add(latency_ms, Ordering::Relaxed);
        self.notification_channel_latency_count(channel)
            .fetch_add(1, Ordering::Relaxed);
    }

    /// Per-channel success counter, keyed by channel name (push/email/sms).
    pub fn notification_channel_success(&self, channel: &str) -> &AtomicU64 {
        match channel {
            "email" => &self.notification_email_success_total,
            "sms" => &self.notification_sms_success_total,
            _ => &self.notification_push_success_total,
        }
    }

    /// Per-channel failure counter, keyed by channel name (push/email/sms).
    pub fn notification_channel_failure(&self, channel: &str) -> &AtomicU64 {
        match channel {
            "email" => &self.notification_email_failure_total,
            "sms" => &self.notification_sms_failure_total,
            _ => &self.notification_push_failure_total,
        }
    }

    /// Per-channel latency sum (milliseconds), keyed by channel name.
    pub fn notification_channel_latency_sum(&self, channel: &str) -> &AtomicU64 {
        match channel {
            "email" => &self.notification_email_latency_ms_sum,
            "sms" => &self.notification_sms_latency_ms_sum,
            _ => &self.notification_push_latency_ms_sum,
        }
    }

    /// Per-channel latency observation count, keyed by channel name.
    pub fn notification_channel_latency_count(&self, channel: &str) -> &AtomicU64 {
        match channel {
            "email" => &self.notification_email_latency_ms_count,
            "sms" => &self.notification_sms_latency_ms_count,
            _ => &self.notification_push_latency_ms_count,
        }
    }

    /// Render all metrics in Prometheus text exposition format.
    pub fn render(&self) -> String {
        let mut out = String::new();

        push_counter(
            &mut out,
            "ttl_legacy_vaults_total",
            "Total vaults created",
            self.vaults_total.load(Ordering::Relaxed),
        );
        push_counter(
            &mut out,
            "ttl_legacy_checkins_total",
            "Total check-ins performed",
            self.checkins_total.load(Ordering::Relaxed),
        );
        push_counter(
            &mut out,
            "ttl_legacy_releases_total",
            "Total vault releases triggered",
            self.releases_total.load(Ordering::Relaxed),
        );
        push_gauge_i64(
            &mut out,
            "ttl_legacy_active_vaults",
            "Currently active (non-released) vaults",
            self.active_vaults.load(Ordering::Relaxed),
        );
        push_counter(
            &mut out,
            "ttl_legacy_request_errors_total",
            "Total API errors",
            self.request_errors_total.load(Ordering::Relaxed),
        );
        push_counter(
            &mut out,
            "ttl_legacy_http_requests_total",
            "Total HTTP requests",
            self.http_requests_total.load(Ordering::Relaxed),
        );
        push_gauge(
            &mut out,
            "ttl_legacy_contract_paused",
            "1 if contract is paused, 0 otherwise",
            self.contract_paused.load(Ordering::Relaxed),
        );
        push_counter(
            &mut out,
            "ttl_legacy_notification_deliveries_total",
            "Total successful notification deliveries across all channels",
            self.notification_deliveries_total.load(Ordering::Relaxed),
        );
        push_counter(
            &mut out,
            "ttl_legacy_notification_delivery_failures_total",
            "Total failed notification deliveries across all channels",
            self.notification_delivery_failures_total.load(Ordering::Relaxed),
        );
        push_counter(
            &mut out,
            "ttl_legacy_notification_push_success_total",
            "Total successful push notification deliveries",
            self.notification_push_success_total.load(Ordering::Relaxed),
        );
        push_counter(
            &mut out,
            "ttl_legacy_notification_push_failure_total",
            "Total failed push notification deliveries",
            self.notification_push_failure_total.load(Ordering::Relaxed),
        );
        push_counter(
            &mut out,
            "ttl_legacy_notification_email_success_total",
            "Total successful email notification deliveries",
            self.notification_email_success_total.load(Ordering::Relaxed),
        );
        push_counter(
            &mut out,
            "ttl_legacy_notification_email_failure_total",
            "Total failed email notification deliveries",
            self.notification_email_failure_total.load(Ordering::Relaxed),
        );
        push_counter(
            &mut out,
            "ttl_legacy_notification_sms_success_total",
            "Total successful SMS notification deliveries",
            self.notification_sms_success_total.load(Ordering::Relaxed),
        );
        push_counter(
            &mut out,
            "ttl_legacy_notification_sms_failure_total",
            "Total failed SMS notification deliveries",
            self.notification_sms_failure_total.load(Ordering::Relaxed),
        );
        push_bucketed_histogram(
            &mut out,
            "ttl_legacy_checkin_latency_ms",
            "Check-in latency in milliseconds",
            &self.checkin_latency_buckets_ms,
            &self.checkin_latency_bucket_counts,
            self.checkin_latency_ms_sum.load(Ordering::Relaxed),
            self.checkin_latency_ms_count.load(Ordering::Relaxed),
        );
        push_histogram(
            &mut out,
            "ttl_legacy_notification_delivery_latency_ms",
            "Notification delivery latency in milliseconds across all channels",
            self.notification_delivery_latency_ms_sum.load(Ordering::Relaxed),
            self.notification_delivery_latency_ms_count.load(Ordering::Relaxed),
        );
        push_histogram(
            &mut out,
            "ttl_legacy_notification_push_delivery_latency_ms",
            "Push notification delivery latency in milliseconds",
            self.notification_push_latency_ms_sum.load(Ordering::Relaxed),
            self.notification_push_latency_ms_count.load(Ordering::Relaxed),
        );
        push_histogram(
            &mut out,
            "ttl_legacy_notification_email_delivery_latency_ms",
            "Email notification delivery latency in milliseconds",
            self.notification_email_latency_ms_sum.load(Ordering::Relaxed),
            self.notification_email_latency_ms_count.load(Ordering::Relaxed),
        );
        push_histogram(
            &mut out,
            "ttl_legacy_notification_sms_delivery_latency_ms",
            "SMS notification delivery latency in milliseconds",
            self.notification_sms_latency_ms_sum.load(Ordering::Relaxed),
            self.notification_sms_latency_ms_count.load(Ordering::Relaxed),
        );

        out
    }
}

fn push_counter(out: &mut String, name: &str, help: &str, value: u64) {
    out.push_str(&format!("# HELP {name} {help}\n"));
    out.push_str(&format!("# TYPE {name} counter\n"));
    out.push_str(&format!("{name} {value}\n"));
}

fn push_gauge(out: &mut String, name: &str, help: &str, value: u64) {
    out.push_str(&format!("# HELP {name} {help}\n"));
    out.push_str(&format!("# TYPE {name} gauge\n"));
    out.push_str(&format!("{name} {value}\n"));
}

fn push_gauge_i64(out: &mut String, name: &str, help: &str, value: i64) {
    out.push_str(&format!("# HELP {name} {help}\n"));
    out.push_str(&format!("# TYPE {name} gauge\n"));
    out.push_str(&format!("{name} {value}\n"));
}

fn push_histogram(out: &mut String, name: &str, help: &str, sum: u64, count: u64) {
    out.push_str(&format!("# HELP {name} {help}\n"));
    out.push_str(&format!("# TYPE {name} histogram\n"));
    out.push_str(&format!("{name}_sum {sum}\n"));
    out.push_str(&format!("{name}_count {count}\n"));
}

fn push_bucketed_histogram(
    out: &mut String,
    name: &str,
    help: &str,
    bounds: &[u64],
    counts: &[AtomicU64],
    sum: u64,
    count: u64,
) {
    out.push_str(&format!("# HELP {name} {help}\n"));
    out.push_str(&format!("# TYPE {name} histogram\n"));
    for (bound, counter) in bounds.iter().zip(counts.iter()) {
        let value = counter.load(Ordering::Relaxed);
        out.push_str(&format!("{name}_bucket{{le=\"{bound}\"}} {value}\n"));
    }
    out.push_str(&format!("{name}_bucket{{le=\"+Inf\"}} {count}\n"));
    out.push_str(&format!("{name}_sum {sum}\n"));
    out.push_str(&format!("{name}_count {count}\n"));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_contains_all_metrics() {
        let m = Metrics::new();
        m.vaults_total.store(5, Ordering::Relaxed);
        m.checkins_total.store(10, Ordering::Relaxed);
        m.contract_paused.store(1, Ordering::Relaxed);

        let output = m.render();
        assert!(output.contains("ttl_legacy_vaults_total 5"));
        assert!(output.contains("ttl_legacy_checkins_total 10"));
        assert!(output.contains("ttl_legacy_contract_paused 1"));
    }

    #[test]
    fn test_render_prometheus_format() {
        let m = Metrics::new();
        let output = m.render();
        assert!(output.contains("# HELP ttl_legacy_vaults_total"));
        assert!(output.contains("# TYPE ttl_legacy_vaults_total counter"));
        assert!(output.contains("# TYPE ttl_legacy_active_vaults gauge"));
    }

    #[test]
    fn test_notification_channel_metrics_per_channel() {
        let m = Metrics::new();

        m.record_notification_success("push");
        m.record_notification_success("email");
        m.record_notification_success("email");
        m.record_notification_failure("sms");

        let output = m.render();
        assert!(output.contains("ttl_legacy_notification_push_success_total 1"));
        assert!(output.contains("ttl_legacy_notification_email_success_total 2"));
        assert!(output.contains("ttl_legacy_notification_sms_failure_total 1"));
        assert!(output.contains("ttl_legacy_notification_deliveries_total 3"));
        assert!(output.contains("ttl_legacy_notification_delivery_failures_total 1"));
    }

    #[test]
    fn test_notification_channel_counters_are_independent() {
        let m = Metrics::new();

        m.record_notification_success("push");
        m.record_notification_failure("push");
        m.record_notification_success("email");
        m.record_notification_failure("sms");

        let output = m.render();
        assert!(output.contains("ttl_legacy_notification_push_success_total 1"));
        assert!(output.contains("ttl_legacy_notification_push_failure_total 1"));
        assert!(output.contains("ttl_legacy_notification_email_success_total 1"));
        assert!(output.contains("ttl_legacy_notification_email_failure_total 0"));
        assert!(output.contains("ttl_legacy_notification_sms_success_total 0"));
        assert!(output.contains("ttl_legacy_notification_sms_failure_total 1"));
    }

    #[test]
    fn test_parse_checkin_latency_buckets_valid() {
        assert_eq!(
            parse_checkin_latency_buckets("10,50,100,500,1000,5000"),
            Some(vec![10, 50, 100, 500, 1000, 5000])
        );
        assert_eq!(
            parse_checkin_latency_buckets(" 5 , 25 ,100 "),
            Some(vec![5, 25, 100])
        );
    }

    #[test]
    fn test_parse_checkin_latency_buckets_invalid() {
        assert_eq!(parse_checkin_latency_buckets(""), None);
        assert_eq!(parse_checkin_latency_buckets("10,,50"), None);
        assert_eq!(parse_checkin_latency_buckets("10,abc,50"), None);
        assert_eq!(parse_checkin_latency_buckets("-10,50"), None);
        // Not strictly increasing.
        assert_eq!(parse_checkin_latency_buckets("100,50"), None);
        assert_eq!(parse_checkin_latency_buckets("50,50"), None);
    }

    #[test]
    fn test_checkin_latency_buckets_fallback_to_defaults() {
        // Invalid input falls back to the defaults.
        let buckets = parse_checkin_latency_buckets("not,a,list")
            .unwrap_or_else(|| DEFAULT_CHECKIN_LATENCY_BUCKETS_MS.to_vec());
        assert_eq!(buckets, DEFAULT_CHECKIN_LATENCY_BUCKETS_MS.to_vec());

        // Empty bucket list also falls back to defaults.
        let m = Metrics::with_checkin_latency_buckets(Vec::new());
        assert_eq!(
            m.checkin_latency_buckets_ms,
            DEFAULT_CHECKIN_LATENCY_BUCKETS_MS.to_vec()
        );
    }

    #[test]
    fn test_checkin_latency_histogram_uses_configured_buckets() {
        let m = Metrics::with_checkin_latency_buckets(vec![100, 200, 300]);
        m.record_checkin_latency(50);
        m.record_checkin_latency(150);
        m.record_checkin_latency(1000);

        let output = m.render();
        assert!(output.contains("# TYPE ttl_legacy_checkin_latency_ms histogram"));
        assert!(output.contains("ttl_legacy_checkin_latency_ms_bucket{le=\"100\"} 1"));
        assert!(output.contains("ttl_legacy_checkin_latency_ms_bucket{le=\"200\"} 2"));
        assert!(output.contains("ttl_legacy_checkin_latency_ms_bucket{le=\"300\"} 2"));
        assert!(output.contains("ttl_legacy_checkin_latency_ms_bucket{le=\"+Inf\"} 3"));
        assert!(output.contains("ttl_legacy_checkin_latency_ms_sum 1200"));
        assert!(output.contains("ttl_legacy_checkin_latency_ms_count 3"));
    }

    #[test]
    fn test_checkin_latency_default_buckets() {
        let m = Metrics::new();
        assert_eq!(
            m.checkin_latency_buckets_ms,
            DEFAULT_CHECKIN_LATENCY_BUCKETS_MS.to_vec()
        );
        let output = m.render();
        assert!(output.contains("ttl_legacy_checkin_latency_ms_bucket{le=\"10\"} 0"));
        assert!(output.contains("ttl_legacy_checkin_latency_ms_bucket{le=\"5000\"} 0"));
    }

    #[test]
    fn test_notification_delivery_latency_histogram() {
        let m = Metrics::new();

        m.record_notification_latency("push", 120);
        m.record_notification_latency("push", 80);
        m.record_notification_latency("email", 250);

        let output = m.render();
        assert!(output.contains("# TYPE ttl_legacy_notification_delivery_latency_ms histogram"));
        assert!(output.contains("ttl_legacy_notification_delivery_latency_ms_sum 450"));
        assert!(output.contains("ttl_legacy_notification_delivery_latency_ms_count 3"));
        assert!(output.contains("ttl_legacy_notification_push_delivery_latency_ms_sum 200"));
        assert!(output.contains("ttl_legacy_notification_push_delivery_latency_ms_count 2"));
        assert!(output.contains("ttl_legacy_notification_email_delivery_latency_ms_sum 250"));
        assert!(output.contains("ttl_legacy_notification_email_delivery_latency_ms_count 1"));
        assert!(output.contains("ttl_legacy_notification_sms_delivery_latency_ms_count 0"));
    }
}
