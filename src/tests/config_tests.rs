use crate::test_utils::create_test_config;
use std::time::Duration;
use validator::Validate;

#[test]
fn test_config_validation_auth_enabled_no_key_fails() {
    let mut config = create_test_config();
    config.auth.allow_unauthenticated = false;
    config.auth.api_key = None;

    assert!(config.validate().is_err());
}

#[test]
fn test_config_validation_auth_enabled_with_key_success() {
    let mut config = create_test_config();
    config.auth.allow_unauthenticated = false;
    config.auth.api_key = Some(crate::domain::NonEmptyString::new("secret".to_string()).unwrap());
    config.auth.token_validity = Duration::from_secs(10); // Ensure token_validity > clock_drift_buffer

    assert!(config.validate().is_ok());
}

#[test]
fn test_config_validation_server_timeouts_zero_fails() {
    let mut config = create_test_config();
    config.server.in_request_timeout = Duration::ZERO;

    assert!(config.validate().is_err());
}

#[test]
fn test_config_validation_engine_threshold_low_fails() {
    let mut config = create_test_config();
    config.engine.stuck_task_threshold = Duration::from_secs(1);
    config.engine.trigger_queue_polling_interval = Duration::from_secs(2);

    assert!(config.validate().is_err());
}

#[test]
fn test_config_validation_auth_token_validity_too_short_fails() {
    let mut config = create_test_config();
    config.auth.token_validity = Duration::from_secs(1);
    config.auth.clock_drift_buffer = Duration::from_secs(5);

    assert!(config.validate().is_err());
}
