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

impl sqlx::Type<sqlx::Sqlite> for BranchName {
    fn type_info() -> sqlx::sqlite::SqliteTypeInfo {
        <String as sqlx::Type<sqlx::Sqlite>>::type_info()
    }
}

impl<'r> sqlx::Decode<'r, sqlx::Sqlite> for BranchName {
    fn decode(value: sqlx::sqlite::SqliteValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
        let s = <String as sqlx::Decode<sqlx::Sqlite>>::decode(value)?;
        Ok(BranchName(s))
    }
}

impl sqlx::Encode<'_, sqlx::Sqlite> for BranchName {
    fn encode_by_ref(
        &self,
        buf: &mut Vec<sqlx::sqlite::SqliteArgumentValue<'_>>,
    ) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
        <String as sqlx::Encode<sqlx::Sqlite>>::encode_by_ref(&self.0, buf)
    }
}
