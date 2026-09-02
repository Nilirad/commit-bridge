//! Update an existing subscription handler.

use super::map_to_hal;
use crate::error::HandlerError;
use crate::model::{SubscriptionHal, UpdateSubscription};
use crate::repository::subscription::SubscriptionRepository;
use crate::state::AppState;
use axum::{
    Json,
    extract::{Path, State},
};
use rovo::rovo;
use tracing::instrument;

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
/// 401: () - Unauthorized
/// 404: () - Subscription was not found
/// 408: () - Request timeout
/// 422: () - Validation error
/// 500: () - Internal server error
///
/// # Metadata
///
/// @tag subscriptions
#[allow(rustdoc::invalid_html_tags)]
#[rovo]
#[instrument(skip_all, fields(
    otel.kind = "internal",
    id = %id,
    payload.target_repo = ?payload.target_repo.as_ref().map(|v| v.as_str()),
    payload.event_type = ?payload.event_type.as_ref().map(|v| v.as_str()),
    payload.gh_app_installation_id = ?payload.gh_app_installation_id,
))]
pub async fn update_subscription(
    state: State<AppState>,
    Path(id): Path<i64>,
    payload: Json<UpdateSubscription>,
) -> Result<Json<SubscriptionHal>, HandlerError> {
    update_subscription_inner(state, Path(id), payload).await
}

/// Internal implementation of [`update_subscription`].
pub(super) async fn update_subscription_inner(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(payload): Json<UpdateSubscription>,
) -> Result<Json<SubscriptionHal>, HandlerError> {
    state.repository.subscriptions_update(id, &payload).await?;
    let sub_with_branch = state
        .repository
        .subscriptions_get_by_id_with_branch(id)
        .await?
        .ok_or(HandlerError::NotFound)?;

    Ok(Json(map_to_hal(sub_with_branch)))
}
