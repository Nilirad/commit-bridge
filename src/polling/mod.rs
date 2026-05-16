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
            "INSERT INTO trigger_queue (branch_id, new_hash) VALUES (?, ?)",
            branch_info.branch.id,
            branch_info.latest_hash
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
            _ = tokio::time::sleep(ctx.config.polling_sleep) => {}
            _ = ctx.token.cancelled() => {}
        },
        Err(e) => handle_polling_error(e, ctx).await,
    }
}

#[cfg(test)]
mod tests {
    use crate::context::SharedContext;
    use crate::polling::poll_branches;
    use crate::test_utils::MockGitFetcher;
    use std::sync::Arc;
    use tokio_util::sync::CancellationToken;

    #[tokio::test]
    async fn test_poll_branches_updates_db_and_queues_trigger() {
        let pool = crate::test_utils::create_test_db().await;

        // Insert a branch
        sqlx::query!(
            "INSERT INTO branches (repo_url, name, last_commit_hash) VALUES (?, ?, ?)",
            "repo",
            "main",
            "old-hash"
        )
        .execute(&pool)
        .await
        .unwrap();

        let mock_fetcher = Arc::new(MockGitFetcher {
            hash: "new-hash".to_string(),
        });

        let ctx = SharedContext {
            config: crate::config::Config::default(),
            db_pool: pool.clone(),
            git_fetcher: mock_fetcher,
            token: CancellationToken::new(),
            github_api_base_url: "".to_string(),
        };

        poll_branches(&ctx).await.unwrap();

        // Verify DB update
        let branch = sqlx::query!("SELECT last_commit_hash FROM branches WHERE name = 'main'")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(branch.last_commit_hash, Some("new-hash".to_string()));

        // Verify trigger queued
        let queued_event = sqlx::query!("SELECT branch_id, new_hash FROM trigger_queue")
            .fetch_one(&pool)
            .await
            .unwrap();

        assert_eq!(queued_event.branch_id, 1);
        assert_eq!(queued_event.new_hash, "new-hash");
    }
}
