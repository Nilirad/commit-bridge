//! Domain type to represent a GitHub's `repository_dispatch` `event_type`.

use crate::error::ValidationError;
use rovo::schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use validator::Validate;

/// The GitHub's `repository_dispatch` `event_type`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(try_from = "String", into = "String")]
pub struct EventType(String);

impl EventType {
    /// Create a new `EventType`.
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

crate::derive_sqlx_traits!(EventType);

impl std::fmt::Display for EventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TryFrom<String> for EventType {
    type Error = ValidationError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        #[derive(Validate)]
        struct EventTypeValidator {
            #[validate(length(max = 100))]
            val: String,
        }

        let validator = EventTypeValidator { val: value.clone() };
        if let Err(e) = validator.validate() {
            tracing::warn!("Validation failed for EventType: {}. Error: {}", value, e);
            return Err(ValidationError::InvalidValue(format!(
                "Event type must be at most 100 characters long. Got: {}",
                value.len()
            )));
        }

        Ok(EventType(value))
    }
}

impl From<EventType> for String {
    fn from(val: EventType) -> Self {
        val.0
    }
}
