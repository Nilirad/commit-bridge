//! Repository access for the `trigger_queue` table.

use crate::model::TriggerQueueItem;
use crate::repository::RepositoryError;
use async_trait::async_trait;

/// Parameters for updating a trigger's retry status.
#[derive(Debug, Clone)]
pub struct UpdateRetryStatus {
    /// The unique identifier of the trigger.
    pub id: i64,

    /// The current retry count of the trigger.
    pub retry_count: i64,

    /// The maximum number of attempts allowed.
    pub max_attempts: u32,

    /// The base backoff duration in seconds.
    pub backoff_base_secs: u64,
}

/// Parameters for upserting trigger events for a branch.
#[derive(Debug, Clone)]
pub struct TriggerQueueUpsertParams<'a> {
    /// The unique identifier of the branch.
    pub branch_id: i64,

    /// The new commit hash.
    pub new_hash: &'a crate::domain::CommitHash,

    /// Optional serialized OpenTelemetry span context.
    pub span_context: Option<&'a str>,
}

/// Interface for `trigger_queue` table operations.
#[async_trait]
pub trait TriggerRepository: Send + Sync {
    /// Finds the oldest pending trigger queue item and marks it as processing in a transaction.
    async fn trigger_queue_process_oldest_pending(
        &self,
    ) -> Result<Option<TriggerQueueItem>, RepositoryError>;

    /// Schedules a retry or marks the trigger as failed if max attempts is reached.
    async fn trigger_queue_update_retry_status(
        &self,
        params: UpdateRetryStatus,
    ) -> Result<(), RepositoryError>;

    /// Recovers tasks that have been stuck in `PROCESSING` for too long.
    async fn trigger_queue_recover_stuck_tasks(
        &self,
        threshold_seconds: u64,
    ) -> Result<(), RepositoryError>;

    /// Deletes the trigger queue item with the given `id`.
    async fn trigger_queue_delete(&self, id: i64) -> Result<(), RepositoryError>;

    /// Queues trigger events for all subscriptions of a branch.
    async fn trigger_queue_upsert(
        &self,
        params: TriggerQueueUpsertParams<'_>,
        executor: &mut sqlx::SqliteConnection,
    ) -> Result<(), RepositoryError>;
}
