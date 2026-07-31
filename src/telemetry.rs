//! OpenTelemetry telemetry helpers.

use opentelemetry::global;
use tracing_opentelemetry::OpenTelemetrySpanExt;

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
        .map_err(|e| tracing::warn!("Failed to serialize span context: {e}"))
        .ok()
}

/// Adds a span link to the given tracing span
/// from a serialized OpenTelemetry span context string, if valid.
pub fn add_link_from_serialized_context(span: &tracing::Span, span_context: Option<&str>) {
    let Some(span_ctx_str) = span_context else {
        return;
    };

    match deserialize_span_context(span_ctx_str) {
        Ok(parent_ctx) => {
            let remote_span_ctx = opentelemetry::trace::TraceContextExt::span(&parent_ctx)
                .span_context()
                .clone();
            if remote_span_ctx.is_valid() {
                tracing_opentelemetry::OpenTelemetrySpanExt::add_link(span, remote_span_ctx);
            }
        }
        Err(e) => {
            tracing::warn!("Failed to deserialize span context: {e}");
        }
    }
}

/// Deserializes a JSON string into an OpenTelemetry context.
fn deserialize_span_context(s: &str) -> Result<opentelemetry::Context, serde_json::Error> {
    let map: std::collections::HashMap<String, String> = serde_json::from_str(s)?;
    let context = global::get_text_map_propagator(|propagator| propagator.extract(&map));

    Ok(context)
}
