//! `TriggerRepository` implementation for SQLite.

use async_trait::async_trait;

use super::SqliteRepository;
use crate::model::TriggerQueueItem;
use crate::repository::{
    RepositoryError,
    trigger::{TriggerRepository, UpdateRetryStatus},
};

#[async_trait]
impl TriggerRepository for SqliteRepository {
    #[tracing::instrument(skip_all, fields(otel.kind = "client", id = %id))]
    async fn trigger_queue_delete(&self, id: i64) -> Result<(), RepositoryError> {
        sqlx::query!("DELETE FROM trigger_queue WHERE id = ?", id)
            .execute(&self.pool)
            .await
            .map_err(RepositoryError::Database)?;
        Ok(())
    }

    #[tracing::instrument(skip_all, fields(otel.kind = "client"))]
    async fn trigger_queue_process_oldest_pending(
        &self,
    ) -> Result<Option<TriggerQueueItem>, RepositoryError> {
        let trigger = sqlx::query_as::<_, TriggerQueueItem>(
            "UPDATE trigger_queue
             SET status = 'PROCESSING', status_updated_at = CURRENT_TIMESTAMP
             WHERE id = (
                 SELECT id FROM trigger_queue
                 WHERE status IN ('PENDING') AND next_retry_at <= CURRENT_TIMESTAMP
                 ORDER BY next_retry_at ASC LIMIT 1
             )
             RETURNING id, branch_id, new_hash, retry_count, target_repo, event_type, gh_app_installation_id, span_context",
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(RepositoryError::Database)?;

        Ok(trigger)
    }

    #[tracing::instrument(
        skip_all,
        fields(otel.kind = "client", id = %params.id, retry_count = %params.retry_count)
    )]
    async fn trigger_queue_update_retry_status(
        &self,
        params: UpdateRetryStatus,
    ) -> Result<(), RepositoryError> {
        let next_retry_count = params.retry_count + 1;

        if next_retry_count as u32 >= params.max_attempts {
            sqlx::query!(
                "UPDATE trigger_queue SET status = 'FAILED', retry_count = ? WHERE id = ?",
                next_retry_count,
                params.id
            )
            .execute(&self.pool)
            .await
            .map_err(RepositoryError::Database)?;
        } else {
            let backoff_secs = (params.backoff_base_secs * (1 << (next_retry_count - 1))) as i64;
            sqlx::query!(
                "UPDATE trigger_queue SET status = 'PENDING', retry_count = ?, next_retry_at = datetime('now', ? || ' seconds') WHERE id = ?",
                next_retry_count,
                backoff_secs,
                params.id
            )
            .execute(&self.pool)
            .await
            .map_err(RepositoryError::Database)?;
        }
        Ok(())
    }

    #[tracing::instrument(
        skip_all,
        fields(otel.kind = "client", threshold_seconds = %threshold_seconds)
    )]
    async fn trigger_queue_recover_stuck_tasks(
        &self,
        threshold_seconds: u64,
    ) -> Result<(), RepositoryError> {
        let threshold_str = format!("-{} seconds", threshold_seconds);

        sqlx::query!(
            "UPDATE trigger_queue
             SET status = 'PENDING', status_updated_at = CURRENT_TIMESTAMP
             WHERE status = 'PROCESSING'
               AND status_updated_at < DATETIME('now', ?)",
            threshold_str
        )
        .execute(&self.pool)
        .await
        .map_err(RepositoryError::Database)?;
        Ok(())
    }

    #[tracing::instrument(
        skip_all,
        fields(otel.kind = "client", branch_id = %params.branch_id)
    )]
    async fn trigger_queue_upsert(
        &self,
        params: crate::repository::trigger::TriggerQueueUpsertParams<'_>,
        executor: &mut sqlx::SqliteConnection,
    ) -> Result<(), RepositoryError> {
        let branch_id = params.branch_id;
        let new_hash = params.new_hash;
        let span_context = params.span_context;
        sqlx::query!(
            "INSERT INTO trigger_queue (branch_id, new_hash, target_repo, event_type, gh_app_installation_id, span_context)
             SELECT ?, ?, s.target_repo, s.event_type, s.gh_app_installation_id, ?
             FROM subscriptions s
             WHERE s.branch_id = ?
             ON CONFLICT(target_repo, event_type) WHERE status = 'PENDING'
             DO UPDATE SET branch_id = excluded.branch_id, new_hash = excluded.new_hash, span_context = excluded.span_context, status_updated_at = CURRENT_TIMESTAMP",
            branch_id,
            new_hash,
            span_context,
            branch_id
        )
        .execute(executor)
        .await
        .map_err(RepositoryError::Database)?;
        Ok(())
    }
}
