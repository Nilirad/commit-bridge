use config::{Config as ConfigCrate, Environment};
use serde::Deserialize;
use std::time::Duration;
use validator::Validate;

use crate::error::FatalError;

/// Holds all application-wide configuration settings.
#[derive(Clone, Deserialize, Validate)]
pub struct Config {
    /// Server-related configuration settings.
    #[validate(nested)]
    pub server: ServerConfig,

    /// Database-related configuration settings.
    #[validate(nested)]
    pub database: DatabaseConfig,

    /// GitHub API communication configuration settings.
    #[validate(nested)]
    pub github_api: GitHubApiConfig,

    /// Asynchronous engine configuration settings.
    #[validate(nested)]
    pub engine: EngineConfig,

    /// Authentication configuration settings.
    #[validate(nested)]
    pub auth: AuthConfig,
}

impl Config {
    /// Bootstraps the application configuration from the environment.
    pub fn load() -> Result<Self, FatalError> {
        let environment = Environment::with_prefix("RELAY")
            .separator("__")
            .try_parsing(true);

        let config: Config = ConfigCrate::builder()
            .add_source(environment)
            .build()?
            .try_deserialize()?;

        config.validate()?;

        Ok(config)
    }
}

/// Configuration for the HTTP server.
#[derive(Clone, Deserialize, Validate)]
pub struct ServerConfig {
    /// The bind address for the server.
    #[validate(length(min = 1))]
    pub address: String,

    /// The User-Agent string for HTTP requests.
    #[validate(length(min = 1))]
    pub user_agent: String,

    /// Timeout duration for incoming HTTP requests.
    #[serde(with = "humantime_serde")]
    pub in_request_timeout: Duration,

    /// Timeout duration for outgoing HTTP requests.
    #[serde(with = "humantime_serde")]
    pub out_request_timeout: Duration,
}

/// Configuration for the database connection.
#[derive(Clone, Deserialize, Validate)]
pub struct DatabaseConfig {
    /// The connection URL for the database.
    #[validate(length(min = 1))]
    pub url: String,

    /// The database connection timeout duration.
    #[serde(with = "humantime_serde")]
    pub timeout: Duration,

    /// The maximum size of the polling database buffer.
    #[validate(range(min = 1))]
    pub polling_db_buffer_size: usize,

    /// The cooldown duration for polling database errors.
    #[serde(with = "humantime_serde")]
    pub polling_db_error_cooldown: Duration,
}

/// Configuration for GitHub API interactions.
#[derive(Clone, Deserialize, Validate)]
pub struct GitHubApiConfig {
    /// The base URL for the GitHub API.
    #[validate(url)]
    pub base_url: String,

    /// The specific GitHub API version to use.
    #[validate(length(min = 1))]
    pub version: String,

    /// The value of the Accept header.
    #[validate(length(min = 1))]
    pub accept_header: String,
}

/// Configuration for internal engine processes.
#[derive(Clone, Deserialize, Validate)]
pub struct EngineConfig {
    /// Duration to sleep between polling cycles.
    #[serde(with = "humantime_serde")]
    pub polling_sleep: Duration,

    /// The interval for polling the trigger queue.
    #[serde(with = "humantime_serde")]
    pub trigger_queue_polling_interval: Duration,

    /// The maximum number of trigger retry attempts.
    #[validate(range(min = 1))]
    pub trigger_retry_max_attempts: u32,

    /// The base duration for retry backoff.
    #[serde(with = "humantime_serde")]
    pub trigger_retry_backoff_base: Duration,

    /// The duration before a task is considered stuck.
    #[serde(with = "humantime_serde")]
    pub stuck_task_threshold: Duration,
}

/// Configuration for authentication mechanisms.
#[derive(Clone, Deserialize, Validate)]
pub struct AuthConfig {
    /// Buffer time allowed for clock drift.
    #[serde(with = "humantime_serde")]
    pub clock_drift_buffer: Duration,

    /// Duration for which an authentication token is valid.
    #[serde(with = "humantime_serde")]
    pub token_validity: Duration,

    /// Optional API key for authentication.
    ///
    /// If set, the `X-API-KEY` header must be present
    /// and match this value on sensible requests.
    pub api_key: Option<String>,

    /// GitHub App's Client ID.
    #[validate(length(min = 1))]
    pub client_id: String,

    /// Path to the GitHub App's private key.
    #[validate(length(min = 1))]
    pub pem_path: String,
}
