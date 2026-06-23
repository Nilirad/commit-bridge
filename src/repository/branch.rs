use crate::model::Branch;
use crate::repository::RepositoryError;
use async_trait::async_trait;

#[async_trait]
pub trait BranchRepository: Send + Sync {
    async fn get_all(&self) -> Result<Vec<Branch>, RepositoryError>;
    async fn find_by_id(&self, id: i64) -> Result<Option<Branch>, RepositoryError>;
    async fn delete_by_id(&self, id: i64) -> Result<u64, RepositoryError>;
}
