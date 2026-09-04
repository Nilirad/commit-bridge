//! Dispatch of `repository_dispatch` events to target repositories.

use tracing::info;

use crate::{
    model::{SubscriptionWithBranch, TriggerQueueItem},
    repository::subscription::SubscriptionRepository,
    trigger::{
        TriggerEngine,
        error::{RequestError, WorkflowTriggerError},
    },
};

/// Sends a `repository_dispatch` event for each relevant [`Subscription`].
///
/// <!-- LINKS -->
/// [`Subscription`]: crate::model::Subscription
#[tracing::instrument(skip_all, fields(otel.kind = "internal"))]
pub async fn dispatch_events(
    engine: &TriggerEngine,
    trigger: &TriggerQueueItem,
) -> Result<(), WorkflowTriggerError> {
    let sub_with_branch = engine
        .ctx
        .repository
        .subscriptions_get_by_keys_with_branch(
            trigger.branch_id,
            &trigger.target_repo,
            &trigger.event_type,
        )
        .await?
        .ok_or_else(|| {
            WorkflowTriggerError::Repository(crate::repository::RepositoryError::NotFound)
        })?;

    info!(
        "Received update event for branch {} (repo: {}, branch: {}): {}",
        trigger.branch_id,
        sub_with_branch.source_branch.repo_url,
        sub_with_branch.source_branch.name,
        trigger.new_hash
    );

    let iat = engine
        .authenticator
        .request_installation_token(&sub_with_branch.subscription)
        .await?;
    notify_subscription(engine, iat, trigger, sub_with_branch).await?;

    Ok(())
}

/// Manages IAT authentication,
/// and sends a `repository_dispatch` event to the specified [`Subscription`].
///
/// <!-- LINKS -->
/// [`Subscription`]: crate::model::Subscription
#[tracing::instrument(skip_all, fields(otel.kind = "internal"))]
async fn notify_subscription(
    engine: &TriggerEngine,
    iat: String,
    trigger: &TriggerQueueItem,
    sub_with_branch: SubscriptionWithBranch,
) -> Result<(), WorkflowTriggerError> {
    send_repository_dispatch(engine, &iat, trigger, &sub_with_branch).await?;
    Ok(())
}

/// Sends a `repository_dispatch` event to the specified [`Subscription`].
///
/// <!-- LINKS -->
/// [`Subscription`]: crate::model::Subscription
#[tracing::instrument(
    skip_all,
    fields(
        otel.kind = "client",
        otel.status_code = tracing::field::Empty,
        error.type = tracing::field::Empty,
    )
)]
async fn send_repository_dispatch(
    engine: &TriggerEngine,
    iat: &str,
    trigger: &TriggerQueueItem,
    sub_with_branch: &SubscriptionWithBranch,
) -> Result<(), WorkflowTriggerError> {
    let api_url = format!(
        "{}/repos/{}/dispatches",
        engine
            .ctx
            .config
            .github_api
            .base_url
            .as_str()
            .trim_end_matches('/'),
        sub_with_branch.subscription.target_repo
    );

    let payload = serde_json::json!({
        "event_type": sub_with_branch.subscription.event_type,
        "client_payload": {
            "branch_id": trigger.branch_id.to_string(),
            "new_commit_hash": trigger.new_hash,
            "source_repo": sub_with_branch.source_branch.repo_url.to_string(),
            "source_branch": sub_with_branch.source_branch.name.to_string(),
        }
    });

    info!(
        "Sending payload to {} (Source repo: {}, Tracked branch: {}): {}",
        sub_with_branch.subscription.target_repo,
        sub_with_branch.source_branch.repo_url,
        sub_with_branch.source_branch.name,
        payload
    );

    let response = engine
        .http_client
        .post(&api_url)
        .bearer_auth(iat)
        .header(
            "Accept",
            engine.ctx.config.github_api.accept_header.to_string(),
        )
        .header(
            "X-GitHub-Api-Version",
            engine.ctx.config.github_api.version.to_string(),
        )
        .json(&payload)
        .send()
        .await?;

    if response.status().is_success() {
        info!(
            "`repository_dispatch` sent to {} (Source repo: {}, Tracked branch: {}): Event: {}",
            sub_with_branch.subscription.target_repo,
            sub_with_branch.source_branch.repo_url,
            sub_with_branch.source_branch.name,
            sub_with_branch.subscription.event_type
        );
        Ok(())
    } else {
        let span = tracing::Span::current();
        span.record("otel.status_code", "ERROR");
        span.record("error.type", response.status().as_u16().to_string());
        Err(WorkflowTriggerError::Api(RequestError::Response {
            status: response.status(),
            text: response.text().await?,
        }))
    }
}
