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
//! `handlers/`. Each span automatically records:
//! - Function arguments (vault_id, amounts, etc.)
//! - Errors via `tracing::error!` / span `record("error", …)`
//! - Outbound Stellar RPC call spans (see [`stellar_rpc_span`])
//! - W3C Trace Context (`traceparent` / `tracestate`) propagation on outbound
//!   Soroban RPC calls via [`inject_trace_context`] and [`current_traceparent`]
//!   (see [`crate::soroban_rpc_client`])
//!
//! # Issue #1145
//! Add OpenTelemetry Distributed Tracing to Backend
//!
//! # Issue #2128
//! Infrastructure: add OpenTelemetry trace context propagation to outbound Soroban RPC calls

use std::collections::HashMap;

use opentelemetry::global;
use opentelemetry::propagation::{Injector, TextMapPropagator};
use opentelemetry::trace::{TraceContextExt, TracerProvider as _};
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{
    propagation::TraceContextPropagator,
    runtime,
    trace::{self as sdktrace, RandomIdGenerator, Sampler},
    Resource,
};
use opentelemetry_semantic_conventions::resource::{SERVICE_NAME, SERVICE_VERSION};
use tracing_opentelemetry::{OpenTelemetryLayer, OpenTelemetrySpanExt};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Registry};

/// W3C Trace Context header name for trace propagation (`00-<trace-id>-<span-id>-<flags>`).
pub const TRACEPARENT_HEADER: &str = "traceparent";

/// W3C Trace Context header name for vendor-specific trace state.
pub const TRACESTATE_HEADER: &str = "tracestate";

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
            eprintln!(
                "[otel] Failed to initialise OTLP tracer: {e}. Falling back to stdout tracing."
            );
            init_stdout_tracer();
            OtelGuard
        }
    }
}

