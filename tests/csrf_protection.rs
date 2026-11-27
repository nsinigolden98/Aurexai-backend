mod common;

use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::json;
use serial_test::serial;
use tower::ServiceExt;

use aurexai_backend::{
    models::{otp, user},
    utils::hash::sha256_hex,
};
use common::{cookie_header, get_set_cookies, setup};

#[tokio::test]
#[serial]
async fn csrf_blocks_unsafe_authenticated_requests_without_header() {
    let Some((app, db)) = setup().await else {
        return;
    };

    let signup_req = Request::builder().method("POST").uri("/api/v1/auth/signup/")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({"name":"A","surname":"B","email":"csrf@example.com","password":"password123","confirm_password":"password123"}).to_string())).unwrap();
    let _ = app.clone().oneshot(signup_req).await.unwrap();
    let user_model = user::Entity::find()
        .filter(user::Column::Email.eq("csrf@example.com"))
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    let otp_model = otp::Entity::find()
        .filter(otp::Column::UserId.eq(user_model.id))
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    let mut otp_active: otp::ActiveModel = otp_model.into();
    otp_active.code_hash = sea_orm::Set(sha256_hex("123456"));
    otp::Entity::update(otp_active).exec(&db).await.unwrap();
    let confirm_req = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/confirm-otp/")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({"email":"csrf@example.com","code":"123456"}).to_string(),
        ))
        .unwrap();
    let confirm_res = app.clone().oneshot(confirm_req).await.unwrap();
    let cookie = cookie_header(&get_set_cookies(&confirm_res));

    // Refresh without CSRF header should fail
    let refresh_req = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/refresh/")
        .header(header::COOKIE, &cookie)
        .body(Body::empty())
        .unwrap();
    let refresh_res = app.clone().oneshot(refresh_req).await.unwrap();
    assert_eq!(refresh_res.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
#[serial]
async fn login_is_not_blocked_by_csrf() {
    let Some((app, _db)) = setup().await else {
        return;
    };

    // Login is a public auth endpoint: no csrf cookie or header should still
    // reach the handler and return 401 for invalid credentials, never 403.
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/login/")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({"email":"no-one@example.com","password":"password123"}).to_string(),
        ))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(
        res.status(),
        StatusCode::UNAUTHORIZED,
        "login must not require a CSRF token"
    );
}
