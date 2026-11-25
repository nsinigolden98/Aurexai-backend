use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};

use crate::{app::AppState, error::ErrorResponse};

pub async fn enforce_allowed_hosts(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Response {
    let host = req
        .headers()
        .get(axum::http::header::HOST)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_lowercase();

    let allowed = state
        .config
        .allowed_hosts
        .iter()
        .any(|h| h.to_lowercase() == host);

    if !allowed {
        return (
            StatusCode::BAD_REQUEST,
            axum::Json(ErrorResponse {
                message: "Invalid host header".to_string(),
            }),
        )
            .into_response();
    }

    next.run(req).await
}
