#![allow(dead_code)]

use std::sync::Arc;

use axum::{body::Body, http::header};
use http_body_util::BodyExt;
use migration::MigratorTrait;
use sea_orm::EntityTrait;
use serde_json::Value;

use aurexai_backend::{
    app::{AppState, create_router},
    config::Config,
    db,
    models::otp,
    services::email::EmailService,
};

pub fn test_config(database_url: String) -> Config {
    Config {
        app_env: "test".to_string(),
        app_host: "127.0.0.1".to_string(),
        app_port: 0,
        database_url,
        frontend_origin: "http://localhost:3000".to_string(),
        frontend_reset_password_url: "http://localhost:3000/reset-password".to_string(),
        allowed_origins: vec!["http://localhost:3000".to_string()],
        allowed_hosts: vec!["localhost:8080".to_string(), "127.0.0.1:8080".to_string()],
        access_token_secret: "test-access-secret".to_string(),
        refresh_token_secret: "test-refresh-secret".to_string(),
        access_token_ttl_minutes: 15,
        refresh_token_ttl_days: 7,
        cookie_secure: false,
        cookie_samesite: "Lax".to_string(),
        cookie_domain: None,
        brevo_api_key: "".to_string(),
        brevo_sender_email: "test@example.com".to_string(),
        brevo_sender_name: "Test".to_string(),
        google_client_id: "".to_string(),
        reset_token_ttl_minutes: 30,
        otp_ttl_minutes: 10,
        otp_resend_cooldown_seconds: 60,
        otp_max_attempts: 5,
    }
}

pub async fn setup() -> Option<(axum::Router, sea_orm::DatabaseConnection)> {
    let Ok(database_url) = std::env::var("TEST_DATABASE_URL") else {
        eprintln!("skipping integration test: TEST_DATABASE_URL not set");
        return None;
    };

    let config = test_config(database_url);
    let db = db::connect(&config.database_url).await.expect("db connect");
    migration::Migrator::up(&db, None)
        .await
        .expect("migrations");

    // Clean tables for deterministic runs
    let _ = password_reset_tokens_cleanup(&db).await;
    let _ = refresh_tokens_cleanup(&db).await;
    let _ = otps_cleanup(&db).await;
    let _ = users_cleanup(&db).await;

    let state = AppState {
        db: db.clone(),
        config: Arc::new(config.clone()),
        email_service: EmailService::new(&config),
        notification_manager: aurexai_backend::services::notification::NotificationManager::new(),
    };

    Some((create_router(state), db))
}

pub async fn users_cleanup(db: &sea_orm::DatabaseConnection) {
    use aurexai_backend::models::user;
    let users = user::Entity::find().all(db).await.unwrap_or_default();
    for u in users {
        let _ = user::Entity::delete_by_id(u.id).exec(db).await;
    }
}
pub async fn otps_cleanup(db: &sea_orm::DatabaseConnection) {
    let rows = otp::Entity::find().all(db).await.unwrap_or_default();
    for r in rows {
        let _ = otp::Entity::delete_by_id(r.id).exec(db).await;
    }
}
pub async fn refresh_tokens_cleanup(db: &sea_orm::DatabaseConnection) {
    use aurexai_backend::models::refresh_token;
    let rows = refresh_token::Entity::find()
        .all(db)
        .await
        .unwrap_or_default();
    for r in rows {
        let _ = refresh_token::Entity::delete_by_id(r.id).exec(db).await;
    }
}
pub async fn password_reset_tokens_cleanup(db: &sea_orm::DatabaseConnection) {
    use aurexai_backend::models::password_reset_token;
    let rows = password_reset_token::Entity::find()
        .all(db)
        .await
        .unwrap_or_default();
    for r in rows {
        let _ = password_reset_token::Entity::delete_by_id(r.id)
            .exec(db)
            .await;
    }
}

pub async fn body_json(body: Body) -> Value {
    let bytes = body.collect().await.expect("body").to_bytes();
    serde_json::from_slice(&bytes).expect("json")
}

pub fn cookie_header(set_cookie_headers: &[String]) -> String {
    set_cookie_headers
        .iter()
        .filter_map(|v| v.split(';').next())
        .collect::<Vec<_>>()
        .join("; ")
}

pub fn get_set_cookies(response: &axum::http::Response<Body>) -> Vec<String> {
    response
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok().map(|s| s.to_string()))
        .collect()
}

pub fn get_csrf(set_cookies: &[String]) -> Option<String> {
    for c in set_cookies {
        if let Some(rest) = c.strip_prefix("csrf_token=") {
            return rest.split(';').next().map(|s| s.to_string());
        }
    }
    None
}
