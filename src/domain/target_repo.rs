//! Domain type to represent a target repository hosted on GitHub.

use crate::error::ValidationError;
use rovo::schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The target GitHub repository in owner/repo format.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(try_from = "String", into = "String")]
pub struct TargetRepo(String);

impl TargetRepo {
    /// Create a new `TargetRepo`.
    pub fn new(value: String) -> Result<Self, ValidationError> {
        Self::try_from(value)
    }

    /// Get the inner string.
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl std::fmt::Display for TargetRepo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// SQLx trait implementations
impl sqlx::Type<sqlx::Sqlite> for TargetRepo {
    fn type_info() -> sqlx::sqlite::SqliteTypeInfo {
        <String as sqlx::Type<sqlx::Sqlite>>::type_info()
    }
}

impl<'r> sqlx::Decode<'r, sqlx::Sqlite> for TargetRepo {
    fn decode(value: sqlx::sqlite::SqliteValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
        let s = <String as sqlx::Decode<sqlx::Sqlite>>::decode(value)?;
        Ok(TargetRepo(s))
    }
}

impl sqlx::Encode<'_, sqlx::Sqlite> for TargetRepo {
    fn encode_by_ref(
        &self,
        buf: &mut Vec<sqlx::sqlite::SqliteArgumentValue<'_>>,
    ) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
        <String as sqlx::Encode<sqlx::Sqlite>>::encode_by_ref(&self.0, buf)
    }
}

impl TryFrom<String> for TargetRepo {
    type Error = ValidationError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let parts: Vec<&str> = value.split('/').collect();
        match parts.as_slice() {
            [owner, repo] if !owner.is_empty() && !repo.is_empty() => Ok(TargetRepo(value)),
            _ => {
                tracing::warn!(
                    "Validation failed for TargetRepo: {}. Must be in owner/repo format",
                    value
                );
                Err(ValidationError::InvalidValue(format!(
                    "Target repository must be in owner/repo format: {}",
                    value
                )))
            }
        }
    }
}

impl From<TargetRepo> for String {
    fn from(val: TargetRepo) -> Self {
        val.0
    }
}
