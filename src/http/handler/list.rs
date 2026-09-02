//! List subscriptions handler.

use super::map_to_hal;
use crate::error::HandlerError;
use crate::http::state::AppState;
use crate::model::{HalLink, SubscriptionHal, SubscriptionPage, SubscriptionPageLinks};
use crate::repository::subscription::SubscriptionRepository;
use axum::{
    Json,
    extract::{Query, State},
};
use rovo::rovo;
use serde::Deserialize;
use tracing::instrument;

/// Query parameters for listing subscriptions.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(super) struct ListSubscriptionsQuery {
    /// Maximum number of subscriptions to return.
    pub limit: Option<usize>,
    /// The ID of the last subscription in the previous page.
    pub last_id: Option<i64>,
}

/// List subscriptions.
///
/// Returns a paginated list of all subscription mappings in the system.
///
/// # Query Parameters
///
/// - `limit`: The maximum number of subscriptions to return.
/// - `last_id`: The ID of the last subscription in the previous page.
///
/// # Responses
///
/// 200: Json<SubscriptionPage> - Paginated list of subscriptions
/// 401: () - Unauthorized
/// 408: () - Request timeout
/// 500: () - Internal server error
///
/// # Metadata
///
/// @tag subscriptions
#[allow(rustdoc::invalid_html_tags)]
#[rovo]
#[instrument(skip_all, fields(
    otel.kind = "internal",
    query.limit = ?query.limit,
    query.last_id = ?query.last_id,
))]
pub async fn list_subscriptions(
    state: State<AppState>,
    query: Query<ListSubscriptionsQuery>,
) -> Result<Json<SubscriptionPage>, HandlerError> {
    list_subscriptions_inner(state, query).await
}

/// Internal implementation of [`list_subscriptions`].
pub(super) async fn list_subscriptions_inner(
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
        .subscriptions_list_paginated(last_id, limit as i64)
        .await?;

    let data: Vec<SubscriptionHal> = subscriptions.into_iter().map(map_to_hal).collect();

    let next_id = data.last().map(|s| s.subscription.id).unwrap_or(last_id);
    let remaining_count = state
        .repository
        .subscriptions_count_remaining(next_id)
        .await?;

    let next_link = data
        .last()
        .filter(|_| remaining_count > 0)
        .map(|s| HalLink {
            href: format!(
                "/subscriptions?limit={}&last_id={}",
                limit, s.subscription.id
            ),
        });

    Ok(Json(SubscriptionPage {
        data,
        remaining_count,
        links: SubscriptionPageLinks { next: next_link },
    }))
}
