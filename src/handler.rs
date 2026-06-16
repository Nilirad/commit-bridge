//! Axum route handlers.

// Needed to bypass a warning raised inside the `#[rovo]` macro.
#![allow(missing_docs, clippy::missing_docs_in_private_items)]

use crate::error::HandlerError;
use crate::model::{
    CreateSubscriber, HalLink, Subscriber, SubscriberHal, SubscriberLinks, SubscriberPage,
    SubscriberPageLinks, UpdateSubscriber,
};

use crate::state::AppState;
use axum::{
    Json,
    extract::{Path, Query, State},
};
use rovo::rovo;
use serde::Deserialize;
use tracing::info;

/// Maps a [`Subscriber`] to its HAL representation.
fn map_to_hal(subscriber: Subscriber) -> SubscriberHal {
    let id = subscriber.id;
    SubscriberHal {
        subscriber,
        links: SubscriberLinks {
            self_link: HalLink {
                href: format!("/subscribers/{}", id),
            },
            update: HalLink {
                href: format!("/subscribers/{}", id),
            },
            delete: HalLink {
                href: format!("/subscribers/{}", id),
            },
        },
    }
}

/// Create a new subscriber mapping.
///
/// Creates a new subscription mapping between a source branch and a target repository.
///
/// # Responses
///
/// 201: Json<SubscriberHal> - Subscriber created successfully
///
/// # Metadata
///
/// @tag subscribers
#[allow(rustdoc::invalid_html_tags)]
#[rovo]
pub async fn create_subscriber(
    state: State<AppState>,
    payload: Json<CreateSubscriber>,
) -> Result<Json<SubscriberHal>, HandlerError> {
    create_subscriber_inner(state, payload).await
}

/// Internal implementation of [`create_subscriber`].
async fn create_subscriber_inner(
    State(state): State<AppState>,
    Json(payload): Json<CreateSubscriber>,
) -> Result<Json<SubscriberHal>, HandlerError> {
    let mut transaction = state.db_pool.begin().await?;
    let branch_id = get_or_insert_branch_id(&mut transaction, &payload).await?;
    let subscriber = sqlx::query_as::<_, Subscriber>(
        "INSERT INTO subscribers (branch_id, target_repo, event_type, gh_app_installation_id) VALUES (?, ?, ?, ?) RETURNING *"
    )
    .bind(branch_id)
    .bind(&payload.target_repo)
    .bind(&payload.event_type)
    .bind(payload.gh_app_installation_id)
    .fetch_one(&mut *transaction)
    .await?;
    transaction.commit().await?;

    info!("Registered new subscriber for branch ID {branch_id}: {subscriber:?}");

    Ok(Json(map_to_hal(subscriber)))
}

/// Query parameters for listing subscribers.
#[derive(Debug, Deserialize, rovo::schemars::JsonSchema)]
pub struct ListSubscribersQuery {
    /// Maximum number of subscribers to return.
    pub limit: Option<usize>,
    /// The ID of the last subscriber in the previous page.
    pub last_id: Option<i64>,
}

/// List all subscribers.
///
/// Returns a list of all subscriber mappings in the system.
///
/// # Query Parameters
///
/// - `limit`: The maximum number of subscribers to return (default: 50, max: 100).
/// - `last_id`: The ID of the last subscriber in the previous page.
///
/// # Responses
///
/// 200: Json<SubscriberPage> - Paginated list of subscribers
///
/// # Metadata
///
/// @tag subscribers
#[allow(rustdoc::invalid_html_tags)]
#[rovo]
pub async fn list_subscribers(
    state: State<AppState>,
    query: Query<ListSubscribersQuery>,
) -> Result<Json<SubscriberPage>, HandlerError> {
    list_subscribers_inner(state, query).await
}

