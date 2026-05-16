//! Interface definitions for async engines.

use async_trait::async_trait;

use tracing::info;

/// Defines the interface of an asynchronous background running engine.
#[async_trait]
pub trait AsyncEngine: Send + Sync + 'static {
    /// The core execution loop of the engine.
    async fn run(&self);
}

/// Starts the engine by spawning it in a new task.
pub fn start_engine(engine: Box<dyn AsyncEngine>, message: &str) {
    info!(message);
    tokio::spawn(async move {
        engine.run().await;
    });
}
