//! Repository access for the `branches` table.

use crate::model::Branch;
use crate::repository::RepositoryError;
use async_trait::async_trait;

/// Interface for `branches` table operations.
#[async_trait]
pub trait BranchRepository: Send + Sync {
    /// Returns all branches.
    async fn get_all(&self) -> Result<Vec<Branch>, RepositoryError>;

    /// Returns the branch with the given `id`.
    async fn find_by_id(&self, id: i64) -> Result<Option<Branch>, RepositoryError>;

    /// Deletes the branch with the given `id`.
    async fn delete_by_id(&self, id: i64) -> Result<(), RepositoryError>;
}
