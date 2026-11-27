use axum::{
    extract::Request,
    http::{Method, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use axum_extra::extract::CookieJar;

use crate::{error::ErrorResponse, middleware::full_path};

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
    let path = full_path(&req);

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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{Router, body::Body, http::Request, middleware::from_fn, routing::post};
    use tower::ServiceExt;

    #[tokio::test]
    async fn nested_public_path_is_exempt_from_csrf() {
        let inner = Router::new()
            .route("/auth/login/", post(|| async { StatusCode::UNAUTHORIZED }))
            .layer(from_fn(require_csrf));
        let app = Router::new().nest("/api/v1", inner);

        let req = Request::builder()
            .method("POST")
            .uri("/api/v1/auth/login/")
            .body(Body::empty())
            .unwrap();

        let res = app.oneshot(req).await.unwrap();
        assert_eq!(
            res.status(),
            StatusCode::UNAUTHORIZED,
            "login must reach the handler, not be blocked by CSRF"
        );
    }

    #[tokio::test]
    async fn nested_protected_path_requires_csrf() {
        let inner = Router::new()
            .route("/auth/refresh/", post(|| async { StatusCode::OK }))
            .layer(from_fn(require_csrf));
        let app = Router::new().nest("/api/v1", inner);

        let req = Request::builder()
            .method("POST")
            .uri("/api/v1/auth/refresh/")
            .body(Body::empty())
            .unwrap();

        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::FORBIDDEN);
    }
}
