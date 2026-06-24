use crate::{
    build_router, domain::NonEmptyString, repository::SqliteRepository, test_utils::create_test_db,
};
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use std::sync::Arc;
use tower::ServiceExt; // for oneshot

#[tokio::test]
async fn test_auth_no_key_configured_fails() {
    let pool = create_test_db().await;
    let mut config = crate::test_utils::create_test_config();
    config.auth.api_key = None;
    config.auth.allow_unauthenticated = false;

    let repository = Arc::new(SqliteRepository::new(pool.clone()));
    let app = build_router(repository, pool, &config);

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

#[tokio::test]
async fn test_auth_allowed_unauthenticated_success() {
    let pool = create_test_db().await;
    let mut config = crate::test_utils::create_test_config();
    config.auth.api_key = None;
    config.auth.allow_unauthenticated = true;

    let repository = Arc::new(SqliteRepository::new(pool.clone()));
    let app = build_router(repository, pool, &config);

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

    let repository = Arc::new(SqliteRepository::new(pool.clone()));
    let app = build_router(repository, pool, &config);

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

    let repository = Arc::new(SqliteRepository::new(pool.clone()));
    let app = build_router(repository, pool, &config);

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

    let repository = Arc::new(SqliteRepository::new(pool.clone()));
    let app = build_router(repository, pool, &config);

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
