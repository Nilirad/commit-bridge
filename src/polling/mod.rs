//! Asynchronous task to periodically check for updated remote branches.

use async_trait::async_trait;

use tracing::info;

use crate::{
    context::SharedContext,
    engine::AsyncEngine,
    polling::{
        db::gather_updated_branches,
        error::{PollingError, handle_polling_error},
    },
};

mod branch;
mod db;
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

    let mut transaction = ctx.db_pool.begin().await?;

    for branch_info in &updated_branches {
        crate::polling::db::write_db(branch_info, &mut *transaction).await?;

        info!(
            "New commit detected for branch {}. Hash: {}",
            branch_info.branch.name, branch_info.latest_hash
        );

        sqlx::query!(
            "INSERT INTO trigger_queue (branch_id, new_hash, target_repo, event_type, gh_app_installation_id)
             SELECT ?, ?, s.target_repo, s.event_type, s.gh_app_installation_id
             FROM subscribers s
             WHERE s.branch_id = ?
             ON CONFLICT(branch_id, target_repo, event_type) WHERE status = 'PENDING'
             DO UPDATE SET new_hash = excluded.new_hash, status_updated_at = CURRENT_TIMESTAMP",
            branch_info.branch.id,
            branch_info.latest_hash,
            branch_info.branch.id
        )
        .execute(&mut *transaction)
        .await?;
    }

    transaction.commit().await?;

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
        // Insert a subscriber
        sqlx::query!("INSERT INTO subscribers (branch_id, target_repo, event_type, gh_app_installation_id) VALUES (?, ?, ?, ?)",
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
        // Insert a subscriber
        sqlx::query!("INSERT INTO subscribers (branch_id, target_repo, event_type, gh_app_installation_id) VALUES (?, ?, ?, ?)",
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
