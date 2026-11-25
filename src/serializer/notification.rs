use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Serialize, ToSchema, Clone)]
pub struct NotificationResponse {
    pub id: Uuid,
    pub title: String,
    pub message: String,
    pub kind: String,
    pub is_read: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct NotificationQuery {
    pub page: Option<u64>,
    pub per_page: Option<u64>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct NotificationListResponse {
    pub notifications: Vec<NotificationResponse>,
    pub page: u64,
    pub per_page: u64,
    pub total: u64,
    pub total_pages: u64,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct MarkReadRequest {
    pub is_read: bool,
}
