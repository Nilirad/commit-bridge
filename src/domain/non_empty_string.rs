//! Domain type to represent a non-empty string.

use crate::error::ValidationError;
use serde::{Deserialize, Serialize};
use validator::Validate;

/// A string that is guaranteed to be non-empty.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(try_from = "String", into = "String")]
pub struct NonEmptyString(String);

impl NonEmptyString {
    /// Create a new `NonEmptyString`.
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

impl std::fmt::Display for NonEmptyString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for NonEmptyString {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for NonEmptyString {
    type Error = ValidationError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        #[derive(Validate)]
        struct NonEmptyStringValidator {
            #[validate(length(min = 1))]
            val: String,
        }

        let validator = NonEmptyStringValidator { val: value.clone() };
        if let Err(e) = validator.validate() {
            tracing::warn!("Validation failed for NonEmptyString. Error: {}", e);
            return Err(ValidationError::InvalidValue(format!(
                "String must not be empty: {}",
                value
            )));
        }

        Ok(NonEmptyString(value))
    }
}

impl std::ops::Deref for NonEmptyString {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl PartialEq<str> for NonEmptyString {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}

impl PartialEq<NonEmptyString> for str {
    fn eq(&self, other: &NonEmptyString) -> bool {
        self == other.0
    }
}

impl PartialEq<String> for NonEmptyString {
    fn eq(&self, other: &String) -> bool {
        &self.0 == other
    }
}

impl PartialEq<NonEmptyString> for String {
    fn eq(&self, other: &NonEmptyString) -> bool {
        self == &other.0
    }
}

impl From<NonEmptyString> for String {
    fn from(val: NonEmptyString) -> Self {
        val.0
    }
}
