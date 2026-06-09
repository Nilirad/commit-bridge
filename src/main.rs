#![doc = include_str!("../README.md")]
#![warn(missing_docs, clippy::missing_docs_in_private_items)]
#![warn(
    clippy::panic,
    clippy::expect_used,
    clippy::todo,
    clippy::unimplemented,
    clippy::indexing_slicing
)]

use relay::run_app;
use tokio_util::task::TaskTracker;
use tracing::{error, info};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    #[cfg(debug_assertions)]
    tracing::warn!("APPLICATION IS RUNNING IN DEBUG MODE.");

    let tracker = TaskTracker::new();

    run_app(&tracker).await.unwrap_or_else(|e| error!("{e}"));

    tracker.close();
    tracker.wait().await;
    info!("All systems terminated. Terminating process.")
}
