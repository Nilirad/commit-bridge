//! Axum route handlers.

// Needed to bypass a warning raised inside the `#[rovo]` macro.
#![allow(missing_docs, clippy::missing_docs_in_private_items)]

use crate::error::HandlerError;
use crate::model::{
    CreateSubscription, HalLink, Subscription, SubscriptionHal, SubscriptionLinks,
    SubscriptionPage, SubscriptionPageLinks, UpdateSubscription,
};
use crate::repository::subscription::SubscriptionRepository;

use crate::state::AppState;
use axum::{
    Json,
    extract::{Path, Query, State},
};
use rovo::rovo;
use serde::Deserialize;
use tracing::info;

/// Maps a [`Subscription`] to its HAL representation.
fn map_to_hal(subscription: Subscription) -> SubscriptionHal {
    let id = subscription.id;
    SubscriptionHal {
        subscription,
        links: SubscriptionLinks {
            self_link: HalLink {
                href: format!("/subscriptions/{}", id),
            },
            update: HalLink {
                href: format!("/subscriptions/{}", id),
            },
            delete: HalLink {
                href: format!("/subscriptions/{}", id),
            },
        },
    }
}

/// Create a new subscription mapping.
///
/// Creates a new subscription mapping between a source branch and a target repository.
///
/// # Responses
///
/// 201: Json<SubscriptionHal> - Subscription created successfully
///
/// # Metadata
///
/// @tag subscriptions
#[allow(rustdoc::invalid_html_tags)]
#[rovo]
pub async fn create_subscription(
    state: State<AppState>,
    payload: Json<CreateSubscription>,
) -> Result<Json<SubscriptionHal>, HandlerError> {
    create_subscription_inner(state, payload).await
}

/// Internal implementation of [`create_subscription`].
async fn create_subscription_inner(
    State(state): State<AppState>,
    Json(payload): Json<CreateSubscription>,
) -> Result<Json<SubscriptionHal>, HandlerError> {
    let subscription = state.repository.create(&payload).await?;

    info!(
        "Registered new subscription for branch ID {}: {:?}",
        subscription.branch_id, subscription
    );

    Ok(Json(map_to_hal(subscription)))
}

/// Query parameters for listing subscriptions.
#[derive(Debug, Deserialize, rovo::schemars::JsonSchema)]
pub struct ListSubscriptionsQuery {
    /// Maximum number of subscriptions to return.
    pub limit: Option<usize>,
    /// The ID of the last subscription in the previous page.
    pub last_id: Option<i64>,
}

/// List all subscriptions.
///
/// Returns a list of all subscription mappings in the system.
///
/// # Query Parameters
///
/// - `limit`: The maximum number of subscriptions to return (default: 50, max: 100).
/// - `last_id`: The ID of the last subscription in the previous page.
///
/// # Responses
///
/// 200: Json<SubscriptionPage> - Paginated list of subscriptions
///
/// # Metadata
///
/// @tag subscriptions
#[allow(rustdoc::invalid_html_tags)]
#[rovo]
pub async fn list_subscriptions(
    state: State<AppState>,
    query: Query<ListSubscriptionsQuery>,
) -> Result<Json<SubscriptionPage>, HandlerError> {
    list_subscriptions_inner(state, query).await
}

/// Internal implementation of [`list_subscriptions`].
async fn list_subscriptions_inner(
    State(state): State<AppState>,
    Query(query): Query<ListSubscriptionsQuery>,
) -> Result<Json<SubscriptionPage>, HandlerError> {
    let limit = query
        .limit
        .unwrap_or(state.config.database.subscriptions_list_limit)
        .min(state.config.database.subscriptions_list_limit_cap);
    let last_id = query.last_id.unwrap_or_default();

    let subscriptions = state
        .repository
        .list_paginated(last_id, limit as i64)
        .await?;

    let next_id = subscriptions.last().map(|s| s.id).unwrap_or(last_id);
    let remaining_count = state.repository.count_remaining(next_id).await?;

    let next_link = subscriptions
        .last()
        .filter(|_| remaining_count > 0)
        .map(|s| HalLink {
            href: format!("/subscriptions?limit={}&last_id={}", limit, s.id),
        });

    Ok(Json(SubscriptionPage {
        data: subscriptions.into_iter().map(map_to_hal).collect(),
        remaining_count,
        links: SubscriptionPageLinks { next: next_link },
    }))
}

