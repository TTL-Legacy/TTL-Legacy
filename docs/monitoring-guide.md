# Monitoring & Alerting Guide

This guide covers setting up Prometheus metrics collection and Grafana dashboards for TTL-Legacy.

## Overview

The backend exposes a `/metrics` endpoint in Prometheus text format. Grafana scrapes this to power dashboards and alerting rules.

## Metrics Exposed

| Metric | Type | Description |
|---|---|---|
| `ttl_legacy_vaults_total` | Counter | Total vaults created |
| `ttl_legacy_checkins_total` | Counter | Total check-ins performed |
| `ttl_legacy_releases_total` | Counter | Total vault releases triggered |
| `ttl_legacy_active_vaults` | Gauge | Currently active (non-released) vaults |
| `ttl_legacy_request_errors_total` | Counter | Total API errors by endpoint |
| `ttl_legacy_contract_paused` | Gauge | 1 if contract is paused, 0 otherwise |
| `ttl_legacy_http_requests_total` | Counter | HTTP requests by method, path, status |
| `ttl_legacy_http_request_duration_seconds` | Histogram | HTTP request latency |

## Prometheus Setup

### 1. Install Prometheus

```bash
# Docker
docker run -d \
  -p 9090:9090 \
  -v $(pwd)/prometheus.yml:/etc/prometheus/prometheus.yml \
  prom/prometheus
```

### 2. Configure Scrape Target

`prometheus.yml`:

```yaml
global:
  scrape_interval: 15s

scrape_configs:
  - job_name: ttl-legacy-backend
    static_configs:
      - targets: ['localhost:8080']
    metrics_path: /metrics
```

### 3. Verify

Open `http://localhost:9090` and query `ttl_legacy_vaults_total`.

## Grafana Setup

### 1. Install Grafana

```bash
docker run -d \
  -p 3000:3000 \
  grafana/grafana
```

Default credentials: `admin` / `admin`.

### 2. Add Prometheus Data Source

1. Go to **Configuration → Data Sources → Add data source**
2. Select **Prometheus**
3. Set URL to `http://localhost:9090`
4. Click **Save & Test**

### 3. Dashboards

#### Vault Volume

```promql
# Vault creation rate (per minute)
rate(ttl_legacy_vaults_total[1m])

# Active vaults
ttl_legacy_active_vaults
```

#### Check-In Rate

```promql
# Check-ins per minute
rate(ttl_legacy_checkins_total[1m])
```

#### Error Rate

```promql
# API error rate
rate(ttl_legacy_request_errors_total[5m])

# Error ratio
rate(ttl_legacy_request_errors_total[5m])
  / rate(ttl_legacy_http_requests_total[5m])
```

## Alerting Rules

Add to `prometheus.yml` or a separate `alerts.yml`:

```yaml
groups:
  - name: ttl-legacy
    rules:
      - alert: HighErrorRate
        expr: |
          rate(ttl_legacy_request_errors_total[5m])
            / rate(ttl_legacy_http_requests_total[5m]) > 0.05
        for: 2m
        labels:
          severity: warning
        annotations:
          summary: "High API error rate (>5%)"

      - alert: BackendDown
        expr: up{job="ttl-legacy-backend"} == 0
        for: 1m
        labels:
          severity: critical
        annotations:
          summary: "TTL-Legacy backend is unreachable"

      - alert: ContractPaused
        expr: ttl_legacy_contract_paused == 1
        for: 0m
        labels:
          severity: warning
        annotations:
          summary: "TTL-Legacy contract is paused"
```

### Vault TTL Below 24 Hours

Add this rule to alert when any vault's remaining TTL drops below 24 hours,
giving owners a last-chance window to check in before their vault is released.

#### PrometheusRule (Kubernetes CRD)

