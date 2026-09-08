//! Handling for errors specific to the polling engine.

use crate::context::SharedContext;
use crate::domain::ValidationError;
use crate::repository::RepositoryError;

use thiserror::Error;
use tracing::{error, warn};

/// An error that interrupted a polling loop iteration.
#[derive(Debug, Error)]
pub enum PollingError {
    /// Could not read or write database.
    #[error("Database operation failed: {0}")]
    DatabaseOperation(#[from] sqlx::Error),
}

impl From<RepositoryError> for PollingError {
    fn from(error: RepositoryError) -> Self {
        match error {
            RepositoryError::Database(e) => PollingError::DatabaseOperation(e),
            RepositoryError::NotFound => PollingError::DatabaseOperation(sqlx::Error::RowNotFound),
            RepositoryError::Mapping(e) => {
                error!("Data mapping error: {e}");
                PollingError::DatabaseOperation(sqlx::Error::RowNotFound)
            }
        }
    }
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

/// Error in fetching the latest commit hash of a remote branch.
#[derive(Debug, Error)]
pub enum CommitHashError {
    /// Validation error.
    #[error("Validation error: {0}")]
    Validation(#[from] ValidationError),

    /// I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Unexpected failure while fetching the commit hash.
    #[error("Unexpected failure while fetching the commit hash: {0}")]
    UnexpectedStatus(String),

    /// Unexpected output while fetching the commit hash.
    #[error(
        "Unexpected output while fetching the commit hash. Repo: {repo_url}; Branch: {branch}; Output: {stdout}"
    )]
    UnexpectedOutput {
        /// The unexpected output text.
        stdout: String,
        /// The relevant git repository URL.
        repo_url: String,
        /// The relevant git branch.
        branch: String,
    },

    /// Failed to find remote.
    #[error("Failed to find remote: {0}")]
    // Error is boxed because it is very large
    RemoteAt(Box<gix::remote::init::Error>),

    /// Failed to connect to remote.
    #[error("Failed to connect to remote: {0}")]
    // Error is boxed because it is very large
    Connect(Box<gix::remote::connect::Error>),

    /// Failed to map refs.
    #[error("Failed to map refs: {0}")]
    // Error is boxed because it is very large
    RefMap(Box<gix::remote::ref_map::Error>),

    /// Failed to parse refspec.
    #[error("Failed to parse refspec: {0}")]
    RefSpecParse(#[from] gix::refspec::parse::Error),

    /// Git operation failed using gix.
    #[error("Git operation failed: {0}")]
    Git(String),
}

impl From<gix::remote::init::Error> for CommitHashError {
    fn from(e: gix::remote::init::Error) -> Self {
        CommitHashError::RemoteAt(Box::new(e))
    }
}

impl From<gix::remote::connect::Error> for CommitHashError {
    fn from(e: gix::remote::connect::Error) -> Self {
        CommitHashError::Connect(Box::new(e))
    }
}

impl From<gix::remote::ref_map::Error> for CommitHashError {
    fn from(e: gix::remote::ref_map::Error) -> Self {
        CommitHashError::RefMap(Box::new(e))
    }
}