/// Get a single subscription.
///
/// Retrieve a subscription mapping by its ID.
///
/// # Path Parameters
///
/// id: The unique identifier of the subscription
///
/// # Responses
///
/// 200: Json<SubscriptionHal> - Successfully retrieved the subscription
/// 404: () - Subscription was not found
///
/// # Metadata
///
/// @tag subscriptions
#[allow(rustdoc::invalid_html_tags)]
#[rovo]
pub async fn get_subscription(
    state: State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<SubscriptionHal>, HandlerError> {
    get_subscription_inner(state, Path(id)).await
}

/// Internal implementation of [`get_subscription`].
async fn get_subscription_inner(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<SubscriptionHal>, HandlerError> {
    let subscription = state
        .repository
        .get_by_id(id)
        .await?
        .ok_or(HandlerError::NotFound)?;
    Ok(Json(map_to_hal(subscription)))
}

/// Update an existing subscription.
///
/// Updates the target repository, event type, and/or GitHub App installation ID of a subscription.
///
/// # Path Parameters
///
/// id: The unique identifier of the subscription to update
///
/// # Responses
///
/// 200: Json<SubscriptionHal> - Subscription updated successfully
/// 404: () - Subscription was not found
///
/// # Metadata
///
/// @tag subscriptions
#[allow(rustdoc::invalid_html_tags)]
#[rovo]
pub async fn update_subscription(
    state: State<AppState>,
    Path(id): Path<i64>,
    payload: Json<UpdateSubscription>,
) -> Result<Json<SubscriptionHal>, HandlerError> {
    update_subscription_inner(state, Path(id), payload).await
}

/// Internal implementation of [`update_subscription`].
async fn update_subscription_inner(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(payload): Json<UpdateSubscription>,
) -> Result<Json<SubscriptionHal>, HandlerError> {
    let subscription = state.repository.update(id, &payload).await?;

    Ok(Json(map_to_hal(subscription)))
}

/// Delete a subscription.
///
/// Permanently deletes a subscription mapping by its ID.
///
/// # Path Parameters
///
/// id: The unique identifier of the subscription to delete
///
/// # Responses
///
/// 204: () - Subscription deleted successfully
/// 404: () - Subscription was not found
///
/// # Metadata
///
/// @tag subscriptions
#[allow(rustdoc::invalid_html_tags)]
#[rovo]
pub async fn delete_subscription(
    state: State<AppState>,
    Path(id): Path<i64>,
) -> Result<(), HandlerError> {
    delete_subscription_inner(state, Path(id)).await
}

/// Internal implementation of [`delete_subscription`].
async fn delete_subscription_inner(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<(), HandlerError> {
    state.repository.delete_subscription_and_cascade(id).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::panic,
        clippy::expect_used,
        clippy::todo,
        clippy::unimplemented,
        clippy::indexing_slicing
    )]

    use super::*;
    use crate::domain::{BranchName, EventType, RepoUrl, TargetRepo};
    use crate::model::CreateSubscription;
    use crate::state::AppState;
    use crate::test_utils::create_test_db;
    use axum::Json;
    use axum::extract::State;

    #[tokio::test]
    async fn test_crud_subscription() {
        let pool = create_test_db().await;
        let config = crate::test_utils::create_test_config();
        let state = AppState {
            config: std::sync::Arc::new(config),
            repository: std::sync::Arc::new(crate::repository::SqliteRepository::new(pool.clone())),
            db_pool: pool.clone(),
        };
        let payload = CreateSubscription {
            source_repo_url: RepoUrl::new("https://github.com/org/repo".to_string()).unwrap(),
            source_branch_name: BranchName::new("main".to_string()).unwrap(),
            target_repo: TargetRepo::new("org/target".to_string()).unwrap(),
            event_type: EventType::new("dispatch".to_string()).unwrap(),
            gh_app_installation_id: 1,
        };

        // Create
        let res = create_subscription_inner(State(state.clone()), Json(payload))
            .await
            .unwrap();
        let id = res.subscription.id;
        assert_eq!(res.links.self_link.href, format!("/subscriptions/{}", id));

        // List
        let list = list_subscriptions_inner(
            State(state.clone()),
            Query(ListSubscriptionsQuery {
                limit: None,
                last_id: None,
            }),
        )
        .await
        .unwrap();
        assert_eq!(list.data.len(), 1);
        assert_eq!(
            list.data[0].links.self_link.href,
            format!("/subscriptions/{}", id)
        );
        assert_eq!(list.remaining_count, 0);

        // Get
        let get = get_subscription_inner(State(state.clone()), Path(id))
            .await
            .unwrap();
        assert_eq!(get.subscription.id, id);
        assert_eq!(get.links.self_link.href, format!("/subscriptions/{}", id));

        // Update
        let update_payload = UpdateSubscription {
            target_repo: Some(TargetRepo::new("org/new-target".to_string()).unwrap()),
            event_type: None,
            gh_app_installation_id: None,
        };
        let updated =
            update_subscription_inner(State(state.clone()), Path(id), Json(update_payload))
                .await
                .unwrap();
        assert_eq!(
            updated.subscription.target_repo,
            TargetRepo::new("org/new-target".to_string()).unwrap()
        );

        assert_eq!(
            updated.links.self_link.href,
            format!("/subscriptions/{}", id)
        );

        // Delete
        delete_subscription_inner(State(state.clone()), Path(id))
            .await
            .unwrap();

        // Verify delete
        let get_after_delete = get_subscription_inner(State(state.clone()), Path(id)).await;
        assert!(get_after_delete.is_err());
    }

    #[tokio::test]
    async fn test_non_existent_subscription_returns_not_found() {
        let pool = create_test_db().await;
        let config = crate::test_utils::create_test_config();
        let state = AppState {
            config: std::sync::Arc::new(config),
            repository: std::sync::Arc::new(crate::repository::SqliteRepository::new(pool.clone())),
            db_pool: pool.clone(),
        };

        // Try getting a non-existent subscription
        let get_res = get_subscription_inner(State(state.clone()), Path(999)).await;
        assert!(matches!(get_res, Err(HandlerError::NotFound)));

        // Try updating a non-existent subscription
        let update_payload = UpdateSubscription {
            target_repo: Some(TargetRepo::new("org/new-target".to_string()).unwrap()),
            event_type: None,
            gh_app_installation_id: None,
        };
        let update_res =
            update_subscription_inner(State(state.clone()), Path(999), Json(update_payload)).await;
        assert!(matches!(update_res, Err(HandlerError::NotFound)));

        // Try deleting a non-existent subscription
        let delete_res = delete_subscription_inner(State(state.clone()), Path(999)).await;
        assert!(matches!(delete_res, Err(HandlerError::NotFound)));
    }

    #[tokio::test]
    async fn test_list_subscriptions_pagination() {
        let pool = create_test_db().await;
        let config = crate::test_utils::create_test_config();
        let state = AppState {
            config: std::sync::Arc::new(config),
            repository: std::sync::Arc::new(crate::repository::SqliteRepository::new(pool.clone())),
            db_pool: pool.clone(),
        };

        // Create 3 subscriptions
        //
        // Lint needs to be silenced here because the `#[tokio::test]` macro
        // probably interferes with the nesting count.
        #[allow(clippy::excessive_nesting)]
        for i in 0..3 {
            let payload = CreateSubscription {
                source_repo_url: RepoUrl::new(format!("https://github.com/org/repo{}", i)).unwrap(),
                source_branch_name: BranchName::new("main".to_string()).unwrap(),
                target_repo: TargetRepo::new("org/target".to_string()).unwrap(),
                event_type: EventType::new("dispatch".to_string()).unwrap(),
                gh_app_installation_id: 1,
            };
            let _ = create_subscription_inner(State(state.clone()), Json(payload))
                .await
                .unwrap();
        }

        // Fetch first page (limit 2)
        let page1 = list_subscriptions_inner(
            State(state.clone()),
            Query(ListSubscriptionsQuery {
                limit: Some(2),
                last_id: None,
            }),
        )
        .await
        .unwrap();
        assert_eq!(page1.data.len(), 2);
        assert_eq!(page1.remaining_count, 1);
        assert!(page1.links.next.is_some());

        // Fetch second page
        let last_id = page1.data.last().unwrap().subscription.id;
        let page2 = list_subscriptions_inner(
            State(state.clone()),
            Query(ListSubscriptionsQuery {
                limit: Some(2),
                last_id: Some(last_id),
            }),
        )
        .await
        .unwrap();
        assert_eq!(page2.data.len(), 1);
        assert_eq!(page2.remaining_count, 0);
        assert!(page2.links.next.is_none());
    }

    #[tokio::test]
    async fn test_cascading_branch_cleanup() {
        let pool = create_test_db().await;
        let config = crate::test_utils::create_test_config();
        let state = AppState {
            config: std::sync::Arc::new(config),
            repository: std::sync::Arc::new(crate::repository::SqliteRepository::new(pool.clone())),
            db_pool: pool.clone(),
        };
        let payload = CreateSubscription {
            source_repo_url: RepoUrl::new("https://github.com/org/repo".to_string()).unwrap(),
            source_branch_name: BranchName::new("main".to_string()).unwrap(),
            target_repo: TargetRepo::new("org/target".to_string()).unwrap(),
            event_type: EventType::new("dispatch".to_string()).unwrap(),
            gh_app_installation_id: 1,
        };

        // Create two subscriptions for the same branch
        let sub1 = create_subscription_inner(State(state.clone()), Json(payload.clone()))
            .await
            .unwrap();
        let sub2 = create_subscription_inner(State(state.clone()), Json(payload))
            .await
            .unwrap();

        let branch_id = sub1.subscription.branch_id;

        // Verify branch exists
        let branch: Option<(i64,)> = sqlx::query_as("SELECT id FROM branches WHERE id = ?")
            .bind(branch_id)
            .fetch_optional(&pool)
            .await
            .unwrap();
        assert!(branch.is_some());

        // Delete first subscription
        delete_subscription_inner(State(state.clone()), Path(sub1.subscription.id))
            .await
            .unwrap();

        // Branch should still exist
        let branch_still_exists: Option<(i64,)> =
            sqlx::query_as("SELECT id FROM branches WHERE id = ?")
                .bind(branch_id)
                .fetch_optional(&pool)
                .await
                .unwrap();
        assert!(branch_still_exists.is_some());

        // Delete second subscription
        delete_subscription_inner(State(state.clone()), Path(sub2.subscription.id))
            .await
            .unwrap();

        // Branch should be gone
        let branch_gone: Option<(i64,)> = sqlx::query_as("SELECT id FROM branches WHERE id = ?")
            .bind(branch_id)
            .fetch_optional(&pool)
            .await
            .unwrap();
        assert!(branch_gone.is_none());
    }
}
