use crate::model::TriggerQueueItem;
use crate::repository::RepositoryError;
use async_trait::async_trait;

#[async_trait]
pub trait TriggerRepository: Send + Sync {
    async fn get_all(&self) -> Result<Vec<TriggerQueueItem>, RepositoryError>;
    async fn delete_by_id(&self, id: i64) -> Result<(), RepositoryError>;
}