```yaml
apiVersion: monitoring.coreos.com/v1
kind: PrometheusRule
metadata:
  name: ttl-legacy-vault-ttl-low
  labels:
    app: ttl-legacy
    prometheus: kube-prometheus
spec:
  groups:
    - name: ttl-legacy-vault-ttl
      interval: 1m
      rules:
        - alert: VaultTtlBelow24Hours
          expr: |
            ttl_legacy_vault_ttl_remaining_seconds > 0
              and
            ttl_legacy_vault_ttl_remaining_seconds < 86400
          for: 5m
          labels:
            severity: warning
          annotations:
            summary: "Vault TTL is below 24 hours (vault {{ $labels.vault_id }})"
            description: >
              Vault {{ $labels.vault_id }} has only
              {{ $value | humanizeDuration }} of TTL remaining.
              The owner must check in before the TTL expires to prevent
              automatic fund release to the beneficiary.
```

> **Metric note**: `ttl_legacy_vault_ttl_remaining_seconds` is a Gauge that the
> backend TTL-watch service (`ttl_watch.rs`) updates for every active vault each
> polling cycle. Each time-series carries a `vault_id` label so alerts fire
> per-vault rather than in aggregate.

#### Grafana Alerting (JSON model)

Import the following JSON via **Alerting → Alert rules → Import**:

```json
{
  "title": "Vault TTL Below 24 Hours",
  "condition": "C",
  "data": [
    {
      "refId": "A",
      "queryType": "range",
      "relativeTimeRange": { "from": 300, "to": 0 },
      "datasourceUid": "${DS_PROMETHEUS}",
      "model": {
        "expr": "ttl_legacy_vault_ttl_remaining_seconds",
        "legendFormat": "vault {{ vault_id }}",
        "intervalMs": 60000,
        "maxDataPoints": 43200
      }
    },
    {
      "refId": "B",
      "queryType": "instant",
      "relativeTimeRange": { "from": 300, "to": 0 },
      "datasourceUid": "__expr__",
      "model": {
        "type": "reduce",
        "expression": "A",
        "reducer": "last",
        "conditions": []
      }
    },
    {
      "refId": "C",
      "queryType": "instant",
      "relativeTimeRange": { "from": 300, "to": 0 },
      "datasourceUid": "__expr__",
      "model": {
        "type": "threshold",
        "expression": "B",
        "conditions": [
          {
            "evaluator": { "type": "lt", "params": [86400] },
            "operator": { "type": "and" },
            "query": { "params": ["B"] },
            "reducer": { "type": "last", "params": [] }
          }
        ]
      }
    }
  ],
  "noDataState": "NoData",
  "execErrState": "Error",
  "for": "5m",
  "labels": { "severity": "warning" },
  "annotations": {
    "summary": "Vault TTL dropping below 24 h — owner should check in immediately",
    "description": "Vault {{ $labels.vault_id }} has {{ $values.B }} seconds of TTL remaining (threshold: 86 400 s / 24 h)."
  }
}
```

**Setup steps:**

1. In Grafana go to **Alerting → Contact points** and add an email / Slack /
   PagerDuty contact point for vault expiry warnings.
2. Go to **Alerting → Alert rules → New alert rule** and paste the JSON above,
   or create the rule manually using the query builder.
3. Set the notification policy to route `severity=warning` alerts to the
   contact point created in step 1.

### Grafana Alert (UI)

1. Open a panel → **Alert** tab → **Create alert rule**
2. Set condition, e.g. `WHEN last() OF query(A, 5m, now) IS ABOVE 0.05`
3. Configure notification channel (email, Slack, PagerDuty)

## Running the Backend

```bash
cd backend
cargo run
# Metrics available at http://localhost:8080/metrics
```

---

## OpenTelemetry Distributed Tracing

> Issue #1145: Add OpenTelemetry Distributed Tracing to Backend

The backend exports distributed traces via the OpenTelemetry Protocol (OTLP).
Every key operation — vault release simulation, backup/restore, notification
dispatch — is wrapped in a span that captures the vault ID, duration, and
errors. Multi-step flows (backend → Stellar RPC → contract) appear as
connected spans in a trace waterfall.

