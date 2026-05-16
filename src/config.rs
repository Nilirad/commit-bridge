use std::time::Duration;

/// Application configuration.
#[derive(Clone)]
pub struct Config {
    /// User agent for HTTP requests.
    pub user_agent: String,

    /// Base URL for the GitHub API.
    pub github_api_base_url: String,

    /// Address the server listens on.
    pub server_address: String,

    /// URL of the database.
    pub database_url: String,

    /// Timeout for database connection acquisition.
    pub database_timeout: Duration,

    /// Polling sleep duration.
    pub polling_sleep: Duration,

    /// Buffer size for database operations.
    pub polling_db_buffer_size: usize,

    /// Cooldown duration for database errors.
    pub polling_db_error_cooldown: Duration,

    /// Interval for polling the trigger queue.
    pub trigger_queue_polling_interval: Duration,

    /// Clock drift buffer for authentication.
    pub auth_clock_drift_buffer: Duration,

    /// Validity duration for authentication tokens.
    pub auth_token_validity: Duration,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            user_agent: "nilirad-relay-server".to_string(),
            github_api_base_url: "https://api.github.com".to_string(),
            server_address: "0.0.0.0:3000".to_string(),
            database_url: "sqlite://relay.db?mode=rwc".to_string(),
            database_timeout: Duration::from_secs(3),
            polling_sleep: Duration::from_secs(5 * 60),
            polling_db_buffer_size: 3,
            polling_db_error_cooldown: Duration::from_secs(5 * 60),
            trigger_queue_polling_interval: Duration::from_secs(5),
            auth_clock_drift_buffer: Duration::from_secs(60),
            auth_token_validity: Duration::from_secs(5 * 60),
        }
    }
}
