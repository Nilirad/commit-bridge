//! Domain type to represent a git commit hash.

use crate::error::ValidationError;
use rovo::schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A git commit hash.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(try_from = "String", into = "String")]
pub struct CommitHash(String);

impl CommitHash {
    /// Create a new `CommitHash`.
    pub fn new(value: String) -> Result<Self, ValidationError> {
        Self::try_from(value)
    }

    /// Get the inner string.
    pub fn into_inner(self) -> String {
        self.0
    }

    /// Get the string as a slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

crate::derive_sqlx_traits!(CommitHash);

impl AsRef<str> for CommitHash {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for CommitHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TryFrom<String> for CommitHash {
    type Error = ValidationError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let len = value.len();
        if len != 40 && len != 64 {
            tracing::warn!(
                "Validation failed for CommitHash: {}. Invalid length: {}",
                value,
                len
            );
            return Err(ValidationError::InvalidValue(format!(
                "Commit hash must be 40 or 64 characters long. Got: {}",
                len
            )));
        }

        if !value.chars().all(|c| c.is_ascii_hexdigit()) {
            tracing::warn!(
                "Validation failed for CommitHash: {}. Non-hex characters",
                value
            );
            return Err(ValidationError::InvalidValue(format!(
                "Commit hash must be a hexadecimal string. Got: {}",
                value
            )));
        }

        Ok(CommitHash(value))
    }
}

impl From<CommitHash> for String {
    fn from(val: CommitHash) -> Self {
        val.0
    }
}

impl std::ops::Deref for CommitHash {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl PartialEq<str> for CommitHash {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}

impl PartialEq<CommitHash> for str {
    fn eq(&self, other: &CommitHash) -> bool {
        self == other.0
    }
}

impl PartialEq<String> for CommitHash {
    fn eq(&self, other: &String) -> bool {
        &self.0 == other
    }
}

impl PartialEq<CommitHash> for String {
    fn eq(&self, other: &CommitHash) -> bool {
        self == &other.0
    }
}
