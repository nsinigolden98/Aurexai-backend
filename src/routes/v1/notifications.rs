use axum::{
    Json, Router,
    extract::{
        Path, Query, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::IntoResponse,
    routing::{get, post},
};
use futures::{SinkExt, StreamExt};
use uuid::Uuid;

use crate::{
    app::AppState,
    error::{AppError, ErrorResponse},
    middleware::auth::CurrentUser,
    serializer::notification::{
        MarkReadRequest, NotificationListResponse, NotificationQuery, NotificationResponse,
    },
    services::notification::NotificationService,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_notifications))
        .route("/ws/", get(ws_notifications))
        .route("/read-all/", post(mark_all_read))
        .route("/{id}/", axum::routing::patch(mark_read))
}

#[utoipa::path(
    get,
    path = "/api/v1/notifications/",
    tag = "Notifications",
    params(
        ("page" = Option<u64>, Query, description = "Page number (default: 1)"),
        ("per_page" = Option<u64>, Query, description = "Items per page (default: 20, max: 100)")
    ),
    responses(
        (status = 200, description = "List notifications for current user", body = NotificationListResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    )
)]
pub async fn list_notifications(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Query(query): Query<NotificationQuery>,
) -> Result<Json<NotificationListResponse>, AppError> {
    let page = query.page.unwrap_or(1);
    let per_page = query.per_page.unwrap_or(20);
    let notifications =
        NotificationService::list_for_user(&state.db, user.id, page, per_page).await?;
    Ok(Json(notifications))
}

#[utoipa::path(
    patch,
    path = "/api/v1/notifications/{id}/",
    tag = "Notifications",
    params(("id" = Uuid, Path, description = "Notification ID")),
    request_body = MarkReadRequest,
    responses(
        (status = 200, description = "Notification updated", body = NotificationResponse),
        (status = 404, description = "Notification not found", body = ErrorResponse)
    )
)]
pub async fn mark_read(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<Uuid>,
    Json(payload): Json<MarkReadRequest>,
) -> Result<Json<NotificationResponse>, AppError> {
    let notification =
        NotificationService::mark_read(&state.db, user.id, id, payload.is_read).await?;
    Ok(Json(notification))
}

#[utoipa::path(
    post,
    path = "/api/v1/notifications/read-all/",
    tag = "Notifications",
    responses(
        (status = 200, description = "All notifications marked as read", body = crate::serializer::auth::MessageResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    )
)]
pub async fn mark_all_read(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
) -> Result<Json<crate::serializer::auth::MessageResponse>, AppError> {
    NotificationService::mark_all_read(&state.db, user.id).await?;
    Ok(Json(crate::serializer::auth::MessageResponse {
        message: "All notifications marked as read".to_string(),
    }))
}

#[utoipa::path(
    get,
    path = "/api/v1/notifications/ws/",
    tag = "Notifications",
    responses(
        (status = 101, description = "WebSocket upgrade. Authenticate via access_token cookie. Server sends JSON notification objects as text messages."),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    )
)]
pub async fn ws_notifications(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state, user.id))
}

async fn handle_socket(socket: WebSocket, state: AppState, user_id: Uuid) {
    let (mut sender, mut receiver) = socket.split();
    let mut rx = state.notification_manager.subscribe(user_id).await;

    let mut send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            if sender.send(Message::Text(msg.into())).await.is_err() {
                break;
            }
        }
    });

    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                Message::Close(_) => break,
                Message::Ping(_) | Message::Pong(_) | Message::Text(_) | Message::Binary(_) => {}
            }
        }
    });

    tokio::select! {
        _ = &mut send_task => recv_task.abort(),
        _ = &mut recv_task => send_task.abort(),
    }
}
