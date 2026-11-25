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
async fn admin_guard_allows_only_staff_or_admin() {
    let Some((app, db)) = setup().await else {
        return;
    };

    // Create and verify user via signup+confirm
    let signup_req = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/signup/")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({
                "name":"Admin","surname":"User","email":"admin@example.com",
                "password":"password123","confirm_password":"password123"
            })
            .to_string(),
        ))
        .unwrap();
    let _ = app.clone().oneshot(signup_req).await.unwrap();

    let user_model = user::Entity::find()
        .filter(user::Column::Email.eq("admin@example.com"))
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
            json!({"email":"admin@example.com","code":"123456"}).to_string(),
        ))
        .unwrap();
    let confirm_res = app.clone().oneshot(confirm_req).await.unwrap();
    let cookies = get_set_cookies(&confirm_res);
    let cookie = cookie_header(&cookies);

    // Non-admin should be forbidden
    let req = Request::builder()
        .method("GET")
        .uri("/api/v1/admin/users/")
        .header(header::COOKIE, &cookie)
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // Promote to admin
    let mut active: user::ActiveModel = user_model.into();
    active.is_admin = sea_orm::Set(true);
    user::Entity::update(active).exec(&db).await.unwrap();

    let req = Request::builder()
        .method("GET")
        .uri("/api/v1/admin/users/")
        .header(header::COOKIE, &cookie)
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}
