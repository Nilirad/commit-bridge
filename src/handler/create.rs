//! Create a new subscription handler.

use super::map_to_hal;
use crate::error::HandlerError;
use crate::model::{CreateSubscription, SubscriptionHal};
use crate::repository::subscription::SubscriptionRepository;
use crate::state::AppState;
use axum::{Json, extract::State};
use rovo::rovo;
use tracing::{info, instrument};

/// Create a new subscription mapping.
///
/// Creates a new subscription mapping between a source branch and a target repository.
///
/// # Responses
///
/// 201: Json<SubscriptionHal> - Subscription created successfully
/// 401: () - Unauthorized
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
    payload.source_repo_url = %payload.source_repo_url.as_str(),
    payload.source_branch_name = %payload.source_branch_name.as_str(),
    payload.target_repo = %payload.target_repo.as_str(),
    payload.event_type = %payload.event_type.as_str(),
    payload.gh_app_installation_id = %payload.gh_app_installation_id,
))]
pub async fn create_subscription(
    state: State<AppState>,
    payload: Json<CreateSubscription>,
) -> Result<Json<SubscriptionHal>, HandlerError> {
    create_subscription_inner(state, payload).await
}

/// Internal implementation of [`create_subscription`].
pub(super) async fn create_subscription_inner(
    State(state): State<AppState>,
    Json(payload): Json<CreateSubscription>,
) -> Result<Json<SubscriptionHal>, HandlerError> {
    let sub_with_branch = state.repository.subscriptions_create(&payload).await?;

    info!(
        "Registered new subscription for branch ID {} (repo: {}, branch: {}): {:?}",
        sub_with_branch.subscription.branch_id,
        sub_with_branch.source_branch.repo_url,
        sub_with_branch.source_branch.name,
        sub_with_branch.subscription
    );

    Ok(Json(map_to_hal(sub_with_branch)))
}
