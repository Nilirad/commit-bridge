use crate::{build_router, domain::NonEmptyString, test_utils::create_test_db};
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use tower::ServiceExt; // for oneshot

#[tokio::test]
async fn test_auth_no_key_configured() {
    let pool = create_test_db().await;
    let mut config = crate::test_utils::create_test_config();
    config.auth.api_key = None;

    let app = build_router(pool, &config);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/subscribers")
                .method("GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_auth_key_configured_success() {
    let pool = create_test_db().await;
    let mut config = crate::test_utils::create_test_config();
    config.auth.api_key = Some(NonEmptyString::new("secret".to_string()).unwrap());

    let app = build_router(pool, &config);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/subscribers")
                .method("GET")
                .header("X-API-KEY", "secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_auth_key_configured_mismatch() {
    let pool = create_test_db().await;
    let mut config = crate::test_utils::create_test_config();
    config.auth.api_key = Some(NonEmptyString::new("secret".to_string()).unwrap());

    let app = build_router(pool, &config);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/subscribers")
                .method("GET")
                .header("X-API-KEY", "wrong")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_auth_key_configured_missing() {
    let pool = create_test_db().await;
    let mut config = crate::test_utils::create_test_config();
    config.auth.api_key = Some(NonEmptyString::new("secret".to_string()).unwrap());

    let app = build_router(pool, &config);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/subscribers")
                .method("GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
