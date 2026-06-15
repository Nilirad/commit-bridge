//! State of the application.

use crate::config::Config;
use crate::domain::NonEmptyString;
use std::sync::Arc;

/// Holds data accessible from each [handler].
///
/// <!-- LINKS -->
/// [handler]: crate::handler
#[derive(Debug, Clone)]
pub struct AppState {
    /// Application configuration.
    pub config: Arc<Config>,

    /// SQLx connection pool for the SQLite database.
    pub db_pool: sqlx::SqlitePool,

    /// Optional API key for authentication.
    pub api_key: Option<NonEmptyString>,

    /// Allow unauthenticated access to the API.
    pub allow_unauthenticated: bool,
}
