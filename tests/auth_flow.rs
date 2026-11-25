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
use common::{body_json, cookie_header, get_csrf, get_set_cookies, setup};

#[tokio::test]
#[serial]
async fn auth_flow_signup_confirm_login_me_refresh_logout() {
    let Some((app, db)) = setup().await else {
        return;
    };

    // Signup
    let signup_req = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/signup/")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({
                "name": "John",
                "surname": "Doe",
                "email": "john@example.com",
                "password": "password123",
                "confirm_password": "password123"
            })
            .to_string(),
        ))
        .unwrap();
    let signup_res = app.clone().oneshot(signup_req).await.unwrap();
    assert_eq!(signup_res.status(), StatusCode::CREATED);

    // Get user and set a known OTP hash for testing
    let user_model = user::Entity::find()
        .filter(user::Column::Email.eq("john@example.com"))
        .one(&db)
        .await
        .unwrap()
        .expect("user exists");

    let otp_model = otp::Entity::find()
        .filter(otp::Column::UserId.eq(user_model.id))
        .one(&db)
        .await
        .unwrap()
        .expect("otp exists");

    let mut otp_active: otp::ActiveModel = otp_model.into();
    otp_active.code_hash = sea_orm::Set(sha256_hex("123456"));
    otp::Entity::update(otp_active).exec(&db).await.unwrap();

    // Confirm OTP
    let confirm_req = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/confirm-otp/")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({"email":"john@example.com","code":"123456"}).to_string(),
        ))
        .unwrap();
    let confirm_res = app.clone().oneshot(confirm_req).await.unwrap();
    assert_eq!(confirm_res.status(), StatusCode::OK);
    let cookies = get_set_cookies(&confirm_res);
    let cookie = cookie_header(&cookies);
    let csrf = get_csrf(&cookies).expect("csrf cookie");
    let body = body_json(confirm_res.into_body()).await;
    assert_eq!(body["user"]["email"], "john@example.com");
    assert!(body["user"]["referral_code"].as_str().is_some());

    // Me
    let me_req = Request::builder()
        .method("GET")
        .uri("/api/v1/auth/me/")
        .header(header::COOKIE, &cookie)
        .body(Body::empty())
        .unwrap();
    let me_res = app.clone().oneshot(me_req).await.unwrap();
    assert_eq!(me_res.status(), StatusCode::OK);

    // Refresh (needs CSRF)
    let refresh_req = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/refresh/")
        .header(header::COOKIE, &cookie)
        .header("x-csrf-token", &csrf)
        .body(Body::empty())
        .unwrap();
    let refresh_res = app.clone().oneshot(refresh_req).await.unwrap();
    assert_eq!(refresh_res.status(), StatusCode::OK);
    let refresh_cookies = get_set_cookies(&refresh_res);
    let refresh_cookie = cookie_header(&refresh_cookies);
    let refresh_csrf = get_csrf(&refresh_cookies).unwrap_or(csrf);

    // Logout (needs CSRF)
    let logout_req = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/logout/")
        .header(header::COOKIE, &refresh_cookie)
        .header("x-csrf-token", &refresh_csrf)
        .body(Body::empty())
        .unwrap();
    let logout_res = app.clone().oneshot(logout_req).await.unwrap();
    assert_eq!(logout_res.status(), StatusCode::OK);
}
