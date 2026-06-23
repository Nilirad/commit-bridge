//! Repository access for the `trigger_queue` table.

use crate::model::TriggerQueueItem;
use crate::repository::RepositoryError;
use async_trait::async_trait;

/// Interface for `trigger_queue` table operations.
#[async_trait]
pub trait TriggerRepository: Send + Sync {
    /// Returns all the trigger queue items.
    async fn get_all(&self) -> Result<Vec<TriggerQueueItem>, RepositoryError>;

    /// Deletes the trigger queue item with the given `id`.
    async fn delete_by_id(&self, id: i64) -> Result<(), RepositoryError>;
}
