//! `BranchRepository` implementation for SQLite.

use async_trait::async_trait;
use sqlx::SqliteConnection;

use super::SqliteRepository;
use crate::model::Branch;
use crate::repository::{RepositoryError, branch::BranchRepository};

#[async_trait]
impl BranchRepository for SqliteRepository {
    #[tracing::instrument(skip_all, fields(otel.kind = "client"))]
    async fn branches_get_all(&self) -> Result<Vec<Branch>, RepositoryError> {
        sqlx::query_as::<_, Branch>("SELECT * FROM branches")
            .fetch_all(&self.pool)
            .await
            .map_err(RepositoryError::Database)
    }

    #[tracing::instrument(skip_all, fields(otel.kind = "client", id = %id))]
    async fn branches_update_last_commit_hash(
        &self,
        id: i64,
        hash: &crate::domain::CommitHash,
        tx: &mut SqliteConnection,
    ) -> Result<(), RepositoryError> {
        sqlx::query!(
            "UPDATE branches SET last_commit_hash = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
            hash,
            id
        )
        .execute(tx)
        .await
        .map_err(RepositoryError::Database)?;
        Ok(())
    }
}
