use std::{collections::HashMap, sync::Arc};

use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, Set,
};
use tokio::sync::{Mutex, broadcast};
use uuid::Uuid;

use crate::{
    error::AppError, models::notification, serializer::notification::NotificationResponse,
};

#[derive(Clone, Default)]
pub struct NotificationManager {
    channels: Arc<Mutex<HashMap<Uuid, broadcast::Sender<String>>>>,
}

impl NotificationManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn subscribe(&self, user_id: Uuid) -> broadcast::Receiver<String> {
        let mut channels = self.channels.lock().await;
        if let Some(sender) = channels.get(&user_id) {
            return sender.subscribe();
        }
        let (tx, rx) = broadcast::channel(100);
        channels.insert(user_id, tx);
        rx
    }

    pub async fn publish(&self, user_id: Uuid, payload: &str) {
        let channels = self.channels.lock().await;
        if let Some(sender) = channels.get(&user_id) {
            let _ = sender.send(payload.to_string());
        }
    }
}

pub struct NotificationService;

impl NotificationService {
    pub async fn create_and_publish(
        db: &DatabaseConnection,
        manager: &NotificationManager,
        user_id: Uuid,
        title: &str,
        message: &str,
        kind: &str,
    ) -> Result<notification::Model, AppError> {
        let now = Utc::now();
        let model = notification::ActiveModel {
            id: Set(Uuid::new_v4()),
            user_id: Set(user_id),
            title: Set(title.to_string()),
            message: Set(message.to_string()),
            kind: Set(kind.to_string()),
            is_read: Set(false),
            created_at: Set(now),
        };
        let created = model
            .insert(db)
            .await
            .map_err(|e| AppError::internal(format!("db error: {e}")))?;

        let payload = serde_json::to_string(&to_response(&created))
            .map_err(|e| AppError::internal(format!("serialization error: {e}")))?;
        manager.publish(user_id, &payload).await;

        Ok(created)
    }

    pub async fn list_for_user(
        db: &DatabaseConnection,
        user_id: Uuid,
        page: u64,
        per_page: u64,
    ) -> Result<crate::serializer::notification::NotificationListResponse, AppError> {
        let page = page.max(1);
        let per_page = per_page.clamp(1, 100);

        let paginator = notification::Entity::find()
            .filter(notification::Column::UserId.eq(user_id))
            .order_by_desc(notification::Column::CreatedAt)
            .paginate(db, per_page);

        let total = paginator
            .num_items()
            .await
            .map_err(|e| AppError::internal(format!("db error: {e}")))?;
        let total_pages = paginator
            .num_pages()
            .await
            .map_err(|e| AppError::internal(format!("db error: {e}")))?;
        let rows = paginator
            .fetch_page(page - 1)
            .await
            .map_err(|e| AppError::internal(format!("db error: {e}")))?;

        Ok(crate::serializer::notification::NotificationListResponse {
            notifications: rows.iter().map(to_response).collect(),
            page,
            per_page,
            total,
            total_pages,
        })
    }

    pub async fn mark_read(
        db: &DatabaseConnection,
        user_id: Uuid,
        notification_id: Uuid,
        is_read: bool,
    ) -> Result<NotificationResponse, AppError> {
        let model = notification::Entity::find_by_id(notification_id)
            .filter(notification::Column::UserId.eq(user_id))
            .one(db)
            .await
            .map_err(|e| AppError::internal(format!("db error: {e}")))?
            .ok_or_else(|| AppError::not_found("Notification not found"))?;

        let mut active: notification::ActiveModel = model.into();
        active.is_read = Set(is_read);
        let updated = active
            .update(db)
            .await
            .map_err(|e| AppError::internal(format!("db error: {e}")))?;
        Ok(to_response(&updated))
    }

    pub async fn mark_all_read(db: &DatabaseConnection, user_id: Uuid) -> Result<(), AppError> {
        let rows = notification::Entity::find()
            .filter(notification::Column::UserId.eq(user_id))
            .filter(notification::Column::IsRead.eq(false))
            .all(db)
            .await
            .map_err(|e| AppError::internal(format!("db error: {e}")))?;
        for row in rows {
            let mut active: notification::ActiveModel = row.into();
            active.is_read = Set(true);
            active
                .update(db)
                .await
                .map_err(|e| AppError::internal(format!("db error: {e}")))?;
        }
        Ok(())
    }
}

pub fn to_response(model: &notification::Model) -> NotificationResponse {
    NotificationResponse {
        id: model.id,
        title: model.title.clone(),
        message: model.message.clone(),
        kind: model.kind.clone(),
        is_read: model.is_read,
        created_at: model.created_at,
    }
}
