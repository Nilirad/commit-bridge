//! State of the application.

use crate::{config::Config, repository::SqliteRepository};
use std::sync::Arc;

/// Holds data accessible from each [handler].
///
/// <!-- LINKS -->
/// [handler]: crate::handler
#[derive(Debug, Clone)]
pub struct AppState {
    /// Application configuration.
    pub config: Arc<Config>,

    /// Repository for data access.
    pub repository: Arc<SqliteRepository>,

    /// SQLx connection pool for the SQLite database.
    pub db_pool: sqlx::SqlitePool,
}
