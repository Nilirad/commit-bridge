//! Lifecycle of a single queued trigger:
//! processing, cleanup, and retry scheduling.

use tracing::warn;

use crate::{
    model::TriggerQueueItem,
    repository::trigger::{TriggerRepository, UpdateRetryStatus},
    trigger::{TriggerEngine, dispatch_events, error::WorkflowTriggerError},
};

/// Processes a single queued trigger.
#[tracing::instrument(
    skip_all,
    fields(
        otel.kind = "consumer",
        otel.status_code = tracing::field::Empty,
        error.type = tracing::field::Empty,
        trigger.id = tracing::field::Empty,
        trigger.branch_id = tracing::field::Empty,
        trigger.new_hash = tracing::field::Empty,
        trigger.target_repo = tracing::field::Empty,
        trigger.event_type = tracing::field::Empty,
        trigger.gh_app_installation_id = tracing::field::Empty,
        trigger.retry_count = tracing::field::Empty,
    )
)]
pub(super) async fn process_trigger(
    engine: &TriggerEngine,
    trigger: TriggerQueueItem,
) -> Result<(), WorkflowTriggerError> {
    let result = process_trigger_inner(engine, trigger).await;
    if result.is_err() {
        tracing::Span::current().record("otel.status_code", "ERROR");
    }
    result
}

/// Internal implementation of [`process_trigger`].
async fn process_trigger_inner(
    engine: &TriggerEngine,
    trigger: TriggerQueueItem,
) -> Result<(), WorkflowTriggerError> {
    crate::telemetry::add_link_from_serialized_context(
        &tracing::Span::current(),
        trigger.span_context.as_deref(),
    );

    let span = tracing::Span::current();
    span.record("trigger.id", trigger.id);
    span.record("trigger.branch_id", trigger.branch_id);
    span.record("trigger.new_hash", trigger.new_hash.as_str());
    span.record("trigger.target_repo", trigger.target_repo.as_str());
    span.record("trigger.event_type", trigger.event_type.as_str());
    span.record(
        "trigger.gh_app_installation_id",
        trigger.gh_app_installation_id,
    );
    span.record("trigger.retry_count", trigger.retry_count);

    let dispatch_result = dispatch_events(engine, &trigger).await;
    match dispatch_result {
        Ok(_) => delete_processed_trigger(engine, trigger.id).await,
        Err(WorkflowTriggerError::Repository(crate::repository::RepositoryError::NotFound)) => {
            warn!(
                "Subscription for branch ID {} and target repo {} was not found (likely deleted). Deleting trigger task {} from queue.",
                trigger.branch_id, trigger.target_repo, trigger.id
            );
            delete_processed_trigger(engine, trigger.id).await
        }
        Err(e) => {
            let span = tracing::Span::current();
            span.record("otel.status_code", "ERROR");
            span.record("error.type", "dispatch_failed");
            warn!("Dispatch failed: {e}");
            if let Err(retry_err) = schedule_retry(engine, trigger, e).await {
                tracing::Span::current().record("error.type", "retry_scheduling_failed");
                return Err(retry_err);
            }
            Ok(())
        }
    }
}

/// Deletes a processed trigger from the queue,
/// marking the current span as failed if the deletion errors.
async fn delete_processed_trigger(
    engine: &TriggerEngine,
    trigger_id: i64,
) -> Result<(), WorkflowTriggerError> {
    if let Err(e) = engine.ctx.repository.trigger_queue_delete(trigger_id).await {
        tracing::Span::current().record("error.type", "queue_delete_failed");
        return Err(e.into());
    }
    Ok(())
}

/// Schedules the next retry for a trigger in the `trigger_queue`.
#[tracing::instrument(
    skip_all,
    fields(
        otel.kind = "internal",
        otel.status_code = tracing::field::Empty,
        error.type = tracing::field::Empty,
    )
)]
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
        let span = tracing::Span::current();
        span.record("otel.status_code", "ERROR");
        span.record("error.type", "retries_exhausted");
        tracing::warn!(
            "Task {} failed after {} attempts: {e}",
            trigger.id,
            max_attempts
        );
    }

    engine
        .ctx
        .repository
        .trigger_queue_update_retry_status(UpdateRetryStatus {
            id: trigger.id,
            retry_count: trigger.retry_count,
            max_attempts,
            backoff_base_secs,
        })
        .await?;

    Ok(())
}

/// Recovers tasks that have been stuck in `PROCESSING` for too long.
#[tracing::instrument(skip_all, fields(otel.kind = "internal"))]
pub async fn recover_stuck_tasks(
    repo: &crate::repository::SqliteRepository,
    config: &crate::config::Config,
) -> Result<(), crate::repository::RepositoryError> {
    let threshold_seconds = config.engine.stuck_task_threshold.as_secs();

    repo.trigger_queue_recover_stuck_tasks(threshold_seconds)
        .await?;
    Ok(())
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

    use super::recover_stuck_tasks;
    use super::schedule_retry;
    use crate::context::SharedContext;
    use crate::domain::{CommitHash, EventType, TargetRepo};
    use crate::model::TriggerQueueItem;
    use crate::repository::trigger::TriggerRepository;
    use crate::test_utils::{MockAuthenticator, MockGitFetcher};
    use crate::trigger::error::{RequestError, WorkflowTriggerError};
    use crate::trigger::{TriggerEngine, process_queue};
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
            .trigger_queue_process_oldest_pending()
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
            span_context: None,
        };

        let engine = TriggerEngine {
            ctx: SharedContext {
                config: crate::test_utils::create_test_config(),
                repository: std::sync::Arc::new(crate::repository::SqliteRepository::new(
                    pool.clone(),
                )),
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

        // Setup subscription
        sqlx::query!(
            "INSERT INTO branches (repo_url, name) VALUES (?, ?)",
            "repo",
            "main"
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query!("INSERT INTO subscriptions (branch_id, target_repo, event_type, gh_app_installation_id) VALUES (?, ?, ?, ?)",
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
