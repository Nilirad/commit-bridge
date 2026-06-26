//! Repository access for the `subscriptions` table.

use crate::domain::{EventType, TargetRepo};
use crate::model::{CreateSubscription, Subscription, SubscriptionWithBranch, UpdateSubscription};
use crate::repository::RepositoryError;
use async_trait::async_trait;

/// Interface for `subscriptions` table operations.
#[async_trait]
pub trait SubscriptionRepository: Send + Sync {
    /// Creates a new subscription and returns it.
    async fn create(
        &self,
        subscription: &CreateSubscription,
    ) -> Result<SubscriptionWithBranch, RepositoryError>;

    /// Returns the subscription with the given id.
    async fn get_by_id(&self, id: i64) -> Result<Option<Subscription>, RepositoryError>;

    /// Returns the subscription with the given id with its branch information.
    async fn get_by_id_with_branch(
        &self,
        id: i64,
    ) -> Result<Option<SubscriptionWithBranch>, RepositoryError>;

    /// Returns the subscription with the given keys with its branch information.
    async fn get_by_keys_with_branch(
        &self,
        branch_id: i64,
        target_repo: &TargetRepo,
        event_type: &EventType,
    ) -> Result<Option<SubscriptionWithBranch>, RepositoryError>;

    /// Lists some subscriptions.
    ///
    /// `last_id` is the last subscription ID that is going to be excluded,
    /// while `limit` is the number of subscriptions to show.
    async fn list_paginated(
        &self,
        last_id: i64,
        limit: i64,
    ) -> Result<Vec<Subscription>, RepositoryError>;

    /// Lists some subscriptions with their branch information.
    async fn list_paginated_with_branches(
        &self,
        last_id: i64,
        limit: i64,
    ) -> Result<Vec<SubscriptionWithBranch>, RepositoryError>;

    /// Counts the remaining subscriptions after `last_id`.
    async fn count_remaining(&self, last_id: i64) -> Result<i64, RepositoryError>;

    /// Updates the subscription with the given id.
    async fn update(
        &self,
        id: i64,
        subscription: &UpdateSubscription,
    ) -> Result<Subscription, RepositoryError>;

    /// Deletes the subscription with the given id.
    async fn delete(&self, id: i64) -> Result<(), RepositoryError>;

    /// Returns the branch ID associated to the given subscription's `id`.
    async fn get_branch_id_by_subscription_id(
        &self,
        id: i64,
    ) -> Result<Option<i64>, RepositoryError>;

    /// Counts the number of subscriptions associated to the given `branch_id`.
    async fn count_subscriptions_by_branch_id(
        &self,
        branch_id: i64,
    ) -> Result<i64, RepositoryError>;

    /// Deletes a subscription and cascades deletion to the associated branch if no other subscriptions exist.
    async fn delete_subscription_and_cascade(&self, id: i64) -> Result<(), RepositoryError>;
}