/// Internal implementation of [`list_subscribers`].
async fn list_subscribers_inner(
    State(state): State<AppState>,
    Query(query): Query<ListSubscribersQuery>,
) -> Result<Json<SubscriberPage>, HandlerError> {
    let limit = query
        .limit
        .unwrap_or(state.config.database.subscribers_list_limit)
        .min(state.config.database.subscribers_list_limit_cap);
    let last_id = query.last_id.unwrap_or_default();

    let subscribers = sqlx::query_as::<_, Subscriber>(
        "SELECT * FROM subscribers WHERE id > ? ORDER BY id ASC LIMIT ?",
    )
    .bind(last_id)
    .bind(limit as i64)
    .fetch_all(&state.db_pool)
    .await?;

    let remaining_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM subscribers WHERE id > ?")
        .bind(subscribers.last().map(|s| s.id).unwrap_or(last_id))
        .fetch_one(&state.db_pool)
        .await?;

    let next_link = subscribers
        .last()
        .filter(|_| remaining_count > 0)
        .map(|s| HalLink {
            href: format!("/subscribers?limit={}&last_id={}", limit, s.id),
        });

    Ok(Json(SubscriberPage {
        data: subscribers.into_iter().map(map_to_hal).collect(),
        remaining_count,
        links: SubscriberPageLinks { next: next_link },
    }))
}

