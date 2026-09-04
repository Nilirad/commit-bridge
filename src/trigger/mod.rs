//! Asynchronous task to trigger remote repository workflows.

use async_trait::async_trait;
use reqwest::Client;
use tracing::{info, warn};

use crate::{
    context::SharedContext,
    engine::AsyncEngine,
    repository::trigger::TriggerRepository,
    trigger::{error::WorkflowTriggerError, process::process_trigger},
};

mod auth;
mod dispatch;
pub mod error;
mod process;

pub use auth::{Authenticator, GitHubAuthenticator};
pub use dispatch::dispatch_events;
pub use process::recover_stuck_tasks;

/// Runs an asynchronous task
/// that triggers a workflow in a remote repository.
pub struct TriggerEngine {
    /// Shared data for all async engines.
    pub ctx: SharedContext,

    /// HTTP client to make requests to the GitHub API.
    pub http_client: Client,

    /// Authenticates requests to the GitHub API.
    pub authenticator: Box<dyn Authenticator + Send + Sync>,
}

#[async_trait]
impl AsyncEngine for TriggerEngine {
    async fn run(&self) {
        trigger_loop(self).await;
    }
}

/// Controls whether to shut down the trigger engine or process a queued event.
async fn trigger_loop(engine: &TriggerEngine) {
    loop {
        tokio::select! {
            _ = engine.ctx.token.cancelled() => break,
            _ = tokio::time::sleep(engine.ctx.config.engine.trigger_queue_polling_interval) => {
                if let Err(e) = process_queue(engine).await {
                    warn!("Error processing queue: {e}");
                }
            }
        }
    }
    info!("Gracefully shutting down trigger engine");
}

/// Processes a single queued event.
///
/// Only enters an instrumented span when a trigger is actually found,
/// so that empty polling cycles do not produce exported spans.
async fn process_queue(engine: &TriggerEngine) -> Result<(), WorkflowTriggerError> {
    let Some(trigger) = engine
        .ctx
        .repository
        .trigger_queue_process_oldest_pending()
        .await?
    else {
        return Ok(());
    };

    process_trigger(engine, trigger).await
}
