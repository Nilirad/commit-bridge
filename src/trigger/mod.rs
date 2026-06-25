//! Asynchronous task to trigger remote repository workflows.

use async_trait::async_trait;
use reqwest::Client;
use tracing::{info, warn};

use crate::{
    context::SharedContext,
    engine::AsyncEngine,
    model::{Subscriber, TriggerQueueItem},
    repository::trigger::{TriggerRepository, UpdateRetryStatus},
    trigger::error::{RequestError, WorkflowTriggerError},
};

mod auth;
pub use auth::{Authenticator, GitHubAuthenticator};
pub mod error;

/// Runs an asynchronous task
/// that triggers a workflow in a remote repository.
pub struct TriggerEngine {
    /// Shared data for all async engines.
    pub ctx: SharedContext,

    /// HTTP client to make requests to the GitHub API.
    pub http_client: Client,

    /// Authenticates requests to the GitHub API.
    pub authenticator: Box<dyn Authenticator + Send + Sync>,
}

#[async_trait]
impl AsyncEngine for TriggerEngine {
    async fn run(&self) {
        trigger_loop(self).await;
    }
}

/// Controls whether to shut down the trigger engine or process a queued event.
async fn trigger_loop(engine: &TriggerEngine) {
    loop {
        tokio::select! {
            _ = engine.ctx.token.cancelled() => break,
            _ = tokio::time::sleep(engine.ctx.config.engine.trigger_queue_polling_interval) => {
                if let Err(e) = process_queue(engine).await {
                    warn!("Error processing queue: {e}");
                }
            }
        }
    }
    info!("Gracefully shutting down trigger engine");
}

/// Processes a single queued event.
async fn process_queue(engine: &TriggerEngine) -> Result<(), WorkflowTriggerError> {
    let Some(trigger) = engine
        .ctx
        .repository
        .find_oldest_pending_and_mark_processing()
        .await?
    else {
        return Ok(());
    };

    let dispatch_result = dispatch_events(engine, &trigger).await;
    match dispatch_result {
        Ok(_) => {
            engine.ctx.repository.delete_by_id(trigger.id).await?;
        }
        Err(e) => {
            warn!("Dispatch failed: {e}");
            schedule_retry(engine, trigger, e).await?;
        }
    }

    Ok(())
}

/// Schedules the next retry for a trigger in the `trigger_queue`.
async fn schedule_retry(
    engine: &TriggerEngine,
    trigger: TriggerQueueItem,
    e: WorkflowTriggerError,
) -> Result<(), WorkflowTriggerError> {
    let next_retry_count = trigger.retry_count + 1;
    let max_attempts = engine.ctx.config.engine.trigger_retry_max_attempts;
    let backoff_base_secs = engine
        .ctx
        .config
        .engine
        .trigger_retry_backoff_base
        .as_secs();

    if next_retry_count as u32 >= max_attempts {
        tracing::warn!(
            "Task {} failed after {} attempts: {e}",
            trigger.id,
            max_attempts
        );
    }

    engine
        .ctx
        .repository
        .update_retry_status(UpdateRetryStatus {
            id: trigger.id,
            retry_count: trigger.retry_count,
            max_attempts,
            backoff_base_secs,
        })
        .await?;

    Ok(())
}

/// Recovers tasks that have been stuck in `PROCESSING` for too long.
pub async fn recover_stuck_tasks(
    repo: &crate::repository::SqliteRepository,
    config: &crate::config::Config,
) -> Result<(), crate::repository::RepositoryError> {
    let threshold_seconds = config.engine.stuck_task_threshold.as_secs();

    repo.recover_stuck_tasks(threshold_seconds).await?;
    Ok(())
}

