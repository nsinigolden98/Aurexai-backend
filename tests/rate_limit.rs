mod common;

use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use serde_json::json;
use serial_test::serial;
use tower::ServiceExt;

use common::setup;

#[tokio::test]
#[serial]
async fn rate_limit_blocks_excessive_login_attempts() {
    let Some((app, _db)) = setup().await else {
        return;
    };

    for i in 0..10 {
        let req = Request::builder()
            .method("POST")
            .uri("/api/v1/auth/login/")
            .header(header::CONTENT_TYPE, "application/json")
            .header("x-forwarded-for", "203.0.113.10")
            .body(Body::from(
                json!({"email":"nobody@example.com","password":"password123"}).to_string(),
            ))
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(
            res.status(),
            StatusCode::UNAUTHORIZED,
            "request {i} should be unauthorized, not rate limited"
        );
    }

    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/login/")
        .header(header::CONTENT_TYPE, "application/json")
        .header("x-forwarded-for", "203.0.113.10")
        .body(Body::from(
            json!({"email":"nobody@example.com","password":"password123"}).to_string(),
        ))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::TOO_MANY_REQUESTS);
}
