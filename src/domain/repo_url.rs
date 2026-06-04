//! Domain type to represent a GitHub repository URL.

use crate::error::ValidationError;
use rovo::schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use validator::Validate;

/// The GitHub repository URL.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(try_from = "String", into = "String")]
pub struct RepoUrl(String);

impl std::fmt::Display for RepoUrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl RepoUrl {
    /// Create a new `RepoUrl`.
    pub fn new(value: String) -> Result<Self, ValidationError> {
        Self::try_from(value)
    }

    /// Get the inner string.
    pub fn into_inner(self) -> String {
        self.0
    }

    /// Get a reference to the inner string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for RepoUrl {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for RepoUrl {
    type Error = ValidationError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        #[derive(Validate)]
        struct RepoUrlValidator {
            #[validate(url)]
            val: String,
        }

        let validator = RepoUrlValidator { val: value.clone() };
        if let Err(e) = validator.validate() {
            tracing::warn!("Validation failed for RepoUrl: {}. Error: {}", value, e);
            return Err(ValidationError::InvalidValue(format!(
                "Invalid URL format: {}",
                value
            )));
        }

        if !value.contains("github.com") {
            tracing::warn!(
                "Validation failed for RepoUrl: {}. Must be a github.com URL",
                value
            );
            return Err(ValidationError::InvalidValue(format!(
                "URL must be a github.com URL: {}",
                value
            )));
        }

        Ok(RepoUrl(value))
    }
}

impl From<RepoUrl> for String {
    fn from(val: RepoUrl) -> Self {
        val.0
    }
}

crate::derive_sqlx_traits!(RepoUrl);
