#[derive(thiserror::Error, Debug)]
pub enum RepositoryError {
    #[error("Resource not found")]
    NotFound,
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
}
