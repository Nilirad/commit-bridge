//! Domain type to represent a GitHub API version in YYYY-MM-DD format.

use crate::error::ValidationError;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

/// The GitHub API version.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(try_from = "String", into = "String")]
pub struct ApiVersion(String);

impl ApiVersion {
    /// Create a new `ApiVersion`.
    pub fn new(value: String) -> Result<Self, ValidationError> {
        Self::try_from(value)
    }

    /// Get the inner string.
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl TryFrom<String> for ApiVersion {
    type Error = ValidationError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if let Err(e) = NaiveDate::parse_from_str(&value, "%Y-%m-%d") {
            tracing::warn!("Validation failed for ApiVersion: {}. Error: {}", value, e);
            return Err(ValidationError::InvalidValue(format!(
                "Invalid API version format (expected YYYY-MM-DD): {}",
                value
            )));
        }

        Ok(ApiVersion(value))
    }
}

impl From<ApiVersion> for String {
    fn from(val: ApiVersion) -> Self {
        val.0
    }
}

impl std::fmt::Display for ApiVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
