//! Definitions for fatal and setup errors.

use crate::repository::RepositoryError;
use config::ConfigError;
use thiserror::Error;
use validator::ValidationErrors;

/// An error that requires the server to be shut down.
#[derive(Debug, Error)]
pub enum FatalError {
    /// Database is down or URL is incorrect.
    #[error("Database connection: {0}")]
    DbConnection(#[from] sqlx::Error),

    /// Repository error.
    #[error("Repository error: {0}")]
    Repository(#[from] RepositoryError),

    /// Error in database migration.
    #[error("Database migration: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),

    /// Could not reserve an IP address with a TCP port
    /// to connect to the server.
    #[error("TCP binding: {0}")]
    TcpBinding(#[source] std::io::Error),

    /// I/O error during server's execution loop.
    #[error("Serve: {0}")]
    Serve(#[source] std::io::Error),

    // Docs deferred to inner type.
    #[allow(missing_docs)]
    #[error("HTTP Client creation: {0}")]
    ClientCreation(#[from] ClientCreationError),

    /// Environment variable not set.
    #[error("Environment variable '{0}' not set")]
    EnvVarNotSet(String),

    /// Could not load the authentication key.
    #[error("Failed to load authentication key: {0}")]
    AuthKeyLoading(#[source] jsonwebtoken::errors::Error),

    /// Could not read the authentication key file.
    #[error("Failed to read authentication key file: {0}")]
    AuthKeyIo(#[source] std::io::Error),

    /// Could not open the gix repository.
    #[error("Failed to open gix repository: {0}")]
    GitOpen(#[from] Box<gix::open::Error>),

    /// Could not initialize the gix repository.
    #[error("Failed to initialize gix repository: {0}")]
    GitInit(#[from] Box<gix::init::Error>),

    /// Configuration is invalid.
    #[error(transparent)]
    Setup(SetupError),

    /// Configuration validation failed.
    #[error("Configuration validation failed: {0}")]
    Validation(#[from] ValidationErrors),
}

/// Error about the setup configuration.
#[derive(Debug, Error)]
pub enum SetupError {
    /// Configuration is incomplete.
    #[error("Configuration error: {0}")]
    Config(#[source] ConfigError),

    /// Error in retrieving configuration from the environment.
    #[error("Failed to load configuration: {0}")]
    Env(#[source] dotenvy::Error),
}

impl From<config::ConfigError> for FatalError {
    fn from(e: config::ConfigError) -> Self {
        FatalError::Setup(SetupError::Config(e))
    }
}

impl From<dotenvy::Error> for FatalError {
    fn from(e: dotenvy::Error) -> Self {
        FatalError::Setup(SetupError::Env(e))
    }
}

/// HTTP Client creation failed.
///
/// The server cannot trigger workflows.
#[derive(Debug, Error)]
#[error(transparent)]
pub struct ClientCreationError(#[from] reqwest::Error);
