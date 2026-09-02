//! Axum router wiring and middlewares.

use std::time::Duration;

use axum::{
    Router,
    body::Body,
    extract::{MatchedPath, State},
    http::{HeaderValue, Request, Response, StatusCode, header},
    middleware::{self, Next},
    response::IntoResponse,
};
use rovo::Router as RovoRouter;
use rovo::aide::openapi::OpenApi;
use rovo::rovo;
use subtle::ConstantTimeEq;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::{MakeSpan, OnResponse, TraceLayer};
use tracing::Span;

use crate::{
    config::Config,
    domain::NonEmptyString,
    handler::{
        create_subscription, delete_subscription, get_subscription, list_subscriptions,
        update_subscription,
    },
    state::AppState,
};

/// Middleware to authorize requests with an API key.
///
/// Records the `authenticated` attribute onto the `http.request` span
/// (the span is current while this middleware runs,
/// since it is layered below the `TraceLayer`).
async fn auth_middleware(
    State(state): State<AppState>,
    req: Request<Body>,
    next: Next,
) -> Response<Body> {
    let needs_authentication =
        req.uri().path().starts_with("/subscriptions") && !state.config.auth.allow_unauthenticated;

    if needs_authentication {
        let auth_header = req.headers().get("X-API-KEY").and_then(|v| v.to_str().ok());

        if !verify_api_key(state.config.auth.api_key.as_ref(), auth_header) {
            tracing::Span::current().record("authenticated", false);
            return StatusCode::UNAUTHORIZED.into_response();
        }

        tracing::Span::current().record("authenticated", true);
    }

    next.run(req).await
}

/// Uses constant-time verification to check API key correspondence.
///
/// If the API keys correspond, returns `true`,
/// otherwise `false`.
///
/// ### Cryptographic security
///
/// The function short-circuits if lengths of `expected` and `provided` are unequal.
/// While this allows an attacker to extract the key length,
/// it is order of magnitudes safer than using a simple string equality test,
/// which would allow the attacker to gradually know the exact key
/// over many requests.
pub fn verify_api_key(expected: Option<&NonEmptyString>, provided: Option<&str>) -> bool {
    expected.zip(provided).is_some_and(|(key, header)| {
        let key_bytes = key.as_bytes();
        let header_bytes = header.as_bytes();
        key_bytes.ct_eq(header_bytes).into()
    })
}

/// Middleware to set Cache-Control header.
async fn set_no_cache_header(req: Request<Body>, next: Next) -> Response<Body> {
    let path = req.uri().path().to_string();
    let mut response = next.run(req).await;
    if path.starts_with("/subscriptions") {
        response.headers_mut().insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static("private, no-cache, no-store, must-revalidate, max-age=0"),
        );
    }
    response
}

/// Records the matched route path (`http.route`)
/// onto the current HTTP request span.
///
/// Runs after routing, so the [`MatchedPath`] extension is available.
async fn record_http_route(req: Request<Body>, next: Next) -> Response<Body> {
    if let Some(matched) = req.extensions().get::<MatchedPath>() {
        tracing::Span::current().record("http.route", matched.as_str());
    }
    next.run(req).await
}

#[allow(missing_docs, clippy::missing_docs_in_private_items)]
mod health_handler {
    use super::*;
    #[rovo]
    #[tracing::instrument(skip_all, fields(otel.kind = "internal"))]
    pub async fn health_check(State(_state): State<AppState>) -> &'static str {
        "CommitBridge is alive"
    }
}

/// Span factory for incoming HTTP requests,
/// following OpenTelemetry semantic conventions.
///
/// The span is created within this crate
/// (instead of using the default `tower_http` span factory)
/// so that its attributes follow OpenTelemetry conventions.
#[derive(Clone, Copy)]
struct HttpRequestSpan;

impl<B> MakeSpan<B> for HttpRequestSpan {
    fn make_span(&mut self, request: &Request<B>) -> Span {
        tracing::info_span!(
            "http.request",
            otel.kind = "server",
            http.request.method = %request.method(),
            url.path = %request.uri().path(),
            http.route = tracing::field::Empty,
            http.response.status_code = tracing::field::Empty,
            otel.status_code = tracing::field::Empty,
            error.type = tracing::field::Empty,
            authenticated = tracing::field::Empty,
        )
    }
}

/// Records HTTP response metadata onto the request span,
/// following OpenTelemetry semantic conventions.
///
/// Must be used with [`HttpRequestSpan`],
/// which declares the fields recorded here.
#[derive(Clone, Copy)]
pub(crate) struct HttpRequestOnResponse {
    /// Whether client error responses (4xx)
    /// should be marked as errors in exported traces.
    mark_client_errors: bool,
}

impl HttpRequestOnResponse {
    /// Creates a new [`HttpRequestOnResponse`].
    pub(crate) const fn new(mark_client_errors: bool) -> Self {
        Self { mark_client_errors }
    }

    /// Returns `true` if the response status should be marked as an error
    /// in exported traces.
    ///
    /// Server errors (5xx) are always marked;
    /// client errors (4xx) are only marked when `mark_client_errors` is set.
    pub(crate) fn should_mark_error(&self, status: StatusCode) -> bool {
        status.is_server_error() || (self.mark_client_errors && status.is_client_error())
    }
}

impl<B> OnResponse<B> for HttpRequestOnResponse {
    fn on_response(self, response: &Response<B>, _latency: Duration, span: &Span) {
        span.record("http.response.status_code", response.status().as_u16());
        if self.should_mark_error(response.status()) {
            span.record("otel.status_code", "ERROR");
            span.record("error.type", response.status().as_u16().to_string());
        }
    }
}

/// Builds the application router.
pub fn build_router(
    repository: std::sync::Arc<crate::repository::SqliteRepository>,
    config: &Config,
) -> Router {
    let state = AppState {
        config: std::sync::Arc::new(config.clone()),
        repository,
    };

    let mut api = OpenApi::default();
    api.info.title = "CommitBridge API".to_string();
    api.info.description =
        Some("API for managing repository subscriptions and triggering workflows".to_string());

    let subscriptions = RovoRouter::<AppState>::new()
        .route(
            "/",
            rovo::routing::post(create_subscription).get(list_subscriptions),
        )
        .route(
            "/{id}",
            rovo::routing::get(get_subscription)
                .patch(update_subscription)
                .delete(delete_subscription),
        );

    RovoRouter::<AppState>::new()
        .route("/health", rovo::routing::get(health_handler::health_check))
        .nest("/subscriptions", subscriptions)
        .with_oas(api)
        .with_scalar("/scalar")
        .with_state(state.clone())
        .finish()
        .layer(middleware::from_fn_with_state(state, auth_middleware))
        .layer(middleware::from_fn(set_no_cache_header))
        .layer(middleware::from_fn(record_http_route))
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            config.server.in_request_timeout,
        ))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(HttpRequestSpan)
                .on_response(HttpRequestOnResponse::new(
                    config.telemetry.mark_client_errors_as_error,
                )),
        )
}
