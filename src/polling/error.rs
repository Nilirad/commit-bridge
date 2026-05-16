//! Handling for errors specific to the polling engine.

use std::time::Duration;

use thiserror::Error;
use tokio_util::sync::CancellationToken;
use tracing::{error, warn};

/// An error that interrupted a polling loop iteration.
#[derive(Debug, Error)]
pub enum PollingError {
    /// Could not read or write database.
    #[error("Database operation failed: {0}")]
    DatabaseOperation(#[from] sqlx::Error),
}

/// Handles polling engine errors.
pub(super) async fn handle_polling_error(error: PollingError, token: &CancellationToken) {
    match error {
        PollingError::DatabaseOperation(e) => handle_sqlx_error(e, token).await,
    }
}

/// Handles SQLx errors.
async fn handle_sqlx_error(error: sqlx::Error, token: &CancellationToken) {
    // TODO: Make retry cooldown configurable.
    const DB_ERROR_COOLDOWN_SECS: u64 = 5 * 60;

    let critical;
    match error {
        sqlx::Error::Database(e) => {
            if e.is_unique_violation() {
                critical = false;
                warn!("Attempted duplicate insertion of unique value: {e}");
            } else {
                critical = true;
                error!("Database error: {e}");
            }
        }
        sqlx::Error::Io(e) => {
            critical = true;
            error!("Database I/O error: {e}");
        }
        e => {
            critical = true;
            error!("{e}")
        }
    }

    if critical {
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(DB_ERROR_COOLDOWN_SECS)) => {}
            _ = token.cancelled() => {}
        }
    }
}
