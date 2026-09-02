//! Repository access for the `branches` table.

use crate::model::Branch;
use crate::repository::RepositoryError;
use async_trait::async_trait;

/// Interface for `branches` table operations.
#[async_trait]
pub trait BranchRepository: Send + Sync {
    /// Returns all branches.
    async fn branches_get_all(&self) -> Result<Vec<Branch>, RepositoryError>;

    /// Updates the last commit hash of the branch.
    async fn branches_update_last_commit_hash(
        &self,
        id: i64,
        hash: &crate::domain::CommitHash,
        tx: &mut sqlx::SqliteConnection,
    ) -> Result<(), RepositoryError>;
}
