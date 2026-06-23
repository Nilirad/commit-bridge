use crate::{build_router, repository::SqliteRepository, test_utils::create_test_db};
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use std::sync::Arc;
use tower::ServiceExt; // for oneshot

#[tokio::test]
async fn test_subscriber_api_routes() {
    let pool = create_test_db().await;
    let mut config = crate::test_utils::create_test_config();
    config.auth.allow_unauthenticated = true;
    let repository = Arc::new(SqliteRepository::new(pool.clone()));

    let app = build_router(repository, pool, &config);

    // Test List Subscribers (Empty)
    let response = app
        .clone()
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

    // Test Create Subscriber
    let payload = serde_json::json!({
        "source_repo_url": "https://github.com/org/repo",
        "source_branch_name": "main",
        "target_repo": "org/target",
        "event_type": "dispatch",
        "gh_app_installation_id": 1
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/subscribers")
                .method("POST")
                .header("Content-Type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Get the ID from the response (it will be 1)
    let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let body_json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let id = body_json["id"].as_i64().unwrap();
    assert_eq!(id, 1);

    // Test Get Subscriber
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/subscribers/{}", id))
                .method("GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Test Delete Subscriber
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/subscribers/{}", id))
                .method("DELETE")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Verify deletion
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/subscribers/{}", id))
                .method("GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
