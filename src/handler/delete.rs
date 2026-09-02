//! Delete a subscription handler.

use crate::error::HandlerError;
use crate::repository::subscription::SubscriptionRepository;
use crate::state::AppState;
use axum::extract::{Path, State};
use rovo::rovo;
use tracing::instrument;

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
pub async fn delete_subscription(
    state: State<AppState>,
    Path(id): Path<i64>,
) -> Result<(), HandlerError> {
    delete_subscription_inner(state, Path(id)).await
}

/// Internal implementation of [`delete_subscription`].
pub(super) async fn delete_subscription_inner(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<(), HandlerError> {
    state.repository.subscriptions_delete(id).await?;
    Ok(())
}
