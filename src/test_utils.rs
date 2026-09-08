#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::todo,
    clippy::unimplemented,
    clippy::indexing_slicing
)]

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};
use tokio_util::sync::CancellationToken;
use url::Url;

use crate::{
    context::SharedContext,
    domain::{AcceptHeader, ApiVersion, CommitHash, NonEmptyString},
    http::state::AppState,
    polling::git::GitFetcher,
    repository::SqliteRepository,
    trigger::{Authenticator, TriggerEngine, error::AuthError},
};

pub struct MockGitFetcher {
    pub hash: CommitHash,
}

#[async_trait]
impl GitFetcher for MockGitFetcher {
    async fn get_latest_hash(
        &self,
        _repo: &str,
        _branch: &str,
    ) -> Result<CommitHash, crate::polling::CommitHashError> {
        Ok(self.hash.clone())
    }
}

pub struct MockAuthenticator {
    pub iat: String,
}

#[async_trait]
impl Authenticator for MockAuthenticator {
    async fn request_installation_token(
        &self,
        _sub: &crate::model::Subscription,
    ) -> Result<String, AuthError> {
        Ok(self.iat.clone())
    }
}

pub async fn create_test_db() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .expect("Failed to create in-memory database");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    pool
}

pub fn create_test_config() -> crate::config::Config {
    crate::config::Config {
        server: crate::config::ServerConfig {
            address: "127.0.0.1:0".parse().unwrap(),
            user_agent: NonEmptyString::new("test-agent".to_string()).unwrap(),
            in_request_timeout: std::time::Duration::from_secs(1),
            out_request_timeout: std::time::Duration::from_secs(1),
        },
        database: crate::config::DatabaseConfig {
            url: Url::parse("sqlite::memory:").unwrap(),
            timeout: std::time::Duration::from_secs(1),
            polling_db_buffer_size: 1,
            polling_db_error_cooldown: std::time::Duration::from_secs(1),
            subscriptions_list_limit: 50,
            subscriptions_list_limit_cap: 100,
        },
        github_api: crate::config::GitHubApiConfig {
            base_url: Url::parse("http://localhost").unwrap(),
            version: ApiVersion::new("2026-03-10".to_string()).unwrap(),
            accept_header: AcceptHeader::new("application/vnd.github+json".to_string()).unwrap(),
        },
        engine: crate::config::EngineConfig {
            polling_sleep: std::time::Duration::from_secs(1),
            trigger_queue_polling_interval: std::time::Duration::from_millis(100),
            trigger_retry_max_attempts: 3,
            trigger_retry_backoff_base: std::time::Duration::from_millis(100),
            stuck_task_threshold: std::time::Duration::from_secs(2 * 60),
        },
        auth: crate::config::AuthConfig {
            clock_drift_buffer: std::time::Duration::from_secs(1),
            token_validity: std::time::Duration::from_secs(1),
            api_key: None,
            allow_unauthenticated: false,
            client_id: NonEmptyString::new("test-client-id".to_string()).unwrap(),
            pem_path: PathBuf::from("test-pem-path"),
        },
        git: crate::config::GitConfig {
            repo_path: PathBuf::from("test-git-repo"),
        },
        telemetry: crate::config::TelemetryConfig {
            mark_client_errors_as_error: false,
        },
    }
}

/// Creates an [`AppState`] with the default test configuration,
/// backed by `pool`.
pub fn create_test_state(pool: SqlitePool) -> AppState {
    AppState {
        config: Arc::new(create_test_config()),
        repository: Arc::new(SqliteRepository::new(pool)),
    }
}

/// Creates a [`SharedContext`] backed by `pool`,
/// whose git fetcher always reports `hash`.
pub fn create_test_context(pool: SqlitePool, hash: CommitHash) -> SharedContext {
    SharedContext {
        config: create_test_config(),
        repository: Arc::new(SqliteRepository::new(pool)),
        token: CancellationToken::new(),
        git_fetcher: Arc::new(MockGitFetcher { hash }),
    }
}

/// Creates a [`TriggerEngine`] with the default test mocks,
/// backed by `pool`.
pub fn create_test_engine(pool: SqlitePool) -> TriggerEngine {
    TriggerEngine {
        ctx: create_test_context(pool, CommitHash::new("a".repeat(40)).expect("valid hash")),
        http_client: reqwest::Client::new(),
        authenticator: Box::new(MockAuthenticator {
            iat: "token".to_string(),
        }),
    }
}
