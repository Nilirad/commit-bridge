#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::todo,
    clippy::unimplemented,
    clippy::indexing_slicing
)]

use crate::{
    polling::git::GitFetcher,
    trigger::{Authenticator, error::AuthError},
};
use async_trait::async_trait;
use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};

pub struct MockGitFetcher {
    pub hash: String,
}

#[async_trait]
impl GitFetcher for MockGitFetcher {
    async fn get_latest_hash(
        &self,
        _repo: &str,
        _branch: &str,
    ) -> Result<String, crate::error::CommitHashError> {
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
        _sub: &crate::model::Subscriber,
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
            address: "127.0.0.1:0".to_string(),
            user_agent: "test-agent".to_string(),
            in_request_timeout: std::time::Duration::from_secs(1),
            out_request_timeout: std::time::Duration::from_secs(1),
        },
        database: crate::config::DatabaseConfig {
            url: "sqlite::memory:".to_string(),
            timeout: std::time::Duration::from_secs(1),
            polling_db_buffer_size: 1,
            polling_db_error_cooldown: std::time::Duration::from_secs(1),
        },
        github_api: crate::config::GitHubApiConfig {
            base_url: "http://localhost".to_string(),
            version: "2026-03-10".to_string(),
            accept_header: "application/vnd.github+json".to_string(),
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
        },
    }
}
