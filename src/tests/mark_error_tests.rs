use crate::HttpRequestOnResponse;
use axum::http::StatusCode;

#[test]
fn test_should_mark_error_server_errors_always_marked() {
    assert!(HttpRequestOnResponse::new(false).should_mark_error(StatusCode::INTERNAL_SERVER_ERROR));
    assert!(HttpRequestOnResponse::new(true).should_mark_error(StatusCode::BAD_GATEWAY));
}

#[test]
fn test_should_mark_error_client_errors_opt_in() {
    assert!(!HttpRequestOnResponse::new(false).should_mark_error(StatusCode::NOT_FOUND));
    assert!(HttpRequestOnResponse::new(true).should_mark_error(StatusCode::NOT_FOUND));
}

#[test]
fn test_should_mark_error_success_and_redirects_never_marked() {
    let on_response = HttpRequestOnResponse::new(true);
    assert!(!on_response.should_mark_error(StatusCode::OK));
    assert!(!on_response.should_mark_error(StatusCode::MOVED_PERMANENTLY));
}
