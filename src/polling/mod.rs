//! Asynchronous task to periodically check for updated remote branches.

use async_trait::async_trait;
use futures::{StreamExt, future::BoxFuture, stream};
use tracing::{info, warn};

use crate::{
    context::SharedContext,
    engine::AsyncEngine,
    error::CommitHashError,
    polling::{
        branch::BranchInfo,
        error::{PollingError, handle_polling_error},
    },
    repository::{RepositoryError, branch::BranchRepository, trigger::TriggerRepository},
};

mod branch;
mod error;
pub mod git;

/// Runs an asynchronous task
/// that periodically polls git branches in remote repositories.
pub struct PollingEngine {
    /// Shared data for all async engines.
    pub ctx: SharedContext,
}

#[async_trait]
impl AsyncEngine for PollingEngine {
    async fn run(&self) {
        polling_loop(self.ctx.clone()).await;
    }
}

/// Controls whether to shut down the polling engine or run a polling cycle.
async fn polling_loop(ctx: SharedContext) {
    loop {
        tokio::select! {
            res = poll_branches(&ctx) => {followup_poll(res, &ctx).await}
            _ = ctx.token.cancelled() => break,
        }
    }
    info!("Gracefully shutting down polling engine");
}

/// Polls branches for updates,
/// updates them in the `branches` table,
/// and queues the updates for the [`TriggerEngine`].
///
/// <!-- LINKS -->
/// [`TriggerEngine`]: crate::trigger::TriggerEngine
async fn poll_branches(ctx: &SharedContext) -> Result<(), PollingError> {
    let updated_branches = gather_updated_branches(ctx).await?;
    if updated_branches.is_empty() {
        return Ok(());
    }

    let repository = ctx.repository.clone();
    let shared_branches = std::sync::Arc::new(updated_branches);
    ctx.repository
        .run_in_transaction(|tx| {
            execute_branch_updates(repository.clone(), shared_branches.clone(), tx)
        })
        .await?;

    Ok(())
}

/// Gathers stored branches that need to be updated.
async fn gather_updated_branches(ctx: &SharedContext) -> Result<Vec<BranchInfo>, sqlx::Error> {
    let branches = BranchRepository::get_all(ctx.repository.as_ref())
        .await
        .map_err(|e| match e {
            crate::repository::RepositoryError::Database(e) => e,
            _ => sqlx::Error::RowNotFound,
        })?;

    let branch_results = stream::iter(branches)
        .map(|b| BranchInfo::new(b, ctx.git_fetcher.as_ref()))
        .buffer_unordered(ctx.config.database.polling_db_buffer_size)
        .collect::<Vec<Result<BranchInfo, CommitHashError>>>()
        .await;

    let errs = branch_results.iter().filter_map(|res| res.as_ref().err());
    for e in errs {
        warn!("{e}");
    }

    let updated_branches = branch_results
        .into_iter()
        .filter_map(|res| res.ok())
        .filter(BranchInfo::has_updated)
        .collect();
    Ok(updated_branches)
}

/// Helper function to pin the branch update process.
fn execute_branch_updates<'a>(
    repository: std::sync::Arc<crate::repository::SqliteRepository>,
    shared_branches: std::sync::Arc<Vec<branch::BranchInfo>>,
    tx: &'a mut sqlx::SqliteConnection,
) -> BoxFuture<'a, Result<(), RepositoryError>> {
    Box::pin(process_branches(repository, shared_branches, tx))
}

/// Processes branch updates within a transaction.
async fn process_branches(
    repo: std::sync::Arc<crate::repository::SqliteRepository>,
    shared_branches: std::sync::Arc<Vec<branch::BranchInfo>>,
    tx: &mut sqlx::SqliteConnection,
) -> Result<(), RepositoryError> {
    let branches = std::sync::Arc::clone(&shared_branches);
    for branch_info in branches.iter() {
        repo.update_last_commit_hash_in_tx(branch_info.branch.id, &branch_info.latest_hash, tx)
            .await?;

        info!(
            "New commit detected for branch {}. Hash: {}",
            branch_info.branch.name, branch_info.latest_hash
        );

        repo.queue_triggers_for_branch(branch_info.branch.id, &branch_info.latest_hash, tx)
            .await?;
    }
    Ok(())
}

/// Handles polling results and puts the task to sleep.
async fn followup_poll(res: Result<(), PollingError>, ctx: &SharedContext) {
    match res {
        Ok(_) => tokio::select! {
            _ = tokio::time::sleep(ctx.config.engine.polling_sleep) => {}
            _ = ctx.token.cancelled() => {}
        },
        Err(e) => handle_polling_error(e, ctx).await,
    }
}

