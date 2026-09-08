//! `SubscriptionRepository` implementation for SQLite.

use async_trait::async_trait;
use chrono::NaiveDateTime;

use super::SqliteRepository;
use crate::domain::{BranchName, EventType, RepoUrl, TargetRepo};
use crate::model::{CreateSubscription, Subscription, SubscriptionWithBranch, UpdateSubscription};
use crate::repository::{RepositoryError, subscription::SubscriptionRepository};

/// A row of the `subscriptions` table joined with its `branches` row.
///
/// Decoded by the `query_as!` macros in `subscriptions_get_*`;
/// keep the fields in sync with those join projections.
struct SubscriptionWithBranchRow {
    /// Subscription primary key.
    id: i64,
    /// Foreign key to the `branches` table.
    branch_id: i64,
    /// Raw `target_repo` column.
    target_repo: String,
    /// Raw `event_type` column.
    event_type: String,
    /// GitHub App installation ID.
    gh_app_installation_id: i64,
    /// Creation timestamp.
    created_at: NaiveDateTime,
    /// Last-update timestamp.
    updated_at: NaiveDateTime,
    /// Repo URL of the source branch, from the join.
    branch_repo_url: String,
    /// Name of the source branch, from the join.
    branch_name: String,
}

impl TryFrom<SubscriptionWithBranchRow> for SubscriptionWithBranch {
    type Error = RepositoryError;

    fn try_from(row: SubscriptionWithBranchRow) -> Result<Self, Self::Error> {
        Ok(Self {
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
    }
}

#[async_trait]
impl SubscriptionRepository for SqliteRepository {
    #[tracing::instrument(skip_all, fields(otel.kind = "client"))]
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

    #[tracing::instrument(skip_all, fields(otel.kind = "client", id = %id))]
    async fn subscriptions_get_by_id_with_branch(
        &self,
        id: i64,
    ) -> Result<Option<SubscriptionWithBranch>, RepositoryError> {
        let row = sqlx::query_as!(
            SubscriptionWithBranchRow,
            "SELECT s.*, b.repo_url as branch_repo_url, b.name as branch_name \
             FROM subscriptions s \
             JOIN branches b ON s.branch_id = b.id \
             WHERE s.id = ?",
            id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(RepositoryError::Database)?;

        row.map(SubscriptionWithBranch::try_from).transpose()
    }

    #[tracing::instrument(
        skip_all,
        fields(
            otel.kind = "client",
            branch_id = %branch_id,
            target_repo = %target_repo,
            event_type = %event_type
        )
    )]
    async fn subscriptions_get_by_keys_with_branch(
        &self,
        branch_id: i64,
        target_repo: &TargetRepo,
        event_type: &EventType,
    ) -> Result<Option<SubscriptionWithBranch>, RepositoryError> {
        let row = sqlx::query_as!(
            SubscriptionWithBranchRow,
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

        row.map(SubscriptionWithBranch::try_from).transpose()
    }

    #[tracing::instrument(
        skip_all,
        fields(otel.kind = "client", last_id = %last_id, limit = %limit)
    )]
    async fn subscriptions_list_paginated(
        &self,
        last_id: i64,
        limit: i64,
    ) -> Result<Vec<SubscriptionWithBranch>, RepositoryError> {
        let rows = sqlx::query_as!(
            SubscriptionWithBranchRow,
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

        rows.into_iter()
            .map(SubscriptionWithBranch::try_from)
            .collect::<Result<Vec<_>, _>>()
    }

    #[tracing::instrument(skip_all, fields(otel.kind = "client", last_id = %last_id))]
    async fn subscriptions_count_remaining(&self, last_id: i64) -> Result<i64, RepositoryError> {
        sqlx::query_scalar!("SELECT COUNT(*) FROM subscriptions WHERE id > ?", last_id)
            .fetch_one(&self.pool)
            .await
            .map_err(RepositoryError::Database)
    }

    #[tracing::instrument(skip_all, fields(otel.kind = "client", id = %id))]
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

    #[tracing::instrument(skip_all, fields(otel.kind = "client", id = %id))]
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
