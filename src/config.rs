use std::time::Duration;

/// Holds all application-wide configuration settings.
#[derive(Clone, Default)]
pub struct Config {
    /// Server-related configuration settings.
    pub server: ServerConfig,

    /// Database-related configuration settings.
    pub database: DatabaseConfig,

    /// GitHub API communication configuration settings.
    pub github_api: GitHubApiConfig,

    /// Asynchronous engine configuration settings.
    pub engine: EngineConfig,

    /// Authentication configuration settings.
    pub auth: AuthConfig,
}

/// Configuration for the HTTP server.
#[derive(Clone)]
pub struct ServerConfig {
    /// The bind address for the server.
    pub address: String,

    /// The User-Agent string for HTTP requests.
    pub user_agent: String,

    /// Timeout duration for incoming HTTP requests.
    pub in_request_timeout: Duration,

    /// Timeout duration for outgoing HTTP requests.
    pub out_request_timeout: Duration,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            address: "0.0.0.0:3000".to_string(),
            user_agent: "nilirad-relay-server".to_string(),
            in_request_timeout: Duration::from_secs(30),
            out_request_timeout: Duration::from_secs(30),
        }
    }
}

/// Configuration for the database connection.
#[derive(Clone)]
pub struct DatabaseConfig {
    /// The connection URL for the database.
    pub url: String,

    /// The database connection timeout duration.
    pub timeout: Duration,

    /// The maximum size of the polling database buffer.
    pub polling_db_buffer_size: usize,

    /// The cooldown duration for polling database errors.
    pub polling_db_error_cooldown: Duration,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: "sqlite://relay.db?mode=rwc".to_string(),
            timeout: Duration::from_secs(3),
            polling_db_buffer_size: 3,
            polling_db_error_cooldown: Duration::from_secs(5 * 60),
        }
    }
}

/// Configuration for GitHub API interactions.
#[derive(Clone)]
pub struct GitHubApiConfig {
    /// The base URL for the GitHub API.
    pub base_url: String,

    /// The specific GitHub API version to use.
    pub version: String,

    /// The value of the Accept header.
    pub accept_header: String,
}

impl Default for GitHubApiConfig {
    fn default() -> Self {
        Self {
            base_url: "https://api.github.com".to_string(),
            version: "2026-03-10".to_string(),
            accept_header: "application/vnd.github+json".to_string(),
        }
    }
}

/// Configuration for internal engine processes.
#[derive(Clone)]
pub struct EngineConfig {
    /// Duration to sleep between polling cycles.
    pub polling_sleep: Duration,

    /// The interval for polling the trigger queue.
    pub trigger_queue_polling_interval: Duration,

    /// The maximum number of trigger retry attempts.
    pub trigger_retry_max_attempts: u32,

    /// The base duration for retry backoff.
    pub trigger_retry_backoff_base: Duration,

    /// The duration before a task is considered stuck.
    pub stuck_task_threshold: Duration,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            polling_sleep: Duration::from_secs(5 * 60),
            trigger_queue_polling_interval: Duration::from_secs(5),
            trigger_retry_max_attempts: 10,
            trigger_retry_backoff_base: Duration::from_secs(10),
            stuck_task_threshold: Duration::from_secs(5 * 60),
        }
    }
}

/// Configuration for authentication mechanisms.
#[derive(Clone)]
pub struct AuthConfig {
    /// Buffer time allowed for clock drift.
    pub clock_drift_buffer: Duration,

    /// Duration for which an authentication token is valid.
    pub token_validity: Duration,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            clock_drift_buffer: Duration::from_secs(60),
            token_validity: Duration::from_secs(5 * 60),
        }
    }
}
