//! Domain type to represent an HTTP Accept header.

use crate::error::ValidationError;
use http::header::HeaderValue;
use serde::{Deserialize, Serialize};

/// The HTTP Accept header value.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(try_from = "String", into = "String")]
pub struct AcceptHeader(String);

impl AcceptHeader {
    /// Create a new `AcceptHeader`.
    pub fn new(value: String) -> Result<Self, ValidationError> {
        Self::try_from(value)
    }

    /// Get the inner string.
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl TryFrom<String> for AcceptHeader {
    type Error = ValidationError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if let Err(e) = HeaderValue::from_str(&value) {
            tracing::warn!(
                "Validation failed for AcceptHeader: {}. Error: {}",
                value,
                e
            );
            return Err(ValidationError::InvalidValue(format!(
                "Invalid Accept header format: {}",
                value
            )));
        }

        Ok(AcceptHeader(value))
    }
}

impl From<AcceptHeader> for String {
    fn from(val: AcceptHeader) -> Self {
        val.0
    }
}

impl std::fmt::Display for AcceptHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
