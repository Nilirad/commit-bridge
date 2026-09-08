//! Validation error type for domain values.

use thiserror::Error;

/// Validation error.
#[derive(Debug, Error)]
pub enum ValidationError {
    /// Invalid field value.
    #[error("Validation error: {0}")]
    InvalidValue(String),
}
