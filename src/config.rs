use config::{Config as ConfigCrate, Environment};
use serde::Deserialize;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;
use url::Url;
use validator::Validate;

use crate::domain::{AcceptHeader, ApiVersion, NonEmptyString};
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
        if dotenvy::dotenv().is_ok() {
            #[cfg(debug_assertions)]
            tracing::info!("Successfully loaded local `.env` file.");

            #[cfg(not(debug_assertions))]
            tracing::warn!(
                "Successfully loaded local `.env` file. \
                If this is a production build, \
                environment variables should be set prior to execution."
            );
        }

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
    pub address: SocketAddr,

    /// The User-Agent string for HTTP requests.
    pub user_agent: NonEmptyString,

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
    pub url: Url,

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
    pub base_url: Url,

    /// The specific GitHub API version to use.
    pub version: ApiVersion,

    /// The value of the Accept header.
    pub accept_header: AcceptHeader,
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
    pub api_key: Option<NonEmptyString>,

    /// Allow unauthenticated access to the API.
    #[serde(default)]
    pub allow_unauthenticated: bool,

    /// GitHub App's Client ID.
    pub client_id: NonEmptyString,

    /// Path to the GitHub App's private key.
    pub pem_path: PathBuf,
}
