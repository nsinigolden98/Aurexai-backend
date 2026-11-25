use std::sync::Arc;

use axum::{
    Json, Router,
    http::{HeaderValue, Method},
    routing::get,
};
use sea_orm::DatabaseConnection;
use serde::Serialize;
use tower_http::{
    cors::{AllowOrigin, CorsLayer},
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};
use utoipa::OpenApi;
// Redoc served via custom HTML endpoint.
use utoipa_swagger_ui::SwaggerUi;

use crate::{
    config::Config,
    error::ErrorResponse,
    routes,
    services::{email::EmailService, notification::NotificationManager},
};

#[derive(Clone)]
pub struct AppState {
    pub db: DatabaseConnection,
    pub config: Arc<Config>,
    pub email_service: EmailService,
    pub notification_manager: NotificationManager,
}

#[derive(OpenApi)]
#[openapi(
    paths(
        health,
        crate::routes::v1::auth::signup,
        crate::routes::v1::auth::confirm_otp,
        crate::routes::v1::auth::resend_otp,
        crate::routes::v1::auth::login,
        crate::routes::v1::auth::refresh,
        crate::routes::v1::auth::logout,
        crate::routes::v1::auth::forgot_password,
        crate::routes::v1::auth::reset_password,
        crate::routes::v1::auth::google_auth,
        crate::routes::v1::auth::me,
        crate::routes::v1::auth::csrf_token,
        crate::routes::v1::admin::list_users,
        crate::routes::v1::admin::get_user,
        crate::routes::v1::admin::update_user,
        crate::routes::v1::notifications::list_notifications,
        crate::routes::v1::notifications::mark_read,
        crate::routes::v1::notifications::mark_all_read,
        crate::routes::v1::notifications::ws_notifications,
    ),
    components(schemas(
        ErrorResponse,
        HealthResponse,
        crate::serializer::auth::SignupRequest,
        crate::serializer::auth::LoginRequest,
        crate::serializer::auth::ConfirmOtpRequest,
        crate::serializer::auth::ResendOtpRequest,
        crate::serializer::auth::ForgotPasswordRequest,
        crate::serializer::auth::ResetPasswordRequest,
        crate::serializer::auth::GoogleAuthRequest,
        crate::serializer::auth::MessageResponse,
        crate::serializer::auth::AuthUserResponse,
        crate::serializer::auth::AuthResponse,
        crate::serializer::admin::AdminUserResponse,
        crate::serializer::admin::AdminUsersListResponse,
        crate::serializer::admin::UpdateUserRoleRequest,
        crate::serializer::notification::NotificationResponse,
        crate::serializer::notification::NotificationListResponse,
        crate::serializer::notification::NotificationQuery,
        crate::serializer::notification::MarkReadRequest,
    )),
    tags(
        (name = "Health", description = "Health check endpoints"),
        (name = "Auth", description = "Authentication endpoints"),
        (name = "Admin", description = "Admin endpoints"),
        (name = "Notifications", description = "In-app notifications and WebSocket stream. Connect to /api/v1/notifications/ws/ with an authenticated browser/session cookie; the server pushes JSON notification objects as text frames.")
    )
)]
pub struct ApiDoc;

#[derive(Serialize, utoipa::ToSchema)]
pub struct HealthResponse {
    pub status: String,
}

#[utoipa::path(
    get,
    path = "/api/v1/health/",
    tag = "Health",
    responses(
        (status = 200, description = "Service is healthy", body = HealthResponse)
    )
)]
pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
    })
}

pub async fn redoc() -> axum::response::Html<&'static str> {
    axum::response::Html(
        r#"<!DOCTYPE html>
<html>
  <head>
    <title>Aurexai API Docs</title>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <style>body { margin: 0; padding: 0; }</style>
  </head>
  <body>
    <redoc spec-url="/api-docs/openapi.json"></redoc>
    <script src="https://cdn.redoc.ly/redoc/latest/bundles/redoc.standalone.js"></script>
  </body>
</html>"#,
    )
}

pub fn create_router(state: AppState) -> Router {
    let allowed_origins: Vec<HeaderValue> = state
        .config
        .allowed_origins
        .iter()
        .filter_map(|o| o.parse::<HeaderValue>().ok())
        .collect();

    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::list(allowed_origins))
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            axum::http::header::ACCEPT,
            axum::http::header::AUTHORIZATION,
            axum::http::header::CONTENT_TYPE,
            axum::http::header::ORIGIN,
            axum::http::HeaderName::from_static("x-csrf-token"),
        ])
        .allow_credentials(true);

    let api_v1 = Router::new()
        .route("/health/", get(health))
        .merge(routes::v1::router())
        .layer(axum::middleware::from_fn(
            crate::middleware::csrf::require_csrf,
        ))
        .layer(axum::middleware::from_fn(
            crate::middleware::rate_limit::limit_sensitive_auth,
        ));

    let mut router = Router::new()
        .merge(SwaggerUi::new("/docs").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .route("/redoc", get(redoc))
        .route("/redoc/", get(redoc))
        .nest("/api/v1", api_v1);

    let admin_dist = std::path::Path::new("admin/dist");
    if admin_dist.exists() {
        let index = admin_dist.join("index.html");
        let serve_admin = ServeDir::new(admin_dist).not_found_service(ServeFile::new(index));
        router = router.nest_service("/admin", serve_admin);
    }

    router
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            crate::middleware::allowed_hosts::enforce_allowed_hosts,
        ))
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state)
}