/// Sends a `repository_dispatch` event for each relevant [`Subscriber`].
pub async fn dispatch_events(
    engine: &TriggerEngine,
    trigger: &TriggerQueueItem,
) -> Result<(), WorkflowTriggerError> {
    info!(
        "Received update event for branch {}: {}",
        trigger.branch_id, trigger.new_hash
    );

    // TODO: Create a subscriber DTO that doesn't contain
    // `id`, `created_at` and `updated_at`.
    let sub = Subscriber {
        id: 0,
        branch_id: trigger.branch_id,
        target_repo: trigger.target_repo.clone(),
        event_type: trigger.event_type.clone(),
        gh_app_installation_id: trigger.gh_app_installation_id,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    let iat = engine
        .authenticator
        .request_installation_token(&sub)
        .await?;
    notify_subscriber(engine, iat, trigger, sub).await?;

    Ok(())
}

/// Manages IAT authentication,
/// and sends a `repository_dispatch` event to the specified [`Subscriber`].
async fn notify_subscriber(
    engine: &TriggerEngine,
    iat: String,
    trigger: &TriggerQueueItem,
    sub: Subscriber,
) -> Result<(), WorkflowTriggerError> {
    send_repository_dispatch(engine, &iat, trigger, &sub).await?;
    Ok(())
}

/// Sends a `repository_dispatch` event to the specified [`Subscriber`].
async fn send_repository_dispatch(
    engine: &TriggerEngine,
    iat: &str,
    trigger: &TriggerQueueItem,
    sub: &Subscriber,
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
        sub.target_repo
    );

    let payload = serde_json::json!({
        "event_type": sub.event_type,
        "client_payload": {
            "branch_id": trigger.branch_id.to_string(),
            "new_commit_hash": trigger.new_hash
       }
    });

    info!("Sending payload to {}: {}", sub.target_repo, payload);

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
            "`repository_dispatch` sent to {}: Event: {}",
            sub.target_repo, sub.event_type
        );
        Ok(())
    } else {
        Err(WorkflowTriggerError::Api(RequestError::Response {
            status: response.status(),
            text: response.text().await?,
        }))
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

    use super::*;
    use crate::domain::{CommitHash, EventType, TargetRepo};
    use crate::test_utils::{MockAuthenticator, MockGitFetcher};
    use std::sync::Arc;

    use tokio_util::sync::CancellationToken;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_recover_stuck_tasks() {
        let pool = crate::test_utils::create_test_db().await;

        // Insert tasks
        let hash = "a".repeat(40);
        // 1. Processing (stuck)
        sqlx::query!(
            "INSERT INTO trigger_queue (branch_id, new_hash, status, retry_count, status_updated_at) VALUES (?, ?, ?, ?, DATETIME('now', '-10 minutes'))",
            1,
            hash,
            "PROCESSING",
            0
        )
        .execute(&pool)
        .await
        .unwrap();
        // 2. Processing (recent)
        sqlx::query!(
            "INSERT INTO trigger_queue (branch_id, new_hash, status, retry_count, next_retry_at, target_repo, event_type, gh_app_installation_id) VALUES (?, ?, ?, ?, datetime('now'), ?, ?, ?)",
            1,
            hash,
            "PROCESSING",
            0,
            "org/repo",
            "event",
            1
        )
        .execute(&pool)
        .await
        .unwrap();
        // 3. Pending
        sqlx::query!(
            "INSERT INTO trigger_queue (branch_id, new_hash, status, retry_count, status_updated_at) VALUES (?, ?, ?, ?, DATETIME('now'))",
            1,
            hash,
            "PENDING",
            0
        )
        .execute(&pool)
        .await
        .unwrap();

        recover_stuck_tasks(
            &crate::repository::SqliteRepository::new(pool.clone()),
            &crate::test_utils::create_test_config(),
        )
        .await
        .unwrap();

        // Check status
        let tasks = sqlx::query!("SELECT status FROM trigger_queue ORDER BY rowid")
            .fetch_all(&pool)
            .await
            .unwrap();

        assert_eq!(tasks[0].status, "PENDING"); // was stuck
        assert_eq!(tasks[1].status, "PROCESSING"); // was recent
        assert_eq!(tasks[2].status, "PENDING"); // was pending
    }

    #[tokio::test]
    async fn test_get_oldest_queued_trigger() {
        let pool = crate::test_utils::create_test_db().await;

        // Insert some dummy items
        let hash = "a".repeat(40);
        sqlx::query!(
            "INSERT INTO trigger_queue (branch_id, new_hash, status, retry_count, next_retry_at, target_repo, event_type, gh_app_installation_id) VALUES (?, ?, ?, ?, datetime('now', '-1 minute'), ?, ?, ?)",
            1,
            hash,
            "PENDING",
            0,
            "org/repo1",
            "event",
            1
        )
        .execute(&pool)
        .await
        .unwrap();
        let hash = "a".repeat(40);
        sqlx::query!(
            "INSERT INTO trigger_queue (branch_id, new_hash, status, retry_count, next_retry_at, target_repo, event_type, gh_app_installation_id) VALUES (?, ?, ?, ?, datetime('now', '-5 minutes'), ?, ?, ?)",
            1,
            hash,
            "PENDING",
            0,
            "org/repo2",
            "event",
            1
        )
        .execute(&pool)
        .await
        .unwrap();
        let hash = "a".repeat(40);
        sqlx::query!(
            "INSERT INTO trigger_queue (branch_id, new_hash, status, retry_count, next_retry_at, target_repo, event_type, gh_app_installation_id) VALUES (?, ?, ?, ?, datetime('now', '+1 minute'), ?, ?, ?)",
            1,
            hash,
            "PENDING",
            0,
            "org/repo3",
            "event",
            1
        )
        .execute(&pool)
        .await
        .unwrap();

        let repo = crate::repository::SqliteRepository::new(pool.clone());
        let trigger = repo
            .find_oldest_pending_and_mark_processing()
            .await
            .unwrap()
            .unwrap();

        // Assert: The one with -5 minutes should be returned
        assert_eq!(trigger.retry_count, 0);

        // Verify it was updated to PROCESSING
        let db_trigger = sqlx::query!("SELECT status FROM trigger_queue WHERE id = ?", trigger.id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(db_trigger.status, "PROCESSING");
    }

    #[tokio::test]
    async fn test_schedule_retry() {
        let pool = crate::test_utils::create_test_db().await;
        let hash = "a".repeat(40);
        let id = sqlx::query!(
            "INSERT INTO trigger_queue (branch_id, new_hash, status, retry_count, next_retry_at) VALUES (?, ?, ?, ?, datetime('now'))",
            1,
            hash,
            "PROCESSING",
            0
        )
        .execute(&pool)
        .await
        .unwrap()
        .last_insert_rowid();

        let trigger = TriggerQueueItem {
            id,
            branch_id: 1,
            new_hash: CommitHash::new("a".repeat(40)).expect("valid commit hash"),
            retry_count: 0,
            target_repo: TargetRepo::new("org/repo".to_string()).unwrap(),
            event_type: EventType::new("event".to_string()).unwrap(),
            gh_app_installation_id: 1,
        };

        let engine = TriggerEngine {
            ctx: SharedContext {
                config: crate::test_utils::create_test_config(),
                repository: std::sync::Arc::new(crate::repository::SqliteRepository::new(
                    pool.clone(),
                )),
                db_pool: pool.clone(),
                token: CancellationToken::new(),
                git_fetcher: Arc::new(MockGitFetcher {
                    hash: CommitHash::new("a".repeat(40)).unwrap(),
                }),
            },
            http_client: reqwest::Client::new(),
            authenticator: Box::new(MockAuthenticator {
                iat: "token".to_string(),
            }),
        };

        schedule_retry(
            &engine,
            trigger,
            WorkflowTriggerError::Api(RequestError::Response {
                status: reqwest::StatusCode::INTERNAL_SERVER_ERROR,
                text: "error".to_string(),
            }),
        )
        .await
        .unwrap();

        let updated = sqlx::query!(
            "SELECT status, retry_count FROM trigger_queue WHERE id = ?",
            id
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(updated.status, "PENDING");
        assert_eq!(updated.retry_count, 1);
    }

    #[tokio::test]
    async fn test_process_queue_failure_and_retry() {
        let pool = crate::test_utils::create_test_db().await;
        let mock_server = MockServer::start().await;

        // Setup subscriber
        sqlx::query!(
            "INSERT INTO branches (repo_url, name) VALUES (?, ?)",
            "repo",
            "main"
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query!("INSERT INTO subscribers (branch_id, target_repo, event_type, gh_app_installation_id) VALUES (?, ?, ?, ?)",
                     1, "org/target", "dispatch", 1).execute(&pool).await.unwrap();

        // Mock token success, but dispatch failure
        Mock::given(method("POST"))
            .and(path("/app/installations/1/access_tokens"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"token": "token"})),
            )
            .mount(&mock_server)
            .await;

        Mock::given(method("POST"))
            .and(path("/repos/org/target/dispatches"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&mock_server)
            .await;

        let hash = "a".repeat(40);
        sqlx::query!(
            "INSERT INTO trigger_queue (branch_id, new_hash, status, retry_count, next_retry_at, target_repo, event_type, gh_app_installation_id) VALUES (?, ?, ?, ?, '2000-01-01 00:00:00', ?, ?, ?)",
            1,
            hash,
            "PENDING",
            0,
            "org/target",
            "dispatch",
            1
        )
        .execute(&pool)
        .await
        .unwrap();

        let engine = TriggerEngine {
            ctx: SharedContext {
                config: crate::test_utils::create_test_config(),
                repository: std::sync::Arc::new(crate::repository::SqliteRepository::new(
                    pool.clone(),
                )),
                db_pool: pool.clone(),
                token: CancellationToken::new(),
                git_fetcher: Arc::new(MockGitFetcher {
                    hash: CommitHash::new("a".repeat(40)).unwrap(),
                }),
            },
            http_client: reqwest::Client::new(),
            authenticator: Box::new(MockAuthenticator {
                iat: "token".to_string(),
            }),
        };

        process_queue(&engine).await.unwrap();

        // Should still exist and retry_count increased
        let trigger = sqlx::query!("SELECT retry_count, status FROM trigger_queue")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(trigger.retry_count, 1);
        assert_eq!(trigger.status, "PENDING");
    }
}
