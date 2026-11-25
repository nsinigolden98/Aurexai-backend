use axum::Router;

use crate::app::AppState;

pub mod admin;
pub mod auth;
pub mod notifications;

pub fn router() -> Router<AppState> {
    Router::new()
        .nest("/auth", auth::router())
        .nest("/admin", admin::router())
        .nest("/notifications", notifications::router())
}
