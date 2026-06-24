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

    /// Updates the last commit hash of the branch.
    async fn update_last_commit_hash(
        &self,
        id: i64,
        hash: &crate::domain::CommitHash,
    ) -> Result<(), RepositoryError>;

    /// Updates the last commit hash of the branch within a transaction.
    async fn update_last_commit_hash_in_tx(
        &self,
        id: i64,
        hash: &crate::domain::CommitHash,
        tx: &mut sqlx::SqliteConnection,
    ) -> Result<(), RepositoryError>;
}
