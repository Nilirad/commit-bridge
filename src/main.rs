#![doc = include_str!("../README.md")]
#![warn(missing_docs, clippy::missing_docs_in_private_items)]
#![warn(
    clippy::panic,
    clippy::expect_used,
    clippy::todo,
    clippy::unimplemented,
    clippy::indexing_slicing
)]

#[allow(unused_imports)]
use axum::{
    Router,
    routing::{delete, get, post, put},
};
use reqwest::{Client, StatusCode};
use tokio_util::sync::CancellationToken;
use tower_http::timeout::TimeoutLayer;
use tracing::error;

use crate::{
    config::Config,
    context::SharedContext,
    engine::AsyncEngine,
    error::{ClientCreationError, FatalError},
    handler::{
        create_subscriber, delete_subscriber, get_subscriber, list_subscribers, update_subscriber,
    },
    polling::PollingEngine,
    state::AppState,
    trigger::{GitHubAuthenticator, TriggerEngine, get_auth_credentials},
};

/// Server configuration module.
mod config;
mod context;
mod engine;
mod error;
mod handler;
mod model;
mod polling;
mod state;
#[cfg(test)]
mod test_utils;
mod trigger;

#[tokio::main]
async fn main() {
    run_app().await.unwrap_or_else(|e| error!("{e}"));
}

/// A task for an engine to be started.
type EngineTask = (Box<dyn AsyncEngine>, &'static str);

/// Runs the server, delegating errors to the caller.
async fn run_app() -> Result<(), FatalError> {
    tracing_subscriber::fmt::init();

    let config = Config::default();
    let pool = init_database(&config).await?;
    let http_client = build_http_client(&config)?;

    let ctx = init_context(pool.clone(), config.clone());

    crate::trigger::recover_stuck_tasks(&pool, &config).await?;

    let engines = init_engines(&ctx, http_client)?;
    for (engine, message) in engines {
        crate::engine::start_engine(engine, message);
    }

    let app = build_router(pool, &config);

    run_server(app, ctx.token.clone(), &ctx.config).await
}

/// Initializes the database pool.
async fn init_database(config: &Config) -> Result<sqlx::SqlitePool, FatalError> {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .acquire_timeout(config.database_timeout)
        .connect(&config.database_url)
        .await?;
    Ok(pool)
}

/// Initializes the shared application context.
fn init_context(pool: sqlx::SqlitePool, config: Config) -> SharedContext {
    let token = CancellationToken::new();
    SharedContext {
        config: config.clone(),
        db_pool: pool,
        token,
        github_api_base_url: config.github_api_base_url,
        git_fetcher: std::sync::Arc::new(crate::polling::git::MainGitFetcher),
    }
}

/// Initializes the background engines.
fn init_engines(ctx: &SharedContext, http_client: Client) -> Result<Vec<EngineTask>, FatalError> {
    let polling_engine = PollingEngine { ctx: ctx.clone() };

    let authenticator = Box::new(GitHubAuthenticator {
        credentials: get_auth_credentials()?,
        http_client: http_client.clone(),
        config: ctx.config.clone(),
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

/// Builds the application router.
fn build_router(pool: sqlx::SqlitePool, config: &Config) -> Router {
    let state = AppState { db_pool: pool };
    Router::new()
        .route("/health", get(|| async { "Relay Server is alive" }))
        .route(
            "/subscribers",
            post(create_subscriber).get(list_subscribers),
        )
        .route(
            "/subscribers/:id",
            get(get_subscriber)
                .put(update_subscriber)
                .delete(delete_subscriber),
        )
        .with_state(state)
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            config.incoming_http_timeout,
        ))
}

/// Runs the server.
async fn run_server(
    app: Router,
    token: CancellationToken,
    config: &Config,
) -> Result<(), FatalError> {
    let listener = tokio::net::TcpListener::bind(&config.server_address)
        .await
        .map_err(FatalError::TcpBinding)?;
    println!("Server listening on http://{}", config.server_address);
    axum::serve(listener, app)
        .await
        .map_err(FatalError::Serve)?;

    token.cancel();

    Ok(())
}

/// Creates a new HTTP client.
pub fn build_http_client(config: &Config) -> Result<Client, ClientCreationError> {
    let client = Client::builder()
        .user_agent(&config.user_agent)
        .timeout(config.outgoing_http_timeout)
        .build()?;

    Ok(client)
}