#[cfg(test)]
#[allow(
    clippy::panic,
    clippy::expect_used,
    clippy::todo,
    clippy::unimplemented,
    clippy::indexing_slicing
)]
mod tests {
    use crate::context::SharedContext;
    use crate::domain::CommitHash;
    use crate::polling::poll_branches;
    use crate::test_utils::MockGitFetcher;
    use std::sync::Arc;
    use tokio_util::sync::CancellationToken;

    #[tokio::test]
    async fn test_poll_branches_updates_db_and_queues_trigger() {
        let pool = crate::test_utils::create_test_db().await;

        // Insert a branch
        let hash = "a".repeat(40);
        sqlx::query!(
            "INSERT INTO branches (repo_url, name, last_commit_hash) VALUES (?, ?, ?)",
            "https://github.com/owner/repo",
            "main",
            hash
        )
        .execute(&pool)
        .await
        .unwrap();
        // Insert a subscription
        sqlx::query!("INSERT INTO subscriptions (branch_id, target_repo, event_type, gh_app_installation_id) VALUES (?, ?, ?, ?)",
            1,
            "org/target",
            "dispatch",
            1
        )
            .execute(&pool)
            .await
            .unwrap();

        let mock_fetcher = Arc::new(MockGitFetcher {
            hash: CommitHash::new("b".repeat(40)).unwrap(),
        });

        let ctx = SharedContext {
            config: crate::test_utils::create_test_config(),
            repository: std::sync::Arc::new(crate::repository::SqliteRepository::new(pool.clone())),
            db_pool: pool.clone(),
            git_fetcher: mock_fetcher,
            token: CancellationToken::new(),
        };

        poll_branches(&ctx).await.unwrap();

        // Verify DB update
        let branch = sqlx::query!("SELECT last_commit_hash FROM branches WHERE name = 'main'")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(branch.last_commit_hash, Some("b".repeat(40)));

        // Verify trigger queued
        let queued_event = sqlx::query!("SELECT branch_id, new_hash FROM trigger_queue")
            .fetch_one(&pool)
            .await
            .unwrap();

        assert_eq!(queued_event.branch_id, Some(1));
        assert_eq!(queued_event.new_hash, Some("b".repeat(40)));
    }

    #[tokio::test]
    async fn test_coalescing_of_trigger_events() {
        let pool = crate::test_utils::create_test_db().await;

        // Insert a branch
        let hash = "a".repeat(40);
        sqlx::query!(
            "INSERT INTO branches (repo_url, name, last_commit_hash) VALUES (?, ?, ?)",
            "https://github.com/owner/repo",
            "main",
            hash
        )
        .execute(&pool)
        .await
        .unwrap();
        // Insert a subscription
        sqlx::query!("INSERT INTO subscriptions (branch_id, target_repo, event_type, gh_app_installation_id) VALUES (?, ?, ?, ?)",
            1,
            "org/target",
            "dispatch",
            1
        )
            .execute(&pool)
            .await
            .unwrap();

        let ctx = SharedContext {
            config: crate::test_utils::create_test_config(),
            repository: std::sync::Arc::new(crate::repository::SqliteRepository::new(pool.clone())),
            db_pool: pool.clone(),
            git_fetcher: Arc::new(crate::test_utils::MockGitFetcher {
                hash: CommitHash::new("b".repeat(40)).unwrap(),
            }),
            token: CancellationToken::new(),
        };

        // First update
        poll_branches(&ctx).await.unwrap();

        // Second update (coalescing)
        // Manually update the mock fetcher to a new hash
        let mock_fetcher = Arc::new(crate::test_utils::MockGitFetcher {
            hash: CommitHash::new("c".repeat(40)).unwrap(),
        });
        let ctx = SharedContext {
            config: ctx.config,
            repository: std::sync::Arc::new(crate::repository::SqliteRepository::new(pool.clone())),
            db_pool: pool.clone(),
            git_fetcher: mock_fetcher,
            token: ctx.token,
        };
        poll_branches(&ctx).await.unwrap();

        // Verify only one entry in queue
        let queued_events = sqlx::query!("SELECT branch_id, new_hash FROM trigger_queue")
            .fetch_all(&pool)
            .await
            .unwrap();

        assert_eq!(queued_events.len(), 1);
        assert_eq!(queued_events[0].branch_id, Some(1));
        assert_eq!(queued_events[0].new_hash, Some("c".repeat(40)));
    }
}
