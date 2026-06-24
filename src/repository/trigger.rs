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

/// Interface for `trigger_queue` table operations.
#[async_trait]
pub trait TriggerRepository: Send + Sync {
    /// Returns all the trigger queue items.
    async fn get_all(&self) -> Result<Vec<TriggerQueueItem>, RepositoryError>;

    /// Finds the oldest pending trigger queue item and marks it as processing in a transaction.
    async fn find_oldest_pending_and_mark_processing(
        &self,
    ) -> Result<Option<TriggerQueueItem>, RepositoryError>;

    /// Schedules a retry or marks the trigger as failed if max attempts is reached.
    async fn update_retry_status(&self, params: UpdateRetryStatus) -> Result<(), RepositoryError>;

    /// Recovers tasks that have been stuck in `PROCESSING` for too long.
    async fn recover_stuck_tasks(&self, threshold_seconds: u64) -> Result<(), RepositoryError>;

    /// Deletes the trigger queue item with the given `id`.
    async fn delete_by_id(&self, id: i64) -> Result<(), RepositoryError>;

    /// Queues trigger events for all subscribers of a branch.
    async fn queue_triggers_for_branch(
        &self,
        branch_id: i64,
        new_hash: &crate::domain::CommitHash,
        executor: &mut sqlx::SqliteConnection,
    ) -> Result<(), RepositoryError>;
}
