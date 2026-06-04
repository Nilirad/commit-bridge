//! Domain type to represent a Git branch name.

use crate::error::ValidationError;
use rovo::schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use validator::Validate;

/// The Git branch name.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(try_from = "String", into = "String")]
pub struct BranchName(String);

impl std::fmt::Display for BranchName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl BranchName {
    /// Create a new `BranchName`.
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

impl AsRef<str> for BranchName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for BranchName {
    type Error = ValidationError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        #[derive(Validate)]
        struct BranchNameValidator {
            #[validate(length(min = 1))]
            val: String,
        }

        let validator = BranchNameValidator { val: value.clone() };
        if let Err(e) = validator.validate() {
            tracing::warn!("Validation failed for BranchName: {}. Error: {}", value, e);
            return Err(ValidationError::InvalidValue(
                "Branch name cannot be empty".to_string(),
            ));
        }

        Ok(BranchName(value))
    }
}

impl From<BranchName> for String {
    fn from(val: BranchName) -> Self {
        val.0
    }
}

crate::derive_sqlx_traits!(BranchName);
