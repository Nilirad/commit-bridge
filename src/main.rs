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
    body::Body,
    extract::State,
    http::{HeaderValue, Request, Response, StatusCode, header},
    middleware::{self, Next},
};
use reqwest::Client;
use rovo::Router as RovoRouter;
use rovo::aide::openapi::OpenApi;
use rovo::rovo;
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
#[cfg(test)]
mod tests;
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
        .acquire_timeout(config.database.timeout)
        .connect(&config.database.url)
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

/// Middleware to set Cache-Control header.
async fn set_no_cache_header(req: Request<Body>, next: Next) -> Response<Body> {
    let path = req.uri().path().to_string();
    let mut response = next.run(req).await;
    if path.starts_with("/subscribers") {
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
        "Relay Server is alive"
    }
}

/// Builds the application router.
pub fn build_router(pool: sqlx::SqlitePool, config: &Config) -> Router {
    let state = AppState { db_pool: pool };

    let mut api = OpenApi::default();
    api.info.title = "Relay API".to_string();
    api.info.description =
        Some("API for managing repository subscribers and triggering workflows".to_string());

    let subscribers = RovoRouter::<AppState>::new()
        .route(
            "/",
            rovo::routing::post(create_subscriber).get(list_subscribers),
        )
        .route(
            "/{id}",
            rovo::routing::get(get_subscriber)
                .patch(update_subscriber)
                .delete(delete_subscriber),
        );

    RovoRouter::<AppState>::new()
        .route("/health", rovo::routing::get(health_handler::health_check))
        .nest("/subscribers", subscribers)
        .with_oas(api)
        .with_scalar("/scalar")
        .with_state(state)
        .finish()
        .layer(middleware::from_fn(set_no_cache_header))
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            config.server.in_request_timeout,
        ))
}

/// Runs the server.
async fn run_server(
    app: Router,
    token: CancellationToken,
    config: &Config,
) -> Result<(), FatalError> {
    let listener = tokio::net::TcpListener::bind(&config.server.address)
        .await
        .map_err(FatalError::TcpBinding)?;
    println!("Server listening on http://{}", config.server.address);
    axum::serve(listener, app)
        .await
        .map_err(FatalError::Serve)?;

    token.cancel();

    Ok(())
}

/// Creates a new HTTP client.
pub fn build_http_client(config: &Config) -> Result<Client, ClientCreationError> {
    let client = Client::builder()
        .user_agent(&config.server.user_agent)
        .timeout(config.server.out_request_timeout)
        .build()?;

    Ok(client)
}
