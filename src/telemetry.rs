//! OpenTelemetry telemetry helpers.

use opentelemetry::{
    global,
    trace::{TraceContextExt, TracerProvider},
};
use opentelemetry_sdk::{Resource, trace::SdkTracerProvider};
use thiserror::Error;
use tracing::{error, warn};
use tracing_opentelemetry::OpenTelemetrySpanExt;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

/// Name used for the OpenTelemetry tracer.
const TRACER_NAME: &str = "commit-bridge";

/// Fallback log filter used when `RUST_LOG` is not set.
///
/// Only targets within this crate are enabled at `info` level by default,
/// plus slow SQL statements (`sqlx::query` at `warn` level, which
/// includes the `db.statement` attribute in exported spans).
/// Set `RUST_LOG=debug` (or narrower targets such as `sqlx::query=debug`)
/// to enrich spans with per-query details.
const DEFAULT_RUST_LOG: &str = "commit_bridge=info,sqlx::query=warn";

/// Guard that gracefully shuts down the tracer provider on drop.
///
/// Must be held alive while spans are still being emitted;
/// dropping it flushes and shuts down the underlying provider.
#[must_use]
pub struct TelemetryGuard(Option<SdkTracerProvider>);

impl Drop for TelemetryGuard {
    fn drop(&mut self) {
        if let Some(provider) = self.0.take()
            && let Err(e) = provider.shutdown()
        {
            error!("Failed to gracefully shut down tracer provider: {e}");
        }
    }
}

/// Sets up the global OpenTelemetry propagator and tracing subscriber.
pub fn init() -> TelemetryGuard {
    global::set_text_map_propagator(opentelemetry_sdk::propagation::TraceContextPropagator::new());

    let tracer_provider = init_tracer_provider();

    let otel_layer = match &tracer_provider {
        Ok(provider) => {
            Some(tracing_opentelemetry::layer().with_tracer(provider.tracer(TRACER_NAME)))
        }
        Err(_) => None,
    };

    let env_filter = EnvFilter::try_from_default_env().unwrap_or(EnvFilter::new(DEFAULT_RUST_LOG));

    tracing_subscriber::registry()
        .with(env_filter)
        .with(tracing_subscriber::fmt::layer())
        .with(otel_layer)
        .init();

    if let Err(reason) = &tracer_provider {
        warn!("{reason}");
    }

    #[cfg(debug_assertions)]
    warn!("APPLICATION IS RUNNING IN DEBUG MODE.");

    TelemetryGuard(tracer_provider.ok())
}

/// Serializes the current tracing span's OpenTelemetry context into an optional JSON string,
/// logging a warning if serialization fails.
pub fn serialize_current_span_context() -> Option<String> {
    let context = tracing::Span::current().context();
    let mut map = std::collections::HashMap::new();
    global::get_text_map_propagator(|propagator| {
        propagator.inject_context(&context, &mut map);
    });

    if map.is_empty() {
        return None;
    }

    serde_json::to_string(&map)
        .map_err(|e| warn!("Failed to serialize span context: {e}"))
        .ok()
}

/// Adds a span link to the given tracing span
/// from a serialized OpenTelemetry span context string, if valid.
pub fn add_link_from_serialized_context(span: &tracing::Span, span_context: Option<&str>) {
    let Some(span_ctx_str) = span_context else {
        return;
    };

    let Some(parent_ctx) = deserialize_span_context(span_ctx_str)
        .map_err(|e| warn!("Failed to deserialize span context: {e}"))
        .ok()
    else {
        return;
    };

    let remote_span_ctx = parent_ctx.span().span_context().clone();
    if remote_span_ctx.is_valid() {
        span.add_link(remote_span_ctx);
    }
}

/// Reason why OpenTelemetry initialization was skipped or failed.
#[derive(Debug, Error)]
enum TelemetryDisabledReason {
    /// The SDK was disabled via `OTEL_SDK_DISABLED`.
    #[error("OpenTelemetry disabled via OTEL_SDK_DISABLED.")]
    SdkDisabled,

    /// The exporter was disabled via `OTEL_TRACES_EXPORTER=none`.
    #[error("OpenTelemetry disabled via OTEL_TRACES_EXPORTER=none.")]
    ExporterNone,

    /// No OTLP endpoint was configured.
    #[error(
        "OTLP endpoint not configured, telemetry disabled. Set OTEL_EXPORTER_OTLP_ENDPOINT to enable."
    )]
    EndpointNotConfigured,

    /// The OTLP span exporter failed to build.
    #[error("Failed to build OTLP span exporter, telemetry disabled: {0}")]
    ExporterBuildFailed(String),
}

/// Initializes the OpenTelemetry tracer provider if not disabled and configuration is valid.
fn init_tracer_provider() -> Result<SdkTracerProvider, TelemetryDisabledReason> {
    if env_var_is_truthy("OTEL_SDK_DISABLED") {
        return Err(TelemetryDisabledReason::SdkDisabled);
    }

    if std::env::var("OTEL_TRACES_EXPORTER").as_deref() == Ok("none") {
        return Err(TelemetryDisabledReason::ExporterNone);
    }

    if !otlp_endpoint_is_configured() {
        return Err(TelemetryDisabledReason::EndpointNotConfigured);
    }

    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .build()
        .map_err(|e| TelemetryDisabledReason::ExporterBuildFailed(e.to_string()))?;

    Ok(SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .with_resource(build_resource())
        .build())
}

/// Builds the tracer provider resource.
///
/// The SDK-provided resource attributes are kept
/// (`telemetry.sdk.*` and `OTEL_RESOURCE_ATTRIBUTES`),
/// and a fallback service name is applied only when
/// no service name is configured through the environment.
pub(crate) fn build_resource() -> Resource {
    let mut builder = Resource::builder();
    if !service_name_is_configured() {
        builder = builder.with_service_name(TRACER_NAME);
    }
    builder.build()
}

/// Returns `true` if a `service.name` resource attribute
/// is configured through the standard OpenTelemetry environment variables.
pub(crate) fn service_name_is_configured() -> bool {
    std::env::var("OTEL_SERVICE_NAME").is_ok_and(|value| !value.is_empty())
        || std::env::var("OTEL_RESOURCE_ATTRIBUTES").is_ok_and(|value| {
            value
                .split(',')
                .filter_map(|entry| entry.split_once('='))
                .any(|(key, value)| key.trim() == "service.name" && !value.trim().is_empty())
        })
}

/// Returns `true` if an OTLP endpoint has been explicitly configured
/// through the standard OpenTelemetry environment variables.
fn otlp_endpoint_is_configured() -> bool {
    is_non_empty_var("OTEL_EXPORTER_OTLP_TRACES_ENDPOINT")
        || is_non_empty_var("OTEL_EXPORTER_OTLP_ENDPOINT")
}

/// Returns `true` if the environment variable is set to a non-empty value.
fn is_non_empty_var(name: &str) -> bool {
    std::env::var_os(name).is_some_and(|value| !value.is_empty())
}

/// Returns `true` if the environment variable is set to a truthy value (`true` or `1`).
fn env_var_is_truthy(name: &str) -> bool {
    std::env::var(name).is_ok_and(|value| value.eq_ignore_ascii_case("true") || value == "1")
}

/// Deserializes a JSON string into an OpenTelemetry context.
fn deserialize_span_context(s: &str) -> Result<opentelemetry::Context, serde_json::Error> {
    let map: std::collections::HashMap<String, String> = serde_json::from_str(s)?;
    let context = global::get_text_map_propagator(|propagator| propagator.extract(&map));

    Ok(context)
}
