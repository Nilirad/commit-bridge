//! Repository access for the `subscriptions` table.

use crate::domain::{EventType, TargetRepo};
use crate::model::{CreateSubscription, Subscription, SubscriptionWithBranch, UpdateSubscription};
use crate::repository::RepositoryError;
use async_trait::async_trait;

/// Interface for `subscriptions` table operations.
#[async_trait]
pub trait SubscriptionRepository: Send + Sync {
    /// Creates a new subscription and returns it.
    async fn subscriptions_create(
        &self,
        subscription: &CreateSubscription,
    ) -> Result<SubscriptionWithBranch, RepositoryError>;

    /// Returns the subscription with the given id with its branch information.
    async fn subscriptions_get_by_id_with_branch(
        &self,
        id: i64,
    ) -> Result<Option<SubscriptionWithBranch>, RepositoryError>;

    /// Returns the subscription with the given keys with its branch information.
    async fn subscriptions_get_by_keys_with_branch(
        &self,
        branch_id: i64,
        target_repo: &TargetRepo,
        event_type: &EventType,
    ) -> Result<Option<SubscriptionWithBranch>, RepositoryError>;

    /// Lists some subscriptions.
    ///
    /// `last_id` is the last subscription ID that is going to be excluded,
    /// while `limit` is the number of subscriptions to show.
    async fn subscriptions_list_paginated(
        &self,
        last_id: i64,
        limit: i64,
    ) -> Result<Vec<SubscriptionWithBranch>, RepositoryError>;

    /// Counts the remaining subscriptions after `last_id`.
    async fn subscriptions_count_remaining(&self, last_id: i64) -> Result<i64, RepositoryError>;

    /// Updates the subscription with the given id.
    async fn subscriptions_update(
        &self,
        id: i64,
        subscription: &UpdateSubscription,
    ) -> Result<Subscription, RepositoryError>;

    /// Deletes a subscription and cascades deletion to the associated branch if no other subscriptions exist.
    async fn subscriptions_delete(&self, id: i64) -> Result<(), RepositoryError>;
}
