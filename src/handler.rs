//! Axum route handlers.

use crate::error::HandlerError;
use crate::model::{CreateSubscriber, Subscriber, UpdateSubscriber};
use crate::state::AppState;
use axum::{
    Json,
    extract::{Path, State},
};
use tracing::info;

/// Stores a new [`Subscriber`] in the database.
pub async fn create_subscriber(
    State(state): State<AppState>,
    Json(payload): Json<CreateSubscriber>,
) -> Result<Json<Subscriber>, HandlerError> {
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

    Ok(Json(subscriber))
}

/// Retrieves all [`Subscriber`]s.
pub async fn list_subscribers(
    State(state): State<AppState>,
) -> Result<Json<Vec<Subscriber>>, HandlerError> {
    let subscribers = sqlx::query_as::<_, Subscriber>("SELECT * FROM subscribers")
        .fetch_all(&state.db_pool)
        .await?;
    Ok(Json(subscribers))
}

/// Retrieves a specific [`Subscriber`] by ID.
pub async fn get_subscriber(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<Subscriber>, HandlerError> {
    let subscriber = sqlx::query_as::<_, Subscriber>("SELECT * FROM subscribers WHERE id = ?")
        .bind(id)
        .fetch_optional(&state.db_pool)
        .await?
        .ok_or(HandlerError::NotFound)?;
    Ok(Json(subscriber))
}

/// Updates an existing [`Subscriber`].
pub async fn update_subscriber(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(payload): Json<UpdateSubscriber>,
) -> Result<Json<Subscriber>, HandlerError> {
    let mut query = "UPDATE subscribers SET ".to_string();
    let mut updates = Vec::new();

    if let Some(target_repo) = &payload.target_repo {
        updates.push(format!("target_repo = '{}'", target_repo));
    }
    if let Some(event_type) = &payload.event_type {
        updates.push(format!("event_type = '{}'", event_type));
    }
    if let Some(id) = payload.gh_app_installation_id {
        updates.push(format!("gh_app_installation_id = {}", id));
    }

    if updates.is_empty() {
        return get_subscriber(State(state), Path(id)).await;
    }

    query.push_str(&updates.join(", "));
    query.push_str(", updated_at = CURRENT_TIMESTAMP WHERE id = ? RETURNING *");

    let subscriber = sqlx::query_as::<_, Subscriber>(&query)
        .bind(id)
        .fetch_optional(&state.db_pool)
        .await?
        .ok_or(HandlerError::NotFound)?;

    Ok(Json(subscriber))
}

/// Deletes a [`Subscriber`] by ID.
pub async fn delete_subscriber(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<(), HandlerError> {
    let result = sqlx::query("DELETE FROM subscribers WHERE id = ?")
        .bind(id)
        .execute(&state.db_pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(HandlerError::NotFound);
    }
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

    sqlx::query_scalar::<_, i64>("INSERT INTO branches (repo_url, name) VALUES (?, ?) RETURNING id")
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
    use crate::model::CreateSubscriber;
    use crate::state::AppState;
    use crate::test_utils::create_test_db;
    use axum::Json;
    use axum::extract::State;

    #[tokio::test]
    async fn test_crud_subscriber() {
        let pool = create_test_db().await;
        let state = AppState {
            db_pool: pool.clone(),
        };
        let payload = CreateSubscriber {
            source_repo_url: "https://github.com/org/repo".to_string(),
            source_branch_name: "main".to_string(),
            target_repo: "https://github.com/org/target".to_string(),
            event_type: "dispatch".to_string(),
            gh_app_installation_id: 1,
        };

        // Create
        let res = create_subscriber(State(state.clone()), Json(payload))
            .await
            .unwrap();
        let id = res.id;

        // List
        let list = list_subscribers(State(state.clone())).await.unwrap();
        assert_eq!(list.len(), 1);

        // Get
        let get = get_subscriber(State(state.clone()), Path(id))
            .await
            .unwrap();
        assert_eq!(get.id, id);

        // Update
        let update_payload = UpdateSubscriber {
            target_repo: Some("https://github.com/org/new-target".to_string()),
            event_type: None,
            gh_app_installation_id: None,
        };
        let updated = update_subscriber(State(state.clone()), Path(id), Json(update_payload))
            .await
            .unwrap();
        assert_eq!(updated.target_repo, "https://github.com/org/new-target");

        // Delete
        delete_subscriber(State(state.clone()), Path(id))
            .await
            .unwrap();

        // Verify delete
        let get_after_delete = get_subscriber(State(state.clone()), Path(id)).await;
        assert!(get_after_delete.is_err());
    }
}
