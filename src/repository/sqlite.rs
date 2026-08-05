//! SQLite implementation of the repository.

use crate::domain::{BranchName, EventType, RepoUrl, TargetRepo};
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

    /// Runs a closure within a transaction.
    #[tracing::instrument(skip_all)]
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
    #[tracing::instrument(skip_all)]
    async fn branches_get_all(&self) -> Result<Vec<Branch>, RepositoryError> {
        sqlx::query_as::<_, Branch>("SELECT * FROM branches")
            .fetch_all(&self.pool)
            .await
            .map_err(RepositoryError::Database)
    }

    #[tracing::instrument(skip_all, fields(id = %id))]
    async fn branches_update_last_commit_hash(
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
    #[tracing::instrument(skip_all)]
    async fn subscriptions_create(
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

    #[tracing::instrument(skip_all, fields(id = %id))]
    async fn subscriptions_get_by_id_with_branch(
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

    #[tracing::instrument(
        skip_all,
        fields(branch_id = %branch_id, target_repo = %target_repo, event_type = %event_type)
    )]
    async fn subscriptions_get_by_keys_with_branch(
        &self,
        branch_id: i64,
        target_repo: &TargetRepo,
        event_type: &EventType,
    ) -> Result<Option<SubscriptionWithBranch>, RepositoryError> {
        let row = sqlx::query!(
            "SELECT s.*, b.repo_url as branch_repo_url, b.name as branch_name \
             FROM subscriptions s \
             JOIN branches b ON s.branch_id = b.id \
             WHERE s.branch_id = ? AND s.target_repo = ? AND s.event_type = ?",
            branch_id,
            target_repo,
            event_type
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

    #[tracing::instrument(skip_all, fields(last_id = %last_id, limit = %limit))]
    async fn subscriptions_list_paginated(
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
                        target_repo: TargetRepo::new(row.target_repo)
                            .map_err(|e| RepositoryError::Mapping(e.to_string()))?,
                        event_type: EventType::new(row.event_type)
                            .map_err(|e| RepositoryError::Mapping(e.to_string()))?,
                        gh_app_installation_id: row.gh_app_installation_id,
                        created_at: row.created_at.and_utc(),
                        updated_at: row.updated_at.and_utc(),
                    },
                    source_branch: crate::model::SourceBranchInfo {
                        repo_url: RepoUrl::new(row.branch_repo_url)
                            .map_err(|e| RepositoryError::Mapping(e.to_string()))?,
                        name: BranchName::new(row.branch_name)
                            .map_err(|e| RepositoryError::Mapping(e.to_string()))?,
                    },
                })
            })
            .collect();
        subscriptions
    }

    #[tracing::instrument(skip_all, fields(last_id = %last_id))]
    async fn subscriptions_count_remaining(&self, last_id: i64) -> Result<i64, RepositoryError> {
        sqlx::query_scalar!("SELECT COUNT(*) FROM subscriptions WHERE id > ?", last_id)
            .fetch_one(&self.pool)
            .await
            .map_err(RepositoryError::Database)
    }

    #[tracing::instrument(skip_all, fields(id = %id))]
    async fn subscriptions_update(
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

    #[tracing::instrument(skip_all, fields(id = %id))]
    async fn subscriptions_delete(&self, id: i64) -> Result<(), RepositoryError> {
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
    #[tracing::instrument(skip_all, fields(id = %id))]
    async fn trigger_queue_delete(&self, id: i64) -> Result<(), RepositoryError> {
        sqlx::query!("DELETE FROM trigger_queue WHERE id = ?", id)
            .execute(&self.pool)
            .await
            .map_err(RepositoryError::Database)?;
        Ok(())
    }

    #[tracing::instrument(skip_all)]
    async fn trigger_queue_process_oldest_pending(
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
             RETURNING id, branch_id, new_hash, retry_count, target_repo, event_type, gh_app_installation_id, span_context",
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(RepositoryError::Database)?;

        Ok(trigger)
    }

    #[tracing::instrument(
        skip_all,
        fields(id = %params.id, retry_count = %params.retry_count)
    )]
    async fn trigger_queue_update_retry_status(
        &self,
        params: UpdateRetryStatus,
    ) -> Result<(), RepositoryError> {
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

    #[tracing::instrument(skip_all, fields(threshold_seconds = %threshold_seconds))]
    async fn trigger_queue_recover_stuck_tasks(
        &self,
        threshold_seconds: u64,
    ) -> Result<(), RepositoryError> {
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

    #[tracing::instrument(skip_all, fields(branch_id = %params.branch_id))]
    async fn trigger_queue_upsert(
        &self,
        params: crate::repository::trigger::TriggerQueueUpsertParams<'_>,
        executor: &mut sqlx::SqliteConnection,
    ) -> Result<(), RepositoryError> {
        let branch_id = params.branch_id;
        let new_hash = params.new_hash;
        let span_context = params.span_context;
        sqlx::query!(
            "INSERT INTO trigger_queue (branch_id, new_hash, target_repo, event_type, gh_app_installation_id, span_context)
             SELECT ?, ?, s.target_repo, s.event_type, s.gh_app_installation_id, ?
             FROM subscriptions s
             WHERE s.branch_id = ?
             ON CONFLICT(target_repo, event_type) WHERE status = 'PENDING'
             DO UPDATE SET branch_id = excluded.branch_id, new_hash = excluded.new_hash, span_context = excluded.span_context, status_updated_at = CURRENT_TIMESTAMP",
            branch_id,
            new_hash,
            span_context,
            branch_id
        )
        .execute(executor)
        .await
        .map_err(RepositoryError::Database)?;
        Ok(())
    }
}
