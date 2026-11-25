use axum::{
    extract::Request,
    http::{Method, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use axum_extra::extract::CookieJar;

use crate::error::ErrorResponse;

const CSRF_COOKIE: &str = "csrf_token";
const CSRF_HEADER: &str = "x-csrf-token";

fn is_public_auth_path(path: &str) -> bool {
    matches!(
        path,
        "/api/v1/auth/signup/"
            | "/api/v1/auth/login/"
            | "/api/v1/auth/confirm-otp/"
            | "/api/v1/auth/resend-otp/"
            | "/api/v1/auth/forgot-password/"
            | "/api/v1/auth/reset-password/"
            | "/api/v1/auth/google/"
    )
}

pub async fn require_csrf(req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_string();

    let is_unsafe = matches!(
        method,
        Method::POST | Method::PUT | Method::PATCH | Method::DELETE
    );

    if !is_unsafe || is_public_auth_path(&path) {
        return next.run(req).await;
    }

    let jar = CookieJar::from_headers(req.headers());
    let csrf_cookie = jar.get(CSRF_COOKIE).map(|c| c.value().to_string());
    let csrf_header = req
        .headers()
        .get(CSRF_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    match (csrf_cookie, csrf_header) {
        (Some(cookie), Some(header)) if !cookie.is_empty() && cookie == header => {
            next.run(req).await
        }
        _ => (
            StatusCode::FORBIDDEN,
            axum::Json(ErrorResponse {
                message: "Invalid or missing CSRF token".to_string(),
            }),
        )
            .into_response(),
    }
}