### Environment Variables

| Variable | Default | Description |
|---|---|---|
| `OTEL_EXPORTER_OTLP_ENDPOINT` | `http://localhost:4317` | OTLP gRPC exporter endpoint |
| `OTEL_SERVICE_NAME` | `ttl-legacy-backend` | Service name shown in Jaeger / Tempo |
| `RUST_LOG` | `info` | Controls span verbosity |

If `OTEL_EXPORTER_OTLP_ENDPOINT` is not set or the exporter fails to connect,
the backend falls back to stdout-only tracing so startup is never blocked.

### Quick Start with Jaeger (all-in-one)

```bash
# Start Jaeger (OTLP gRPC on :4317, UI on :16686)
docker run -d \
  --name jaeger \
  -p 4317:4317 \
  -p 16686:16686 \
  jaegertracing/all-in-one:latest

# Start the backend with tracing enabled
OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317 \
OTEL_SERVICE_NAME=ttl-legacy-backend \
cargo run --manifest-path backend/Cargo.toml

# Open Jaeger UI
open http://localhost:16686
```

### Example Jaeger Queries

Search for all vault release traces in the last hour:

```
Service: ttl-legacy-backend
Operation: simulate_release_handler
Tags: vault_id=<your-vault-id>
Lookback: 1h
```

Find slow Stellar RPC calls:

```
Service: ttl-legacy-backend
Operation: stellar.rpc
Min Duration: 500ms
```

Find all traces with errors:

```
Service: ttl-legacy-backend
Tags: error=true
```

### Instrumented Operations

| Span Name | Attributes | Description |
|---|---|---|
| `simulate_release_handler` | `vault_id` | Full vault release simulation |
| `backup_vault_handler` | `vault_id` | Vault state backup |
| `restore_vault_handler` | `backup_id` | Vault state restore |
| `set_notification_preferences_handler` | `vault_id` | Notification preference update |
| `list_vault_reminders` | `vault_id` | List reminder preferences for a vault |
| `set_preferences` | `vault_id` | Set reminder preferences |
| `get_preferences` | `vault_id` | Get reminder preferences |
| `delete_preferences` | `vault_id` | Delete reminder preferences |
| `unsubscribe` | — | Process unsubscribe request |
| `schedule_expiry_warning` | `vault_id` | Schedule expiry warning notification |
| `schedule_immediate` | `vault_id`, `notification_type` | Schedule immediate notification |
| `flush_pending` | — | Dispatch all due notifications |
| `flush_retries` | — | Retry failed notifications |
| `deliver_with_retry` | `vault_id`, `notification_type`, `attempt` | Single notification delivery with retry |
| `run` | — | Scheduler main loop (reminder + TTL insurance) |
| `extend_ttl_for_inactive_owners` | — | TTL extension for inactive vault owners |
| `stellar.rpc` | `stellar.contract_id`, `db.operation` | Outbound Soroban RPC call |

### Propagating Trace Context

Outbound Stellar RPC calls are wrapped using the `stellar_rpc_span` helper
defined in `backend/src/otel.rs`:

```rust
use crate::otel::stellar_rpc_span;
use tracing::Instrument as _;

async {
    // … soroban client invoke …
}
.instrument(stellar_rpc_span("trigger_release", &contract_id))
.await;
```

This attaches the parent trace context so Jaeger shows the RPC call as a
child span within the handler span.

### Docker Compose Integration

Add Jaeger to `docker-compose.yml` for local development:

```yaml
jaeger:
  image: jaegertracing/all-in-one:latest
  ports:
    - "4317:4317"   # OTLP gRPC
    - "16686:16686" # Jaeger UI
  environment:
    COLLECTOR_OTLP_ENABLED: "true"
```

Then set in `docker-compose.override.yml`:

```yaml
backend:
  environment:
    OTEL_EXPORTER_OTLP_ENDPOINT: http://jaeger:4317
    OTEL_SERVICE_NAME: ttl-legacy-backend
```