/// Get a single subscriber.
///
/// Retrieve a subscriber mapping by its ID.
///
/// # Path Parameters
///
/// id: The unique identifier of the subscriber
///
/// # Responses
///
/// 200: Json<SubscriberHal> - Successfully retrieved the subscriber
/// 404: () - Subscriber was not found
///
/// # Metadata
///
/// @tag subscribers
#[allow(rustdoc::invalid_html_tags)]
#[rovo]
pub async fn get_subscriber(
    state: State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<SubscriberHal>, HandlerError> {
    get_subscriber_inner(state, Path(id)).await
}

/// Internal implementation of [`get_subscriber`].
async fn get_subscriber_inner(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<SubscriberHal>, HandlerError> {
    let subscriber = sqlx::query_as::<_, Subscriber>("SELECT * FROM subscribers WHERE id = ?")
        .bind(id)
        .fetch_optional(&state.db_pool)
        .await?
        .ok_or(HandlerError::NotFound)?;
    Ok(Json(map_to_hal(subscriber)))
}

/// Update an existing subscriber.
///
/// Updates the target repository, event type, and/or GitHub App installation ID of a subscriber.
///
/// # Path Parameters
///
/// id: The unique identifier of the subscriber to update
///
/// # Responses
///
/// 200: Json<SubscriberHal> - Subscriber updated successfully
/// 404: () - Subscriber was not found
///
/// # Metadata
///
/// @tag subscribers
#[allow(rustdoc::invalid_html_tags)]
#[rovo]
pub async fn update_subscriber(
    state: State<AppState>,
    Path(id): Path<i64>,
    payload: Json<UpdateSubscriber>,
) -> Result<Json<SubscriberHal>, HandlerError> {
    update_subscriber_inner(state, Path(id), payload).await
}

/// Internal implementation of [`update_subscriber`].
async fn update_subscriber_inner(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(payload): Json<UpdateSubscriber>,
) -> Result<Json<SubscriberHal>, HandlerError> {
    let mut query_builder = sqlx::QueryBuilder::new("UPDATE subscribers SET ");
    let mut separated = query_builder.separated(", ");

    if let Some(target_repo) = &payload.target_repo {
        separated
            .push("target_repo = ")
            .push_bind_unseparated(target_repo);
    }
    if let Some(event_type) = &payload.event_type {
        separated
            .push("event_type = ")
            .push_bind_unseparated(event_type);
    }
    if let Some(gh_app_installation_id) = payload.gh_app_installation_id {
        separated
            .push("gh_app_installation_id = ")
            .push_bind_unseparated(gh_app_installation_id);
    }

    separated.push("updated_at = CURRENT_TIMESTAMP");

    query_builder.push(" WHERE id = ");
    query_builder.push_bind(id);
    query_builder.push(" RETURNING *");

    let subscriber = query_builder
        .build_query_as::<Subscriber>()
        .fetch_optional(&state.db_pool)
        .await?
        .ok_or(HandlerError::NotFound)?;

    Ok(Json(map_to_hal(subscriber)))
}

/// Delete a subscriber.
///
/// Permanently deletes a subscriber mapping by its ID.
///
/// # Path Parameters
///
/// id: The unique identifier of the subscriber to delete
///
/// # Responses
///
/// 204: () - Subscriber deleted successfully
/// 404: () - Subscriber was not found
///
/// # Metadata
///
/// @tag subscribers
#[allow(rustdoc::invalid_html_tags)]
#[rovo]
pub async fn delete_subscriber(
    state: State<AppState>,
    Path(id): Path<i64>,
) -> Result<(), HandlerError> {
    delete_subscriber_inner(state, Path(id)).await
}

/// Internal implementation of [`delete_subscriber`].
async fn delete_subscriber_inner(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<(), HandlerError> {
    let mut transaction = state.db_pool.begin().await?;

    let branch_id: i64 = sqlx::query_scalar("SELECT branch_id FROM subscribers WHERE id = ?")
        .bind(id)
        .fetch_optional(&mut *transaction)
        .await?
        .ok_or(HandlerError::NotFound)?;

    let delete_subscriber_result = sqlx::query("DELETE FROM subscribers WHERE id = ?")
        .bind(id)
        .execute(&mut *transaction)
        .await?;

    if delete_subscriber_result.rows_affected() == 0 {
        return Err(HandlerError::NotFound);
    }

    let remaining_subscribers: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM subscribers WHERE branch_id = ?")
            .bind(branch_id)
            .fetch_one(&mut *transaction)
            .await?;

    if remaining_subscribers == 0 {
        sqlx::query("DELETE FROM branches WHERE id = ?")
            .bind(branch_id)
            .execute(&mut *transaction)
            .await?;
    }

    transaction.commit().await?;
    Ok(())
}

/// Gets the branch ID specified in the [`CreateSubscriber`] payload.
///
/// If the branch doesn't exist, it is created and its ID is returned.
async fn get_or_insert_branch_id(
    transaction: &mut sqlx::SqliteConnection,
    payload: &CreateSubscriber,
) -> Result<i64, HandlerError> {
    let branch_id_opt =
        sqlx::query_scalar::<_, i64>("SELECT id FROM branches WHERE repo_url = ? AND name = ?")
            .bind(&payload.source_repo_url)
            .bind(&payload.source_branch_name)
            .fetch_optional(&mut *transaction)
            .await?;

    if let Some(id) = branch_id_opt {
        return Ok(id);
    }

    sqlx::query_scalar::<_, i64>(
        "INSERT INTO branches (repo_url, name) VALUES (?, ?) \
         ON CONFLICT(repo_url, name) DO UPDATE SET repo_url=excluded.repo_url \
         RETURNING id",
    )
    .bind(&payload.source_repo_url)
    .bind(&payload.source_branch_name)
    .fetch_one(&mut *transaction)
    .await
    .map_err(Into::into)
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
    use crate::model::CreateSubscriber;
    use crate::state::AppState;
    use crate::test_utils::create_test_db;
    use axum::Json;
    use axum::extract::State;

    #[tokio::test]
    async fn test_crud_subscriber() {
        let pool = create_test_db().await;
        let config = crate::test_utils::create_test_config();
        let state = AppState {
            config: std::sync::Arc::new(config),
            db_pool: pool.clone(),
        };
        let payload = CreateSubscriber {
            source_repo_url: RepoUrl::new("https://github.com/org/repo".to_string()).unwrap(),
            source_branch_name: BranchName::new("main".to_string()).unwrap(),
            target_repo: TargetRepo::new("org/target".to_string()).unwrap(),
            event_type: EventType::new("dispatch".to_string()).unwrap(),
            gh_app_installation_id: 1,
        };

        // Create
        let res = create_subscriber_inner(State(state.clone()), Json(payload))
            .await
            .unwrap();
        let id = res.subscriber.id;
        assert_eq!(res.links.self_link.href, format!("/subscribers/{}", id));

        // List
        let list = list_subscribers_inner(
            State(state.clone()),
            Query(ListSubscribersQuery {
                limit: None,
                last_id: None,
            }),
        )
        .await
        .unwrap();
        assert_eq!(list.data.len(), 1);
        assert_eq!(
            list.data[0].links.self_link.href,
            format!("/subscribers/{}", id)
        );
        assert_eq!(list.remaining_count, 0);

        // Get
        let get = get_subscriber_inner(State(state.clone()), Path(id))
            .await
            .unwrap();
        assert_eq!(get.subscriber.id, id);
        assert_eq!(get.links.self_link.href, format!("/subscribers/{}", id));

        // Update
        let update_payload = UpdateSubscriber {
            target_repo: Some(TargetRepo::new("org/new-target".to_string()).unwrap()),
            event_type: None,
            gh_app_installation_id: None,
        };
        let updated = update_subscriber_inner(State(state.clone()), Path(id), Json(update_payload))
            .await
            .unwrap();
        assert_eq!(
            updated.subscriber.target_repo,
            TargetRepo::new("org/new-target".to_string()).unwrap()
        );

        assert_eq!(updated.links.self_link.href, format!("/subscribers/{}", id));

        // Delete
        delete_subscriber_inner(State(state.clone()), Path(id))
            .await
            .unwrap();

        // Verify delete
        let get_after_delete = get_subscriber_inner(State(state.clone()), Path(id)).await;
        assert!(get_after_delete.is_err());
    }

    #[tokio::test]
    async fn test_list_subscribers_pagination() {
        let pool = create_test_db().await;
        let config = crate::test_utils::create_test_config();
        let state = AppState {
            config: std::sync::Arc::new(config),
            db_pool: pool.clone(),
        };

        // Create 3 subscribers
        for i in 0..3 {
            let payload = CreateSubscriber {
                source_repo_url: RepoUrl::new(format!("https://github.com/org/repo{}", i)).unwrap(),
                source_branch_name: BranchName::new("main".to_string()).unwrap(),
                target_repo: TargetRepo::new("org/target".to_string()).unwrap(),
                event_type: EventType::new("dispatch".to_string()).unwrap(),
                gh_app_installation_id: 1,
            };
            let _ = create_subscriber_inner(State(state.clone()), Json(payload))
                .await
                .unwrap();
        }

        // Fetch first page (limit 2)
        let page1 = list_subscribers_inner(
            State(state.clone()),
            Query(ListSubscribersQuery {
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
        let last_id = page1.data.last().unwrap().subscriber.id;
        let page2 = list_subscribers_inner(
            State(state.clone()),
            Query(ListSubscribersQuery {
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
            db_pool: pool.clone(),
        };
        let payload = CreateSubscriber {
            source_repo_url: RepoUrl::new("https://github.com/org/repo".to_string()).unwrap(),
            source_branch_name: BranchName::new("main".to_string()).unwrap(),
            target_repo: TargetRepo::new("org/target".to_string()).unwrap(),
            event_type: EventType::new("dispatch".to_string()).unwrap(),
            gh_app_installation_id: 1,
        };

        // Create two subscribers for the same branch
        let sub1 = create_subscriber_inner(State(state.clone()), Json(payload.clone()))
            .await
            .unwrap();
        let sub2 = create_subscriber_inner(State(state.clone()), Json(payload))
            .await
            .unwrap();

        let branch_id = sub1.subscriber.branch_id;

        // Verify branch exists
        let branch: Option<(i64,)> = sqlx::query_as("SELECT id FROM branches WHERE id = ?")
            .bind(branch_id)
            .fetch_optional(&pool)
            .await
            .unwrap();
        assert!(branch.is_some());

        // Delete first subscriber
        delete_subscriber_inner(State(state.clone()), Path(sub1.subscriber.id))
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

        // Delete second subscriber
        delete_subscriber_inner(State(state.clone()), Path(sub2.subscriber.id))
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
