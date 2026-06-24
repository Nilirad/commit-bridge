//! Shared context for background engines.

use crate::config::Config;
use crate::polling::git::GitFetcher;
use crate::repository::SqliteRepository;
use sqlx::SqlitePool;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

/// Shared dependencies across background engines.
#[derive(Clone)]
pub struct SharedContext {
    /// Configuration
    pub config: Config,

    /// Repository for data access.
    pub repository: Arc<SqliteRepository>,

    /// SQLx connection pool.
    pub db_pool: SqlitePool,

    /// Token to signal task cancellation.
    pub token: CancellationToken,

    /// Git fetcher for polling.
    pub git_fetcher: Arc<dyn GitFetcher>,
}
