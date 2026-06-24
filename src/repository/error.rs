//! Error types for repository operation failures.

/// An error in a repository.
#[derive(thiserror::Error, Debug)]
pub enum RepositoryError {
    /// Requested resource not found.
    #[error("Resource not found")]
    NotFound,

    /// Database operation error.
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
}
