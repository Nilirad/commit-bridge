#![doc = include_str!("../README.md")]
#![warn(missing_docs, clippy::missing_docs_in_private_items)]
#![warn(
    clippy::panic,
    clippy::expect_used,
    clippy::todo,
    clippy::unimplemented,
    clippy::indexing_slicing,
    clippy::undocumented_unsafe_blocks
)]

use std::fs;

use jsonwebtoken::EncodingKey;
use reqwest::Client;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;

use crate::{
    config::Config,
    context::SharedContext,
    engine::AsyncEngine,
    error::FatalError,
    polling::PollingEngine,
    trigger::{GitHubAuthenticator, TriggerEngine},
};

/// Server configuration module.
pub mod config;
pub mod context;
pub mod domain;
pub mod engine;
pub mod error;
pub mod http;
pub mod model;
pub mod polling;
pub mod repository;
pub mod telemetry;
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
    let repository =
        std::sync::Arc::new(crate::repository::SqliteRepository::connect(&config.database).await?);
    let http_client = crate::http::build_http_client(&config)?;

    let ctx = init_context(repository.clone(), config.clone(), token.clone())?;

    crate::trigger::recover_stuck_tasks(&repository, &config)
        .await
        .map_err(FatalError::Repository)?;

    let engines = init_engines(&ctx, http_client)?;
    for (engine, message) in engines {
        crate::engine::start_engine(engine, message, tracker);
    }

    let app = crate::http::router::build_router(repository, &config);

    crate::http::server::run_server(app, &ctx.config, token.clone()).await?;

    Ok(())
}

/// Logs the outcome of the `.env` file load.
///
/// Must only be called after the tracing subscriber is initialized.
pub fn log_dotenv_status(loaded: bool) {
    if !loaded {
        return;
    }

    #[cfg(debug_assertions)]
    tracing::info!("Successfully loaded local `.env` file.");

    #[cfg(not(debug_assertions))]
    tracing::warn!(
        "Successfully loaded local `.env` file. \
        If this is a production build, \
        environment variables should be set prior to execution."
    );
}

/// Initializes the shared application context.
fn init_context(
    repository: std::sync::Arc<crate::repository::SqliteRepository>,
    config: Config,
    token: CancellationToken,
) -> Result<SharedContext, FatalError> {
    let repo_path = &config.git.repo_path;
    let repo = match gix::open(repo_path) {
        Ok(repo) => repo,
        Err(gix::open::Error::NotARepository { .. }) => gix::init(repo_path).map_err(Box::new)?,
        Err(e) => return Err(crate::error::FatalError::GitOpen(Box::new(e))),
    };
    let git_fetcher =
        crate::polling::git::MainGitFetcher::new(repo, config.server.out_request_timeout);

    Ok(SharedContext {
        config,
        repository,
        token,
        git_fetcher: std::sync::Arc::new(git_fetcher),
    })
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
