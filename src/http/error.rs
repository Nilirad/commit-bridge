//! Error types for the HTTP layer.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use rovo::aide::OperationOutput;
use thiserror::Error;

use crate::repository::RepositoryError;

/// An error happened inside an Axum handler.
#[derive(Debug, Error)]
pub enum HandlerError {
    /// Database query execution failure.
    #[error("Repository Error: {0}")]
    DbQuery(RepositoryError),

    /// Requested resource not found.
    #[error("Not Found")]
    NotFound,
}

impl From<RepositoryError> for HandlerError {
    fn from(err: RepositoryError) -> Self {
        match err {
            RepositoryError::NotFound => HandlerError::NotFound,
            other => HandlerError::DbQuery(other),
        }
    }
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
