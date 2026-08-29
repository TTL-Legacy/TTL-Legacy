//! OpenTelemetry distributed tracing initialisation and span helpers.
//!
//! # Overview
//!
//! This module configures the OpenTelemetry SDK and bridges it with the
//! [`tracing`] crate so that every `#[instrument]` span is exported to an
//! OTLP-compatible backend (Jaeger, Grafana Tempo, etc.).
//!
//! The OTLP exporter endpoint is configured via the `OTEL_EXPORTER_OTLP_ENDPOINT`
//! environment variable (default: `http://localhost:4317`).
//!
//! # Usage
//!
//! Call [`init_tracer`] once at application startup, before the Axum server
//! starts accepting requests. Store the returned [`OtelGuard`] for the entire
//! lifetime of the process — dropping it triggers a graceful flush.
//!
//! ```rust,no_run
//! #[tokio::main]
//! async fn main() {
//!     let _otel = ttl_legacy_backend::otel::init_tracer("ttl-legacy-backend");
//!     // … start server …
//! }
//! ```
//!
//! # Instrumentation
//!
//! Key handler functions are annotated with `#[tracing::instrument]` in
//! `handlers.rs`. Each span automatically records:
//! - Function arguments (vault_id, amounts, etc.)
//! - Errors via `tracing::error!` / span `record("error", …)`
//! - Outbound Stellar RPC call spans (see [`stellar_rpc_span`])
//!
//! # Issue #1145
//! Add OpenTelemetry Distributed Tracing to Backend

use opentelemetry::global;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{
    runtime,
    trace::{self as sdktrace, RandomIdGenerator, Sampler},
    Resource,
};
use opentelemetry_semantic_conventions::resource::{SERVICE_NAME, SERVICE_VERSION};
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Registry};

/// Opaque guard that flushes and shuts down the OpenTelemetry pipeline when
/// dropped. Hold this for the entire lifetime of the process.
pub struct OtelGuard;

impl Drop for OtelGuard {
    fn drop(&mut self) {
        global::shutdown_tracer_provider();
    }
}

/// Initialise the OpenTelemetry tracing pipeline and install a
/// [`tracing_subscriber`] that exports spans via OTLP.
///
/// # Environment variables
///
/// | Variable | Default | Description |
/// |---|---|---|
/// | `OTEL_EXPORTER_OTLP_ENDPOINT` | `http://localhost:4317` | OTLP gRPC endpoint |
/// | `OTEL_SERVICE_NAME` | `service_name` argument | Overrides the service name |
/// | `RUST_LOG` | `info` | Log / span filter |
///
/// # Panics
///
/// Panics if the OTLP exporter cannot be built (e.g. TLS configuration error).
/// In production use `try_init_tracer` instead.
pub fn init_tracer(service_name: &'static str) -> OtelGuard {
    match try_init_tracer(service_name) {
        Ok(guard) => guard,
        Err(e) => {
            // Fall back to plain tracing-subscriber without OTLP export
            eprintln!("[otel] Failed to initialise OTLP tracer: {e}. Falling back to stdout tracing.");
            init_stdout_tracer();
            OtelGuard
        }
    }
}

/// Fallible variant of [`init_tracer`]. Prefer this in library code.
pub fn try_init_tracer(service_name: &'static str) -> Result<OtelGuard, Box<dyn std::error::Error>> {
    let otlp_endpoint = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
        .unwrap_or_else(|_| "http://localhost:4317".to_string());

    let service_name_override = std::env::var("OTEL_SERVICE_NAME")
        .unwrap_or_else(|_| service_name.to_string());

    let resource = Resource::new(vec![
        opentelemetry::KeyValue::new(SERVICE_NAME, service_name_override.clone()),
        opentelemetry::KeyValue::new(
            SERVICE_VERSION,
            env!("CARGO_PKG_VERSION"),
        ),
    ]);

    let exporter = opentelemetry_otlp::new_exporter()
        .tonic()
        .with_endpoint(&otlp_endpoint);

    let tracer_provider = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(exporter)
        .with_trace_config(
            sdktrace::Config::default()
                .with_sampler(Sampler::AlwaysOn)
                .with_id_generator(RandomIdGenerator::default())
                .with_max_events_per_span(64)
                .with_max_attributes_per_span(32)
                .with_resource(resource),
        )
        .install_batch(runtime::Tokio)?;

    let tracer = tracer_provider.tracer(service_name_override);
    global::set_tracer_provider(tracer_provider);

    let otel_layer = OpenTelemetryLayer::new(tracer);
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let use_json = std::env::var("LOG_FORMAT").map(|v| v.to_lowercase() == "json").unwrap_or(false);

    if use_json {
        Registry::default()
            .with(env_filter)
            .with(tracing_subscriber::fmt::layer().json())
            .with(otel_layer)
            .try_init()?;
    } else {
        Registry::default()
            .with(env_filter)
            .with(tracing_subscriber::fmt::layer())
            .with(otel_layer)
            .try_init()?;
    }

    tracing::info!(
        service = service_name,
        otlp_endpoint = %otlp_endpoint,
        "OpenTelemetry tracing initialised"
    );

    Ok(OtelGuard)
}

/// Initialise plain stdout tracing without OTLP export (fallback path).
fn init_stdout_tracer() {
    let use_json = std::env::var("LOG_FORMAT").map(|v| v.to_lowercase() == "json").unwrap_or(false);
    if use_json {
        let _ = tracing_subscriber::fmt()
            .json()
            .with_env_filter(EnvFilter::from_default_env())
            .try_init();
    } else {
        let _ = tracing_subscriber::fmt()
            .with_env_filter(EnvFilter::from_default_env())
            .try_init();
    }
}

// ---------------------------------------------------------------------------
// Span helpers for outbound Stellar RPC calls
// ---------------------------------------------------------------------------

/// Creates a child span representing a single outbound Stellar RPC call.
///
/// Attach this to any async block that invokes the Soroban RPC endpoint so
/// that latency and errors are visible in the trace waterfall.
///
/// # Example
///
/// ```rust,no_run
/// use tracing::Instrument as _;
/// use crate::otel::stellar_rpc_span;
///
/// async fn invoke_contract() {
///     async {
///         // … soroban client call …
///     }
///     .instrument(stellar_rpc_span("trigger_release", &contract_id))
///     .await;
/// }
/// ```
pub fn stellar_rpc_span(operation: &str, contract_id: &str) -> tracing::Span {
    tracing::info_span!(
        "stellar.rpc",
        otel.kind = "CLIENT",
        db.system = "stellar/soroban",
        db.operation = operation,
        stellar.contract_id = contract_id,
    )
}
