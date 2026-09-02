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

use commit_bridge::{log_dotenv_status, run_app, telemetry};
use dotenvy::dotenv;
use tokio_util::{sync::CancellationToken, task::TaskTracker};
use tracing::{error, info};

#[tokio::main]
async fn main() {
    let tracker = TaskTracker::new();
    let token = CancellationToken::new();

    let dotenv_loaded = dotenv().is_ok();
    let tracer_guard = telemetry::init();
    log_dotenv_status(dotenv_loaded);

    let result = run_app(&tracker, &token).await;

    token.cancel();
    tracker.close();
    tracker.wait().await;

    match result {
        Ok(()) => info!("All systems terminated. Terminating process."),
        Err(e) => error!("{e}"),
    }

    drop(tracer_guard);
}
