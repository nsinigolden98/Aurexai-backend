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
use common::setup;

#[tokio::test]
#[serial]
async fn forgot_and_reset_password_flow() {
    let Some((app, db)) = setup().await else {
        return;
    };

    let signup_req = Request::builder().method("POST").uri("/api/v1/auth/signup/")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({"name":"Reset","surname":"User","email":"reset@example.com","password":"password123","confirm_password":"password123"}).to_string())).unwrap();
    let _ = app.clone().oneshot(signup_req).await.unwrap();

    let user_model = user::Entity::find()
        .filter(user::Column::Email.eq("reset@example.com"))
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
            json!({"email":"reset@example.com","code":"123456"}).to_string(),
        ))
        .unwrap();
    let _ = app.clone().oneshot(confirm_req).await.unwrap();

    // Forgot password
    let forgot_req = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/forgot-password/")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({"email":"reset@example.com"}).to_string()))
        .unwrap();
    let forgot_res = app.clone().oneshot(forgot_req).await.unwrap();
    assert_eq!(forgot_res.status(), StatusCode::OK);

    // Replace reset token hash with known token
    use aurexai_backend::models::password_reset_token;
    let prt = password_reset_token::Entity::find()
        .filter(password_reset_token::Column::UserId.eq(user_model.id))
        .one(&db)
        .await
        .unwrap()
        .expect("reset token");
    let mut prt_active: password_reset_token::ActiveModel = prt.into();
    prt_active.token_hash = sea_orm::Set(sha256_hex("known-reset-token"));
    password_reset_token::Entity::update(prt_active)
        .exec(&db)
        .await
        .unwrap();

    // Reset password
    let reset_req = Request::builder().method("POST").uri("/api/v1/auth/reset-password/")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({"token":"known-reset-token","password":"newpassword123","confirm_password":"newpassword123"}).to_string())).unwrap();
    let reset_res = app.clone().oneshot(reset_req).await.unwrap();
    assert_eq!(reset_res.status(), StatusCode::OK);

    // Login with new password
    let login_req = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/login/")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({"email":"reset@example.com","password":"newpassword123"}).to_string(),
        ))
        .unwrap();
    let login_res = app.clone().oneshot(login_req).await.unwrap();
    assert_eq!(login_res.status(), StatusCode::OK);
}
