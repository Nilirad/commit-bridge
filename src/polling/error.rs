//! Handling for errors specific to the polling engine.

use crate::context::SharedContext;

use thiserror::Error;
use tracing::{error, warn};

/// An error that interrupted a polling loop iteration.
#[derive(Debug, Error)]
pub enum PollingError {
    /// Could not read or write database.
    #[error("Database operation failed: {0}")]
    DatabaseOperation(#[from] sqlx::Error),
}

/// Handles polling engine errors.
pub(super) async fn handle_polling_error(error: PollingError, ctx: &SharedContext) {
    match error {
        PollingError::DatabaseOperation(e) => handle_sqlx_error(e, ctx).await,
    }
}

/// Handles SQLx errors.
async fn handle_sqlx_error(error: sqlx::Error, ctx: &SharedContext) {
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
            _ = tokio::time::sleep(ctx.config.database.polling_db_error_cooldown) => {}
            _ = ctx.token.cancelled() => {}
        }
    }
}
