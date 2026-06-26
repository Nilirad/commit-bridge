//! SQLite implementation of the repository.

use crate::model::{
    Branch, CreateSubscription, Subscription, SubscriptionWithBranch, TriggerQueueItem,
    UpdateSubscription,
};
use crate::repository::{
    RepositoryError,
    branch::BranchRepository,
    subscription::SubscriptionRepository,
    trigger::{TriggerRepository, UpdateRetryStatus},
};
use async_trait::async_trait;
use futures::future::BoxFuture;
use sqlx::{SqliteConnection, SqlitePool};

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

    /// Runs a closure within a transaction.
    pub async fn run_in_transaction<'a, F, T, E>(&self, f: F) -> Result<T, E>
    where
        F: for<'b> FnOnce(&'b mut SqliteConnection) -> BoxFuture<'b, Result<T, E>> + Send + 'a,
        E: From<sqlx::Error> + Send + 'a,
        T: Send + 'a,
    {
        let mut tx = self.pool.begin().await?;
        let result = f(&mut tx).await?;
        tx.commit().await?;
        Ok(result)
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

    async fn update_last_commit_hash(
        &self,
        id: i64,
        hash: &crate::domain::CommitHash,
    ) -> Result<(), RepositoryError> {
        sqlx::query!(
            "UPDATE branches SET last_commit_hash = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
            hash,
            id
        )
        .execute(&self.pool)
        .await
        .map_err(RepositoryError::Database)?;
        Ok(())
    }

    async fn update_last_commit_hash_in_tx(
        &self,
        id: i64,
        hash: &crate::domain::CommitHash,
        tx: &mut sqlx::SqliteConnection,
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

#[async_trait]
impl SubscriptionRepository for SqliteRepository {
    async fn create(
        &self,
        subscription_payload: &CreateSubscription,
    ) -> Result<SubscriptionWithBranch, RepositoryError> {
        let mut transaction = self.pool.begin().await.map_err(RepositoryError::Database)?;

        let branch_id = sqlx::query_scalar::<_, i64>(
            "INSERT INTO branches (repo_url, name) VALUES (?, ?) \
             ON CONFLICT(repo_url, name) DO UPDATE SET repo_url=excluded.repo_url \
             RETURNING id",
        )
        .bind(&subscription_payload.source_repo_url)
        .bind(&subscription_payload.source_branch_name)
        .fetch_one(&mut *transaction)
        .await
        .map_err(RepositoryError::Database)?;

        let subscription = sqlx::query_as::<_, Subscription>(
            "INSERT INTO subscriptions (branch_id, target_repo, event_type, gh_app_installation_id) VALUES (?, ?, ?, ?) RETURNING *",
        )
        .bind(branch_id)
        .bind(&subscription_payload.target_repo)
        .bind(&subscription_payload.event_type)
        .bind(subscription_payload.gh_app_installation_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(RepositoryError::Database)?;

        transaction
            .commit()
            .await
            .map_err(RepositoryError::Database)?;

        Ok(SubscriptionWithBranch {
            subscription,
            source_branch: crate::model::SourceBranchInfo {
                repo_url: subscription_payload.source_repo_url.clone(),
                name: subscription_payload.source_branch_name.clone(),
            },
        })
    }

    async fn get_by_id(&self, id: i64) -> Result<Option<Subscription>, RepositoryError> {
        sqlx::query_as::<_, Subscription>("SELECT * FROM subscriptions WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(RepositoryError::Database)
    }

    async fn get_by_id_with_branch(
        &self,
        id: i64,
    ) -> Result<Option<SubscriptionWithBranch>, RepositoryError> {
        let row = sqlx::query!(
            "SELECT s.*, b.repo_url as branch_repo_url, b.name as branch_name \
             FROM subscriptions s \
             JOIN branches b ON s.branch_id = b.id \
             WHERE s.id = ?",
            id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(RepositoryError::Database)?;

        match row {
            Some(row) => Ok(Some(SubscriptionWithBranch {
                subscription: Subscription {
                    id: row.id,
                    branch_id: row.branch_id,
                    target_repo: crate::domain::TargetRepo::new(row.target_repo)
                        .map_err(|e| RepositoryError::Mapping(e.to_string()))?,
                    event_type: crate::domain::EventType::new(row.event_type)
                        .map_err(|e| RepositoryError::Mapping(e.to_string()))?,
                    gh_app_installation_id: row.gh_app_installation_id,
                    created_at: row.created_at.and_utc(),
                    updated_at: row.updated_at.and_utc(),
                },
                source_branch: crate::model::SourceBranchInfo {
                    repo_url: crate::domain::RepoUrl::new(row.branch_repo_url)
                        .map_err(|e| RepositoryError::Mapping(e.to_string()))?,
                    name: crate::domain::BranchName::new(row.branch_name)
                        .map_err(|e| RepositoryError::Mapping(e.to_string()))?,
                },
            })),
            None => Ok(None),
        }
    }

    async fn list_paginated(
        &self,
        last_id: i64,
        limit: i64,
    ) -> Result<Vec<Subscription>, RepositoryError> {
        sqlx::query_as::<_, Subscription>(
            "SELECT * FROM subscriptions WHERE id > ? ORDER BY id ASC LIMIT ?",
        )
        .bind(last_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(RepositoryError::Database)
    }

    async fn list_paginated_with_branches(
        &self,
        last_id: i64,
        limit: i64,
    ) -> Result<Vec<SubscriptionWithBranch>, RepositoryError> {
        let rows = sqlx::query!(
            "SELECT s.*, b.repo_url as branch_repo_url, b.name as branch_name \
             FROM subscriptions s \
             JOIN branches b ON s.branch_id = b.id \
             WHERE s.id > ? ORDER BY s.id ASC LIMIT ?",
            last_id,
            limit
        )
        .fetch_all(&self.pool)
        .await
        .map_err(RepositoryError::Database)?;

        let subscriptions: Result<Vec<SubscriptionWithBranch>, RepositoryError> = rows
            .into_iter()
            .map(|row| {
                Ok(SubscriptionWithBranch {
                    subscription: Subscription {
                        id: row.id,
                        branch_id: row.branch_id,
                        target_repo: crate::domain::TargetRepo::new(row.target_repo)
                            .map_err(|e| RepositoryError::Mapping(e.to_string()))?,
                        event_type: crate::domain::EventType::new(row.event_type)
                            .map_err(|e| RepositoryError::Mapping(e.to_string()))?,
                        gh_app_installation_id: row.gh_app_installation_id,
                        created_at: row.created_at.and_utc(),
                        updated_at: row.updated_at.and_utc(),
                    },
                    source_branch: crate::model::SourceBranchInfo {
                        repo_url: crate::domain::RepoUrl::new(row.branch_repo_url)
                            .map_err(|e| RepositoryError::Mapping(e.to_string()))?,
                        name: crate::domain::BranchName::new(row.branch_name)
                            .map_err(|e| RepositoryError::Mapping(e.to_string()))?,
                    },
                })
            })
            .collect();
        subscriptions
    }

    async fn count_remaining(&self, last_id: i64) -> Result<i64, RepositoryError> {
        sqlx::query_scalar!("SELECT COUNT(*) FROM subscriptions WHERE id > ?", last_id)
            .fetch_one(&self.pool)
            .await
            .map_err(RepositoryError::Database)
    }

    async fn update(
        &self,
        id: i64,
        subscription: &UpdateSubscription,
    ) -> Result<Subscription, RepositoryError> {
        let mut query_builder = sqlx::QueryBuilder::new("UPDATE subscriptions SET ");
        let mut separated = query_builder.separated(", ");

        if let Some(target_repo) = &subscription.target_repo {
            separated
                .push("target_repo = ")
                .push_bind_unseparated(target_repo);
        }
        if let Some(event_type) = &subscription.event_type {
            separated
                .push("event_type = ")
                .push_bind_unseparated(event_type);
        }
        if let Some(gh_app_installation_id) = subscription.gh_app_installation_id {
            separated
                .push("gh_app_installation_id = ")
                .push_bind_unseparated(gh_app_installation_id);
        }

        separated.push("updated_at = CURRENT_TIMESTAMP");

        query_builder.push(" WHERE id = ");
        query_builder.push_bind(id);
        query_builder.push(" RETURNING *");

        query_builder
            .build_query_as::<Subscription>()
            .fetch_optional(&self.pool)
            .await
            .map_err(RepositoryError::Database)?
            .ok_or(RepositoryError::NotFound)
    }

    async fn delete(&self, id: i64) -> Result<(), RepositoryError> {
        let result = sqlx::query!("DELETE FROM subscriptions WHERE id = ?", id)
            .execute(&self.pool)
            .await
            .map_err(RepositoryError::Database)?;

        if result.rows_affected() == 0 {
            return Err(RepositoryError::NotFound);
        }
        Ok(())
    }

    async fn get_branch_id_by_subscription_id(
        &self,
        id: i64,
    ) -> Result<Option<i64>, RepositoryError> {
        sqlx::query_scalar!("SELECT branch_id FROM subscriptions WHERE id = ?", id)
            .fetch_optional(&self.pool)
            .await
            .map_err(RepositoryError::Database)
    }

    async fn count_subscriptions_by_branch_id(
        &self,
        branch_id: i64,
    ) -> Result<i64, RepositoryError> {
        sqlx::query_scalar!(
            "SELECT COUNT(*) FROM subscriptions WHERE branch_id = ?",
            branch_id
        )
        .fetch_one(&self.pool)
        .await
        .map_err(RepositoryError::Database)
    }

    async fn delete_subscription_and_cascade(&self, id: i64) -> Result<(), RepositoryError> {
        self.run_in_transaction(|tx| {
            Box::pin(async move {
                let branch_id = sqlx::query_scalar!(
                    "DELETE FROM subscriptions WHERE id = ? RETURNING branch_id",
                    id
                )
                .fetch_optional(&mut *tx)
                .await
                .map_err(RepositoryError::Database)?
                .ok_or(RepositoryError::NotFound)?;

                let remaining_subscriptions = sqlx::query_scalar!(
                    "SELECT COUNT(*) FROM subscriptions WHERE branch_id = ?",
                    branch_id
                )
                .fetch_one(&mut *tx)
                .await
                .map_err(RepositoryError::Database)?;

                if remaining_subscriptions == 0 {
                    sqlx::query!("DELETE FROM branches WHERE id = ?", branch_id)
                        .execute(&mut *tx)
                        .await
                        .map_err(RepositoryError::Database)?;
                }

                Ok(())
            })
        })
        .await
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

    async fn find_oldest_pending_and_mark_processing(
        &self,
    ) -> Result<Option<TriggerQueueItem>, RepositoryError> {
        let trigger = sqlx::query_as::<_, TriggerQueueItem>(
            "UPDATE trigger_queue 
             SET status = 'PROCESSING', status_updated_at = CURRENT_TIMESTAMP 
             WHERE id = (
                 SELECT id FROM trigger_queue
                 WHERE status IN ('PENDING') AND next_retry_at <= CURRENT_TIMESTAMP
                 ORDER BY next_retry_at ASC LIMIT 1
             )
             RETURNING id, branch_id, new_hash, retry_count, target_repo, event_type, gh_app_installation_id",
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(RepositoryError::Database)?;

        Ok(trigger)
    }

    async fn update_retry_status(&self, params: UpdateRetryStatus) -> Result<(), RepositoryError> {
        let next_retry_count = params.retry_count + 1;

        if next_retry_count as u32 >= params.max_attempts {
            sqlx::query!(
                "UPDATE trigger_queue SET status = 'FAILED', retry_count = ? WHERE id = ?",
                next_retry_count,
                params.id
            )
            .execute(&self.pool)
            .await
            .map_err(RepositoryError::Database)?;
        } else {
            let backoff_secs = (params.backoff_base_secs * (1 << (next_retry_count - 1))) as i64;
            sqlx::query!(
                "UPDATE trigger_queue SET status = 'PENDING', retry_count = ?, next_retry_at = datetime('now', ? || ' seconds') WHERE id = ?",
                next_retry_count,
                backoff_secs,
                params.id
            )
            .execute(&self.pool)
            .await
            .map_err(RepositoryError::Database)?;
        }
        Ok(())
    }

    async fn recover_stuck_tasks(&self, threshold_seconds: u64) -> Result<(), RepositoryError> {
        let threshold_str = format!("-{} seconds", threshold_seconds);

        sqlx::query!(
            "UPDATE trigger_queue
             SET status = 'PENDING', status_updated_at = CURRENT_TIMESTAMP
             WHERE status = 'PROCESSING'
               AND status_updated_at < DATETIME('now', ?)",
            threshold_str
        )
        .execute(&self.pool)
        .await
        .map_err(RepositoryError::Database)?;
        Ok(())
    }

    async fn queue_triggers_for_branch(
        &self,
        branch_id: i64,
        new_hash: &crate::domain::CommitHash,
        executor: &mut sqlx::SqliteConnection,
    ) -> Result<(), RepositoryError> {
        sqlx::query!(
            "INSERT INTO trigger_queue (branch_id, new_hash, target_repo, event_type, gh_app_installation_id)
             SELECT ?, ?, s.target_repo, s.event_type, s.gh_app_installation_id
             FROM subscriptions s
             WHERE s.branch_id = ?
             ON CONFLICT(branch_id, target_repo, event_type) WHERE status = 'PENDING'
             DO UPDATE SET new_hash = excluded.new_hash, status_updated_at = CURRENT_TIMESTAMP",
            branch_id,
            new_hash,
            branch_id
        )
        .execute(executor)
        .await
        .map_err(RepositoryError::Database)?;
        Ok(())
    }
}
