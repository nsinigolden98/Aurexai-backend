use axum::extract::{OriginalUri, Request};

pub mod allowed_hosts;
pub mod auth;
pub mod csrf;
pub mod rate_limit;

/// Full request path, including any prefixes stripped by `Router::nest`.
///
/// Axum rewrites `req.uri()` inside nested routers, so middleware attached to a
/// nested router must read `OriginalUri` to match full paths like
/// `/api/v1/auth/login/`.
pub(crate) fn full_path(req: &Request) -> String {
    req.extensions()
        .get::<OriginalUri>()
        .map(|uri| uri.0.path().to_string())
        .unwrap_or_else(|| req.uri().path().to_string())
}
