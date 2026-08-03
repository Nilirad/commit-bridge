#![doc = include_str!("../README.md")]
#![warn(missing_docs, clippy::missing_docs_in_private_items)]
#![warn(
    clippy::panic,
    clippy::expect_used,
    clippy::todo,
    clippy::unimplemented,
    clippy::indexing_slicing
)]

use commit_bridge::run_app;
use tokio_util::{sync::CancellationToken, task::TaskTracker};
use tracing::{error, info};

#[tokio::main]
async fn main() {
    let tracker = TaskTracker::new();
    let token = CancellationToken::new();

    match run_app(&tracker, &token).await {
        Ok(_tracer_guard) => {
            token.cancel();
            tracker.close();
            tracker.wait().await;
            info!("All systems terminated. Terminating process.");
        }
        Err(e) => error!("{e}"),
    }
}
