#![doc = include_str!("../README.md")]
#![warn(missing_docs, clippy::missing_docs_in_private_items)]
#![warn(
    clippy::panic,
    clippy::expect_used,
    clippy::todo,
    clippy::unimplemented,
    clippy::indexing_slicing
)]

use axum::{
    Router,
    routing::{get, post},
};
use reqwest::Client;
use tokio_util::sync::CancellationToken;
use tracing::error;

use crate::{
    context::SharedContext,
    engine::AsyncEngine,
    error::{ClientCreationError, FatalError},
    handler::create_subscriber,
    polling::PollingEngine,
    state::AppState,
    trigger::{GitHubAuthenticator, TriggerEngine, get_auth_credentials},
};

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

    let pool = init_database().await?;
    let http_client = build_http_client()?;

    let ctx = init_context(pool.clone());

    crate::trigger::recover_stuck_tasks(&pool).await?;

    let engines = init_engines(&ctx, http_client)?;
    for (engine, message) in engines {
        crate::engine::start_engine(engine, message);
    }

    let app = build_router(pool);

    run_server(app, ctx.token.clone()).await
}

/// Initializes the database pool.
async fn init_database() -> Result<sqlx::SqlitePool, FatalError> {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .acquire_timeout(std::time::Duration::from_secs(3))
        .connect("sqlite://relay.db?mode=rwc")
        .await?;
    Ok(pool)
}

/// Initializes the shared application context.
fn init_context(pool: sqlx::SqlitePool) -> SharedContext {
    let token = CancellationToken::new();
    SharedContext {
        db_pool: pool,
        token,
        github_api_base_url: "https://api.github.com".to_string(),
        git_fetcher: std::sync::Arc::new(crate::polling::git::MainGitFetcher),
    }
}

/// Initializes the background engines.
fn init_engines(ctx: &SharedContext, http_client: Client) -> Result<Vec<EngineTask>, FatalError> {
    let polling_engine = PollingEngine { ctx: ctx.clone() };

    let authenticator = Box::new(GitHubAuthenticator {
        credentials: get_auth_credentials()?,
        http_client: http_client.clone(),
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
fn build_router(pool: sqlx::SqlitePool) -> Router {
    let state = AppState { db_pool: pool };
    Router::new()
        .route("/health", get(|| async { "Relay Server is alive" }))
        .route("/subscribers", post(create_subscriber))
        .with_state(state)
}

/// Runs the server.
async fn run_server(app: Router, token: CancellationToken) -> Result<(), FatalError> {
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .map_err(FatalError::TcpBinding)?;
    println!("Server listening on http://0.0.0.0:3000");
    axum::serve(listener, app)
        .await
        .map_err(FatalError::Serve)?;

    token.cancel();

    Ok(())
}

/// Creates a new HTTP client.
pub fn build_http_client() -> Result<Client, ClientCreationError> {
    const USER_AGENT: &str = "nilirad-relay-server";

    let client = Client::builder().user_agent(USER_AGENT).build()?;

    Ok(client)
}
