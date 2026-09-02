//! Shared context for background engines.

use crate::config::Config;
use crate::polling::git::GitFetcher;
use crate::repository::SqliteRepository;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

/// Shared dependencies across background engines.
#[derive(Clone)]
pub struct SharedContext {
    /// Configuration
    pub config: Config,

    /// Repository for data access.
    pub repository: Arc<SqliteRepository>,

    /// Token to signal task cancellation.
    pub token: CancellationToken,

    /// Git fetcher for polling.
    pub git_fetcher: Arc<dyn GitFetcher>,
}
