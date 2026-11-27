use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use axum::{
    extract::{ConnectInfo, Request},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};

use crate::{error::ErrorResponse, middleware::full_path};

#[derive(Clone)]
pub struct RateLimiter {
    state: Arc<Mutex<HashMap<String, (Instant, u32)>>>,
    limit: u32,
    window: Duration,
}

impl RateLimiter {
    pub fn new(limit: u32, window: Duration) -> Self {
        Self {
            state: Arc::new(Mutex::new(HashMap::new())),
            limit,
            window,
        }
    }

    fn check(&self, key: &str) -> bool {
        let mut map = self.state.lock().expect("rate limiter mutex poisoned");
        let now = Instant::now();
        let entry = map.entry(key.to_string()).or_insert((now, 0));
        if now.duration_since(entry.0) >= self.window {
            *entry = (now, 1);
            return true;
        }
        if entry.1 >= self.limit {
            return false;
        }
        entry.1 += 1;
        true
    }
}

pub fn client_ip(req: &Request) -> String {
    if let Some(forwarded) = req
        .headers()
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        && let Some(first) = forwarded.split(',').next()
    {
        let ip = first.trim();
        if !ip.is_empty() {
            return ip.to_string();
        }
    }
    if let Some(ConnectInfo(addr)) = req.extensions().get::<ConnectInfo<SocketAddr>>() {
        return addr.ip().to_string();
    }
    "unknown".to_string()
}

pub async fn limit_sensitive_auth(req: Request, next: Next) -> Response {
    static LIMITER: std::sync::OnceLock<RateLimiter> = std::sync::OnceLock::new();
    let limiter = LIMITER.get_or_init(|| RateLimiter::new(10, Duration::from_secs(60)));

    let path = full_path(&req);
    let sensitive = matches!(
        path.as_str(),
        "/api/v1/auth/signup/"
            | "/api/v1/auth/login/"
            | "/api/v1/auth/confirm-otp/"
            | "/api/v1/auth/resend-otp/"
            | "/api/v1/auth/forgot-password/"
            | "/api/v1/auth/reset-password/"
            | "/api/v1/auth/google/"
    );

    if !sensitive {
        return next.run(req).await;
    }

    let ip = client_ip(&req);
    let key = format!("{ip}:{path}");
    if !limiter.check(&key) {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            axum::Json(ErrorResponse {
                message: "Too many requests. Please try again later.".to_string(),
            }),
        )
            .into_response();
    }

    next.run(req).await
}