/// Fallible variant of [`init_tracer`]. Prefer this in library code.
pub fn try_init_tracer(
    service_name: &'static str,
) -> Result<OtelGuard, Box<dyn std::error::Error>> {
    let otlp_endpoint = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
        .unwrap_or_else(|_| "http://localhost:4317".to_string());

    let service_name_override =
        std::env::var("OTEL_SERVICE_NAME").unwrap_or_else(|_| service_name.to_string());

    let resource = Resource::new(vec![
        opentelemetry::KeyValue::new(SERVICE_NAME, service_name_override.clone()),
        opentelemetry::KeyValue::new(SERVICE_VERSION, env!("CARGO_PKG_VERSION")),
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
    global::set_text_map_propagator(TraceContextPropagator::new());

    let otel_layer = OpenTelemetryLayer::new(tracer);
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let use_json = std::env::var("LOG_FORMAT")
        .map(|v| v.to_lowercase() == "json")
        .unwrap_or(false);

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
    global::set_text_map_propagator(TraceContextPropagator::new());
    let use_json = std::env::var("LOG_FORMAT")
        .map(|v| v.to_lowercase() == "json")
        .unwrap_or(false);
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

// ---------------------------------------------------------------------------
// Outbound W3C Trace Context propagation (Issue #2128)
// ---------------------------------------------------------------------------

struct HeaderMapInjector<'a>(&'a mut reqwest::header::HeaderMap);

impl Injector for HeaderMapInjector<'_> {
    fn set(&mut self, key: &str, value: String) {
        if let (Ok(name), Ok(val)) = (
            reqwest::header::HeaderName::from_bytes(key.as_bytes()),
            reqwest::header::HeaderValue::from_str(&value),
        ) {
            self.0.insert(name, val);
        }
    }
}

/// Formats an OpenTelemetry [`opentelemetry::trace::SpanContext`] as a W3C
/// `traceparent` header value (`00-<trace-id>-<span-id>-<trace-flags>`).
pub fn format_traceparent(span_context: &opentelemetry::trace::SpanContext) -> String {
    format!(
        "00-{}-{}-{:02x}",
        span_context.trace_id(),
        span_context.span_id(),
        span_context.trace_flags().to_u8()
    )
}

/// Generates a deterministic W3C `traceparent` value derived from the active
/// `tracing::Span` metadata when no OpenTelemetry SDK exporter is attached.
///
/// This guarantees that outbound Soroban RPC requests always carry a well-formed
/// W3C `traceparent` header even in local/stdout-only or unit-test environments.
fn fallback_traceparent_from_tracing_span(span: &tracing::Span) -> String {
    use sha2::{Digest, Sha256};

    let span_id_raw = span
        .id()
        .map(|id| id.into_u64())
        .unwrap_or_else(|| {
            // Deterministic non-zero fallback when no tracing subscriber is active
            0x0102_0304_0506_0708
        });

    let mut hasher = Sha256::new();
    hasher.update(b"ttl-legacy-otel-traceparent:");
    hasher.update(span_id_raw.to_be_bytes());
    if let Some(meta) = span.metadata() {
        hasher.update(meta.name().as_bytes());
        hasher.update(meta.target().as_bytes());
    }
    let digest = hasher.finalize();

    // W3C trace-id is 16 bytes (32 lowercase hex chars), must not be all zeros.
    let mut trace_bytes = [0u8; 16];
    trace_bytes.copy_from_slice(&digest[..16]);
    if trace_bytes.iter().all(|&b| b == 0) {
        trace_bytes[15] = 1;
    }

    // W3C span-id is 8 bytes (16 lowercase hex chars), must not be all zeros.
    let span_id_nonzero = if span_id_raw == 0 { 1 } else { span_id_raw };

    let mut trace_hex = String::with_capacity(32);
    for b in trace_bytes {
        use std::fmt::Write as _;
        let _ = write!(&mut trace_hex, "{b:02x}");
    }

    format!("00-{trace_hex}-{span_id_nonzero:016x}-01")
}

/// Validates whether a string conforms to the W3C Trace Context `traceparent`
/// specification (`00-<32-hex-trace-id>-<16-hex-span-id>-<2-hex-flags>`).
pub fn is_valid_traceparent(value: &str) -> bool {
    let parts: Vec<&str> = value.split('-').collect();
    if parts.len() != 4 {
        return false;
    }
    let (version, trace_id, span_id, flags) = (parts[0], parts[1], parts[2], parts[3]);
    if version.len() != 2 || !version.bytes().all(|b| b.is_ascii_hexdigit()) || version == "ff" {
        return false;
    }
    if trace_id.len() != 32
        || !trace_id.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        || trace_id.bytes().all(|b| b == b'0')
    {
        return false;
    }
    if span_id.len() != 16
        || !span_id.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        || span_id.bytes().all(|b| b == b'0')
    {
        return false;
    }
    if flags.len() != 2 || !flags.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    {
        return false;
    }
    true
}

/// Extracts the W3C `traceparent` header value for the given [`tracing::Span`].
///
/// If an OpenTelemetry context with a valid `SpanContext` is attached to `span`,
/// the official W3C `TraceContextPropagator` is used. Otherwise, a valid W3C
/// `traceparent` is synthesized from the active `tracing` span so outbound RPC
/// calls are always correlated.
pub fn span_traceparent(span: &tracing::Span) -> String {
    let cx = span.context();
    let span_ref = cx.span();
    let span_context = span_ref.span_context();
    if span_context.is_valid() {
        format_traceparent(span_context)
    } else {
        fallback_traceparent_from_tracing_span(span)
    }
}

/// Returns the W3C `traceparent` header value for the currently active span.
pub fn current_traceparent() -> String {
    span_traceparent(&tracing::Span::current())
}

/// Returns a map of W3C Trace Context headers (`traceparent` and optionally
/// `tracestate`) for the given [`tracing::Span`].
pub fn trace_context_headers_for_span(span: &tracing::Span) -> HashMap<String, String> {
    let mut headers = HashMap::new();
    let cx = span.context();
    let propagator = TraceContextPropagator::new();
    propagator.inject_context(&cx, &mut headers);

    if !headers.contains_key(TRACEPARENT_HEADER) {
        headers.insert(TRACEPARENT_HEADER.to_string(), span_traceparent(span));
    }
    headers
}

/// Injects the current OpenTelemetry W3C Trace Context (`traceparent` and
/// `tracestate` when present) into a [`reqwest::header::HeaderMap`].
pub fn inject_trace_context(headers: &mut reqwest::header::HeaderMap) {
    inject_span_trace_context(&tracing::Span::current(), headers);
}

/// Injects the OpenTelemetry W3C Trace Context for an explicit [`tracing::Span`]
/// into a [`reqwest::header::HeaderMap`].
pub fn inject_span_trace_context(
    span: &tracing::Span,
    headers: &mut reqwest::header::HeaderMap,
) {
    let cx = span.context();
    let propagator = TraceContextPropagator::new();
    propagator.inject_context(&cx, &mut HeaderMapInjector(headers));

    if !headers.contains_key(TRACEPARENT_HEADER) {
        if let Ok(value) = reqwest::header::HeaderValue::from_str(&span_traceparent(span)) {
            headers.insert(
                reqwest::header::HeaderName::from_static(TRACEPARENT_HEADER),
                value,
            );
        }
    }
}
