//! Definitions for common error types.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use config::ConfigError;
use rovo::aide::OperationOutput;
use thiserror::Error;
use validator::ValidationErrors;

/// Validation error.
#[derive(Debug, Error)]
pub enum ValidationError {
    /// Invalid field value.
    #[error("Validation error: {0}")]
    InvalidValue(String),
}

impl IntoResponse for ValidationError {
    fn into_response(self) -> Response {
        (StatusCode::UNPROCESSABLE_ENTITY, self.to_string()).into_response()
    }
}

/// An error happened inside an Axum handler.
#[derive(Debug, Error)]
pub enum HandlerError {
    /// Database query execution failure.
    #[error("SQLx Error: {0}")]
    DbQuery(#[from] sqlx::Error),

    /// Requested resource not found.
    #[error("Not Found")]
    NotFound,
}

impl OperationOutput for HandlerError {
    type Inner = ();
}

impl IntoResponse for HandlerError {
    fn into_response(self) -> Response {
        let status = match self {
            HandlerError::DbQuery(_) => StatusCode::INTERNAL_SERVER_ERROR,
            HandlerError::NotFound => StatusCode::NOT_FOUND,
        };
        (status, self.to_string()).into_response()
    }
}

/// An error that requires the server to be shut down.
#[derive(Debug, Error)]
pub enum FatalError {
    /// Database is down or URL is incorrect.
    #[error("Database connection: {0}")]
    DbConnection(#[from] sqlx::Error),

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

/// Error in retrieving a commit or its info using `git ls-remote`.
#[derive(Debug, Error)]
pub enum CommitHashError {
    /// Validation error.
    #[error("Validation error: {0}")]
    Validation(#[from] ValidationError),

    /// I/O error while spawning the process.
    #[error("I/O error in `git ls-remote`: {0}")]
    Io(#[from] std::io::Error),

    /// Unexpected exit status.
    #[error("Unexpected `git ls-remote` exit status: {0}")]
    UnexpectedStatus(String),

    /// Unexpected output format.
    #[error(
        "Unexpected `git ls-remote` output format. Repo: {repo_url}; Branch: {branch}; Stdout: {stdout}"
    )]
    UnexpectedOutput {
        /// The process output text.
        stdout: String,
        /// The relevant git repository URL.
        repo_url: String,
        /// The relevant git branch.
        branch: String,
    },
}
