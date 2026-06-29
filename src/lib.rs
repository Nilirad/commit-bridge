#![doc = include_str!("../README.md")]
#![warn(missing_docs, clippy::missing_docs_in_private_items)]
#![warn(
    clippy::panic,
    clippy::expect_used,
    clippy::todo,
    clippy::unimplemented,
    clippy::indexing_slicing
)]

use std::fs;
use std::str::FromStr;

use axum::{
    Router,
    body::Body,
    extract::State,
    http::{HeaderValue, Request, Response, StatusCode, header},
    middleware::{self, Next},
    response::IntoResponse,
};
use jsonwebtoken::EncodingKey;
use reqwest::Client;
use rovo::Router as RovoRouter;
use rovo::aide::openapi::OpenApi;
use rovo::rovo;
use sqlx::sqlite::SqliteConnectOptions;
use subtle::ConstantTimeEq;
use tokio::signal;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use tower_http::timeout::TimeoutLayer;
use tracing::info;

use crate::{
    config::Config,
    context::SharedContext,
    domain::NonEmptyString,
    engine::AsyncEngine,
    error::{ClientCreationError, FatalError},
    handler::{
        create_subscription, delete_subscription, get_subscription, list_subscriptions,
        update_subscription,
    },
    polling::PollingEngine,
    state::AppState,
    trigger::{GitHubAuthenticator, TriggerEngine},
};

/// Server configuration module.
pub mod config;
pub mod context;
pub mod domain;
pub mod engine;
pub mod error;
pub mod handler;
pub mod model;
pub mod polling;
pub mod repository;
pub mod state;
#[cfg(test)]
mod test_utils;
#[cfg(test)]
mod tests;
pub mod trigger;

/// A task for an engine to be started.
type EngineTask = (Box<dyn AsyncEngine>, &'static str);

/// Runs the server, delegating errors to the caller.
pub async fn run_app(tracker: &TaskTracker, token: &CancellationToken) -> Result<(), FatalError> {
    let config = Config::load()?;
    let pool = init_database(&config).await?;
    let repository = std::sync::Arc::new(crate::repository::SqliteRepository::new(pool.clone()));
    let http_client = build_http_client(&config)?;

    let ctx = init_context(
        repository.clone(),
        pool.clone(),
        config.clone(),
        token.clone(),
    );

    crate::trigger::recover_stuck_tasks(&repository, &config)
        .await
        .map_err(FatalError::Repository)?;

    let engines = init_engines(&ctx, http_client)?;
    for (engine, message) in engines {
        crate::engine::start_engine(engine, message, tracker);
    }

    let app = build_router(repository, pool, &config);

    run_server(app, &ctx.config, token.clone()).await
}

/// Initializes the database pool.
async fn init_database(config: &Config) -> Result<sqlx::SqlitePool, FatalError> {
    let options = SqliteConnectOptions::from_str(config.database.url.as_str())?
        .foreign_keys(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal);

    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .acquire_timeout(config.database.timeout)
        .connect_with(options)
        .await?;

    // Ensures database schema is up to date in all environments.
    sqlx::migrate!().run(&pool).await?;

    Ok(pool)
}

/// Initializes the shared application context.
fn init_context(
    repository: std::sync::Arc<crate::repository::SqliteRepository>,
    pool: sqlx::SqlitePool,
    config: Config,
    token: CancellationToken,
) -> SharedContext {
    SharedContext {
        config,
        repository,
        db_pool: pool,
        token,
        git_fetcher: std::sync::Arc::new(crate::polling::git::MainGitFetcher),
    }
}

/// Initializes the background engines.
fn init_engines(ctx: &SharedContext, http_client: Client) -> Result<Vec<EngineTask>, FatalError> {
    let polling_engine = PollingEngine { ctx: ctx.clone() };

    let pem = fs::read(&ctx.config.auth.pem_path).map_err(FatalError::AuthKeyIo)?;
    let encoding_key = EncodingKey::from_rsa_pem(&pem).map_err(FatalError::AuthKeyLoading)?;

    let authenticator = Box::new(GitHubAuthenticator {
        http_client: http_client.clone(),
        config: ctx.config.clone(),
        encoding_key,
    });
    let trigger_engine = TriggerEngine {
        ctx: ctx.clone(),
        http_client,
        authenticator,
    };

    Ok(vec![
        (Box::new(polling_engine), "Starting polling engine"),
        (Box::new(trigger_engine), "Starting trigger engine"),
    ])
}

/// Middleware to authorize requests with an API key.
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
            return StatusCode::UNAUTHORIZED.into_response();
        }
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

#[allow(missing_docs, clippy::missing_docs_in_private_items)]
mod health_handler {
    use super::*;
    #[rovo]
    pub async fn health_check(State(_state): State<AppState>) -> &'static str {
        "CommitBridge is alive"
    }
}

/// Builds the application router.
pub fn build_router(
    repository: std::sync::Arc<crate::repository::SqliteRepository>,
    pool: sqlx::SqlitePool,
    config: &Config,
) -> Router {
    let state = AppState {
        config: std::sync::Arc::new(config.clone()),
        repository,
        db_pool: pool,
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
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            config.server.in_request_timeout,
        ))
}

/// Runs the server.
async fn run_server(
    app: Router,
    config: &Config,
    token: CancellationToken,
) -> Result<(), FatalError> {
    let listener = tokio::net::TcpListener::bind(config.server.address)
        .await
        .map_err(FatalError::TcpBinding)?;
    println!("Server listening on http://{}", config.server.address);
    println!(
        "Scalar UI available at http://{}/scalar",
        config.server.address
    );

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(token))
        .await
        .map_err(FatalError::Serve)?;

    Ok(())
}

/// Creates a future that resolves when a termination signal is received.
async fn shutdown_signal(token: CancellationToken) {
    let ctrl_c = signal::ctrl_c();

    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut signal) = signal::unix::signal(signal::unix::SignalKind::terminate()) {
            signal.recv().await;
        } else {
            std::future::pending::<()>().await;
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
        _ = token.cancelled() => {},
    }
    info!("Shutdown signal received, initiating graceful shutdown...");
}

/// Creates a new HTTP client.
pub fn build_http_client(config: &Config) -> Result<Client, ClientCreationError> {
    let client = Client::builder()
        .user_agent(config.server.user_agent.to_string())
        .timeout(config.server.out_request_timeout)
        .build()?;

    Ok(client)
}
