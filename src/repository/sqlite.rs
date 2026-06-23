//! SQLite implementation of the repository.

use crate::model::{Branch, CreateSubscriber, Subscriber, TriggerQueueItem, UpdateSubscriber};
use crate::repository::{
    RepositoryError, branch::BranchRepository, subscriber::SubscriberRepository,
    trigger::TriggerRepository,
};
use async_trait::async_trait;
use sqlx::SqlitePool;

#[derive(Debug)]
/// Access point of the repository using a SQLite connection pool.
pub struct SqliteRepository {
    /// The SQLite connection pool to the database.
    pool: SqlitePool,
}

impl SqliteRepository {
    /// Creates a new [`SqliteRepository`] from a [`SqlitePool`].
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Returns the stored [`SqlitePool`].
    pub fn get_pool(&self) -> &SqlitePool {
        &self.pool
    }
}

#[async_trait]
impl BranchRepository for SqliteRepository {
    async fn get_all(&self) -> Result<Vec<Branch>, RepositoryError> {
        sqlx::query_as::<_, Branch>("SELECT * FROM branches")
            .fetch_all(&self.pool)
            .await
            .map_err(RepositoryError::Database)
    }

    async fn find_by_id(&self, id: i64) -> Result<Option<Branch>, RepositoryError> {
        sqlx::query_as::<_, Branch>("SELECT * FROM branches WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(RepositoryError::Database)
    }

    async fn delete_by_id(&self, id: i64) -> Result<(), RepositoryError> {
        let result = sqlx::query!("DELETE FROM branches WHERE id = ?", id)
            .execute(&self.pool)
            .await
            .map_err(RepositoryError::Database)?;

        if result.rows_affected() == 0 {
            return Err(RepositoryError::NotFound);
        }
        Ok(())
    }
}

#[async_trait]
impl SubscriberRepository for SqliteRepository {
    async fn create(&self, subscriber: &CreateSubscriber) -> Result<Subscriber, RepositoryError> {
        let mut transaction = self.pool.begin().await.map_err(RepositoryError::Database)?;

        let branch_id = sqlx::query_scalar::<_, i64>(
            "INSERT INTO branches (repo_url, name) VALUES (?, ?) \
             ON CONFLICT(repo_url, name) DO UPDATE SET repo_url=excluded.repo_url \
             RETURNING id",
        )
        .bind(&subscriber.source_repo_url)
        .bind(&subscriber.source_branch_name)
        .fetch_one(&mut *transaction)
        .await
        .map_err(RepositoryError::Database)?;

        let subscriber = sqlx::query_as::<_, Subscriber>(
            "INSERT INTO subscribers (branch_id, target_repo, event_type, gh_app_installation_id) VALUES (?, ?, ?, ?) RETURNING *",
        )
        .bind(branch_id)
        .bind(&subscriber.target_repo)
        .bind(&subscriber.event_type)
        .bind(subscriber.gh_app_installation_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(RepositoryError::Database)?;

        transaction
            .commit()
            .await
            .map_err(RepositoryError::Database)?;
        Ok(subscriber)
    }

    async fn get_by_id(&self, id: i64) -> Result<Option<Subscriber>, RepositoryError> {
        sqlx::query_as::<_, Subscriber>("SELECT * FROM subscribers WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(RepositoryError::Database)
    }

    async fn list_paginated(
        &self,
        last_id: i64,
        limit: i64,
    ) -> Result<Vec<Subscriber>, RepositoryError> {
        sqlx::query_as::<_, Subscriber>(
            "SELECT * FROM subscribers WHERE id > ? ORDER BY id ASC LIMIT ?",
        )
        .bind(last_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(RepositoryError::Database)
    }

    async fn count_remaining(&self, last_id: i64) -> Result<i64, RepositoryError> {
        sqlx::query_scalar!("SELECT COUNT(*) FROM subscribers WHERE id > ?", last_id)
            .fetch_one(&self.pool)
            .await
            .map_err(RepositoryError::Database)
    }

    async fn update(
        &self,
        id: i64,
        subscriber: &UpdateSubscriber,
    ) -> Result<Subscriber, RepositoryError> {
        let mut query_builder = sqlx::QueryBuilder::new("UPDATE subscribers SET ");
        let mut separated = query_builder.separated(", ");

        if let Some(target_repo) = &subscriber.target_repo {
            separated
                .push("target_repo = ")
                .push_bind_unseparated(target_repo);
        }
        if let Some(event_type) = &subscriber.event_type {
            separated
                .push("event_type = ")
                .push_bind_unseparated(event_type);
        }
        if let Some(gh_app_installation_id) = subscriber.gh_app_installation_id {
            separated
                .push("gh_app_installation_id = ")
                .push_bind_unseparated(gh_app_installation_id);
        }

        separated.push("updated_at = CURRENT_TIMESTAMP");

        query_builder.push(" WHERE id = ");
        query_builder.push_bind(id);
        query_builder.push(" RETURNING *");

        query_builder
            .build_query_as::<Subscriber>()
            .fetch_optional(&self.pool)
            .await
            .map_err(RepositoryError::Database)?
            .ok_or(RepositoryError::NotFound)
    }

    async fn delete(&self, id: i64) -> Result<(), RepositoryError> {
        let result = sqlx::query!("DELETE FROM subscribers WHERE id = ?", id)
            .execute(&self.pool)
            .await
            .map_err(RepositoryError::Database)?;

        if result.rows_affected() == 0 {
            return Err(RepositoryError::NotFound);
        }
        Ok(())
    }

    async fn get_branch_id_by_subscriber_id(
        &self,
        id: i64,
    ) -> Result<Option<i64>, RepositoryError> {
        sqlx::query_scalar!("SELECT branch_id FROM subscribers WHERE id = ?", id)
            .fetch_optional(&self.pool)
            .await
            .map_err(RepositoryError::Database)
    }

    async fn count_subscribers_by_branch_id(&self, branch_id: i64) -> Result<i64, RepositoryError> {
        sqlx::query_scalar!(
            "SELECT COUNT(*) FROM subscribers WHERE branch_id = ?",
            branch_id
        )
        .fetch_one(&self.pool)
        .await
        .map_err(RepositoryError::Database)
    }
}

#[async_trait]
impl TriggerRepository for SqliteRepository {
    async fn get_all(&self) -> Result<Vec<TriggerQueueItem>, RepositoryError> {
        sqlx::query_as::<_, TriggerQueueItem>("SELECT * FROM trigger_queue")
            .fetch_all(&self.pool)
            .await
            .map_err(RepositoryError::Database)
    }

    async fn delete_by_id(&self, id: i64) -> Result<(), RepositoryError> {
        sqlx::query!("DELETE FROM trigger_queue WHERE id = ?", id)
            .execute(&self.pool)
            .await
            .map_err(RepositoryError::Database)?;
        Ok(())
    }
}
