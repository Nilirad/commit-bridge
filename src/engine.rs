//! Interface definitions for async engines.

use async_trait::async_trait;
use std::sync::Arc;

use tracing::info;

/// Defines the interface of an asynchronous backround running engine.
#[async_trait]
pub trait AsyncEngine: Send + Sync + 'static {
    /// Starts the engine.
    fn start(self: Arc<Self>, message: &str) {
        info!(message);
        tokio::spawn(async move {
            self.loop_function().await;
        });
    }

    async fn loop_function(&self);
}
