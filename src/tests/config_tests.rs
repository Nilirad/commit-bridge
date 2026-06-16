use crate::test_utils::create_test_config;
use validator::Validate;

#[test]
fn test_config_validation_auth_enabled_no_key_fails() {
    let mut config = create_test_config();
    config.auth.allow_unauthenticated = false;
    config.auth.api_key = None;

    let result = config.validate();
    assert!(result.is_err());
}

#[test]
fn test_config_validation_auth_enabled_with_key_success() {
    let mut config = create_test_config();
    config.auth.allow_unauthenticated = false;
    config.auth.api_key = Some(crate::domain::NonEmptyString::new("secret".to_string()).unwrap());

    let result = config.validate();
    assert!(result.is_ok());
}
