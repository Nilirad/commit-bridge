//! Repository access for the `subscribers` table.

use crate::model::{CreateSubscriber, Subscriber, UpdateSubscriber};
use crate::repository::RepositoryError;
use async_trait::async_trait;

/// Interface for `subscribers` table operations.
#[async_trait]
pub trait SubscriberRepository: Send + Sync {
    /// Creates a new subscriber and returns it.
    async fn create(&self, subscriber: &CreateSubscriber) -> Result<Subscriber, RepositoryError>;

    /// Returns the subscriber with the given id.
    async fn get_by_id(&self, id: i64) -> Result<Option<Subscriber>, RepositoryError>;

    /// Lists some subscribers.
    ///
    /// `last_id` is the last subscriber ID that is going to be excluded,
    /// while `limit` is the number of subscribers to show.
    async fn list_paginated(
        &self,
        last_id: i64,
        limit: i64,
    ) -> Result<Vec<Subscriber>, RepositoryError>;

    /// Counts the remaining subscribers after `last_id`.
    async fn count_remaining(&self, last_id: i64) -> Result<i64, RepositoryError>;

    /// Updates the subscriber with the given id.
    async fn update(
        &self,
        id: i64,
        subscriber: &UpdateSubscriber,
    ) -> Result<Subscriber, RepositoryError>;

    /// Deletes the subscriber with the given id.
    async fn delete(&self, id: i64) -> Result<(), RepositoryError>;

    /// Returns the branch ID associated to the given subscriber's `id`.
    async fn get_branch_id_by_subscriber_id(&self, id: i64)
    -> Result<Option<i64>, RepositoryError>;

    /// Counts the number of subscribers associated to the given `branch_id`.
    async fn count_subscribers_by_branch_id(&self, branch_id: i64) -> Result<i64, RepositoryError>;

    /// Deletes a subscriber and cascades deletion to the associated branch if no other subscribers exist.
    async fn delete_subscriber_and_cascade(&self, id: i64) -> Result<(), RepositoryError>;
}
