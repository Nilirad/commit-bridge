use crate::model::{CreateSubscriber, Subscriber, UpdateSubscriber};
use crate::repository::RepositoryError;
use async_trait::async_trait;

#[async_trait]
pub trait SubscriberRepository: Send + Sync {
    async fn create(&self, subscriber: &CreateSubscriber) -> Result<Subscriber, RepositoryError>;
    async fn get_by_id(&self, id: i64) -> Result<Option<Subscriber>, RepositoryError>;
    async fn list_paginated(
        &self,
        last_id: i64,
        limit: i64,
    ) -> Result<Vec<Subscriber>, RepositoryError>;
    async fn count_remaining(&self, last_id: i64) -> Result<i64, RepositoryError>;
    async fn update(
        &self,
        id: i64,
        subscriber: &UpdateSubscriber,
    ) -> Result<Subscriber, RepositoryError>;
    async fn delete(&self, id: i64) -> Result<(), RepositoryError>;
    async fn get_branch_id_by_subscriber_id(&self, id: i64)
    -> Result<Option<i64>, RepositoryError>;
    async fn count_subscribers_by_branch_id(&self, branch_id: i64) -> Result<i64, RepositoryError>;
    async fn delete_branch_by_id(&self, branch_id: i64) -> Result<(), RepositoryError>;
}
