//! Get a single subscription handler.

use super::map_to_hal;
use crate::error::HandlerError;
use crate::http::state::AppState;
use crate::model::SubscriptionHal;
use crate::repository::subscription::SubscriptionRepository;
use axum::{
    Json,
    extract::{Path, State},
};
use rovo::rovo;
use tracing::instrument;

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
/// 401: () - Unauthorized
/// 404: () - Subscription was not found
/// 408: () - Request timeout
/// 500: () - Internal server error
///
/// # Metadata
///
/// @tag subscriptions
#[allow(rustdoc::invalid_html_tags)]
#[rovo]
#[instrument(skip_all, fields(otel.kind = "internal", id = %id))]
pub async fn get_subscription(
    state: State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<SubscriptionHal>, HandlerError> {
    get_subscription_inner(state, Path(id)).await
}

/// Internal implementation of [`get_subscription`].
pub(super) async fn get_subscription_inner(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<SubscriptionHal>, HandlerError> {
    let sub_with_branch = state
        .repository
        .subscriptions_get_by_id_with_branch(id)
        .await?
        .ok_or(HandlerError::NotFound)?;
    Ok(Json(map_to_hal(sub_with_branch)))
}
