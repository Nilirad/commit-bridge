use crate::telemetry::{
    add_link_from_serialized_context, build_resource, deserialize_span_context, env_var_is_truthy,
    otlp_endpoint_is_configured, service_name_is_configured,
};
use opentelemetry::{
    Context,
    trace::{SpanContext, SpanId, TraceContextExt, TraceFlags, TraceId, TraceState},
};
use opentelemetry_sdk::propagation::TraceContextPropagator;
use std::sync::{Mutex, MutexGuard};

const SERVICE_NAME_KEY: opentelemetry::Key = opentelemetry::Key::from_static_str("service.name");

/// Serializes all environment access in the test suite.
///
/// The environment-mutating tests hold this lock for their entire body,
/// covering both the mutation
/// and the reads performed by the functions under test.
/// All environment access in the test binary is confined to these tests,
/// so holding this lock guarantees that
/// no other thread reads or mutates the environment concurrently.
static ENV_LOCK: Mutex<()> = Mutex::new(());

/// Sets an environment variable for the duration of the current test.
///
/// Must only be called from a test that holds [`ENV_LOCK`].
fn set_env(name: &str, value: &str) {
    // SAFETY: Every test that calls this helper
    // (directly or through another helper)
    // holds `ENV_LOCK` for its whole body,
    // and all environment access in the test binary is confined to those tests.
    // No other thread can read or mutate the environment while this call runs.
    unsafe {
        std::env::set_var(name, value);
    }
}

/// Removes an environment variable for the duration of the current test.
///
/// Must only be called from a test that holds [`ENV_LOCK`].
fn remove_env(name: &str) {
    // SAFETY: Every test that calls this helper
    // (directly or through another helper)
    // holds `ENV_LOCK` for its whole body,
    // and all environment access in the test binary is confined to those tests.
    // No other thread can read or mutate the environment while this call runs.
    unsafe {
        std::env::remove_var(name);
    }
}

/// Acquires [`ENV_LOCK`], tolerating poisoning.
fn lock_env() -> MutexGuard<'static, ()> {
    ENV_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn sample_span_context() -> SpanContext {
    SpanContext::new(
        TraceId::from_hex("4bf92f3577b34da6a3ce929d0e0e4736").unwrap(),
        SpanId::from_hex("00f067aa0ba902b7").unwrap(),
        TraceFlags::SAMPLED,
        true,
        TraceState::default(),
    )
}

#[test]
fn test_span_context_serialization_round_trip() {
    opentelemetry::global::set_text_map_propagator(TraceContextPropagator::new());

    let remote_context = Context::new().with_remote_span_context(sample_span_context());
    let mut map = std::collections::HashMap::new();
    opentelemetry::global::get_text_map_propagator(|propagator| {
        propagator.inject_context(&remote_context, &mut map);
    });
    let serialized = serde_json::to_string(&map).unwrap();

    let deserialized = deserialize_span_context(&serialized).unwrap();
    let deserialized_span = deserialized.span();
    let extracted = deserialized_span.span_context();

    let expected = sample_span_context();
    assert_eq!(extracted.trace_id(), expected.trace_id());
    assert_eq!(extracted.span_id(), expected.span_id());
    assert_eq!(extracted.trace_flags(), expected.trace_flags());
    assert!(extracted.is_valid());
}

#[test]
fn test_add_link_from_serialized_context_noop_without_parent() {
    let span = tracing::info_span!("test_span");
    add_link_from_serialized_context(&span, None);
}

#[test]
fn test_add_link_from_serialized_context_noop_on_invalid_json() {
    let span = tracing::info_span!("test_span");
    add_link_from_serialized_context(&span, Some("not valid json {"));
}

#[test]
fn test_service_name_is_configured_respects_env() {
    let _guard = lock_env();
    set_env("OTEL_SERVICE_NAME", "my-service");
    assert!(service_name_is_configured());
    remove_env("OTEL_SERVICE_NAME");

    set_env(
        "OTEL_RESOURCE_ATTRIBUTES",
        "service.name=from-attrs,deployment.environment=dev",
    );
    assert!(service_name_is_configured());
    remove_env("OTEL_RESOURCE_ATTRIBUTES");

    assert!(!service_name_is_configured());
}

#[test]
fn test_service_name_is_configured_ignores_empty_values() {
    let _guard = lock_env();
    set_env("OTEL_SERVICE_NAME", "");
    assert!(!service_name_is_configured());
    remove_env("OTEL_SERVICE_NAME");

    set_env("OTEL_RESOURCE_ATTRIBUTES", "service.name=");
    assert!(!service_name_is_configured());
    remove_env("OTEL_RESOURCE_ATTRIBUTES");
}

#[test]
fn test_build_resource_prefers_env_service_name() {
    let _guard = lock_env();
    set_env("OTEL_SERVICE_NAME", "my-service");
    let resource = build_resource();
    assert_eq!(
        resource
            .get(&SERVICE_NAME_KEY)
            .map(|v| v.to_string())
            .as_deref(),
        Some("my-service")
    );
    remove_env("OTEL_SERVICE_NAME");
}

#[test]
fn test_build_resource_falls_back_to_tracer_name() {
    let _guard = lock_env();
    remove_env("OTEL_SERVICE_NAME");
    remove_env("OTEL_RESOURCE_ATTRIBUTES");
    let resource = build_resource();
    assert_eq!(
        resource
            .get(&SERVICE_NAME_KEY)
            .map(|v| v.to_string())
            .as_deref(),
        Some("commit-bridge")
    );
}

#[test]
fn test_env_var_truthiness() {
    let _guard = lock_env();
    set_env("CBRIDGE_TEST_TRUTHY", "1");
    assert!(env_var_is_truthy("CBRIDGE_TEST_TRUTHY"));
    set_env("CBRIDGE_TEST_TRUTHY", "true");
    assert!(env_var_is_truthy("CBRIDGE_TEST_TRUTHY"));
    set_env("CBRIDGE_TEST_TRUTHY", "TRUE");
    assert!(env_var_is_truthy("CBRIDGE_TEST_TRUTHY"));
    set_env("CBRIDGE_TEST_TRUTHY", "yes");
    assert!(!env_var_is_truthy("CBRIDGE_TEST_TRUTHY"));
    remove_env("CBRIDGE_TEST_TRUTHY");
    assert!(!env_var_is_truthy("CBRIDGE_TEST_TRUTHY"));
}

#[test]
fn test_otlp_endpoint_detection() {
    let _guard = lock_env();
    set_env("OTEL_EXPORTER_OTLP_ENDPOINT", "http://localhost:4317");
    assert!(otlp_endpoint_is_configured());
    remove_env("OTEL_EXPORTER_OTLP_ENDPOINT");

    set_env(
        "OTEL_EXPORTER_OTLP_TRACES_ENDPOINT",
        "http://localhost:4317/v1/traces",
    );
    assert!(otlp_endpoint_is_configured());
    remove_env("OTEL_EXPORTER_OTLP_TRACES_ENDPOINT");

    assert!(!otlp_endpoint_is_configured());
}
