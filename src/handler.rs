//! Axum route handlers.

// Needed to bypass a warning
// raised by code generated inside the `#[rovo]` macro,
// used by CRUD submodules.
#![allow(missing_docs, clippy::missing_docs_in_private_items)]

pub mod create;
pub mod delete;
pub mod get;
pub mod list;
pub mod update;

pub use create::create_subscription;
pub use delete::delete_subscription;
pub use get::get_subscription;
pub use list::list_subscriptions;
pub use update::update_subscription;

use crate::model::{HalLink, SubscriptionHal, SubscriptionLinks, SubscriptionWithBranch};

/// Maps a [`SubscriptionWithBranch`] to its HAL representation.
fn map_to_hal(sub_with_branch: SubscriptionWithBranch) -> SubscriptionHal {
    let id = sub_with_branch.subscription.id;
    SubscriptionHal {
        subscription: sub_with_branch.subscription,
        source_branch: sub_with_branch.source_branch,
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

#[cfg(test)]
mod tests {
    #![allow(
        clippy::panic,
        clippy::expect_used,
        clippy::todo,
        clippy::unimplemented,
        clippy::indexing_slicing
    )]

    use super::create::create_subscription_inner;
    use super::delete::delete_subscription_inner;
    use super::get::get_subscription_inner;
    use super::list::{ListSubscriptionsQuery, list_subscriptions_inner};
    use super::update::update_subscription_inner;
    use crate::domain::{BranchName, EventType, RepoUrl, TargetRepo};
    use crate::error::HandlerError;
    use crate::model::{CreateSubscription, UpdateSubscription};
    use crate::state::AppState;
    use crate::test_utils::create_test_db;
    use axum::Json;
    use axum::extract::{Path, Query, State};

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
