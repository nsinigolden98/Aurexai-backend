use axum::{
    Json, Router,
    extract::{Path, State},
    routing::get,
};
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use uuid::Uuid;

use crate::{
    app::AppState,
    error::{AppError, ErrorResponse},
    middleware::auth::AdminUser,
    models::user,
    serializer::admin::{AdminUserResponse, AdminUsersListResponse, UpdateUserRoleRequest},
    services::notification::NotificationService,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/users/", get(list_users))
        .route("/users/{id}/", get(get_user).patch(update_user))
}

fn to_admin_user_response(user: &user::Model) -> AdminUserResponse {
    AdminUserResponse {
        id: user.id,
        name: user.name.clone(),
        surname: user.surname.clone(),
        email: user.email.clone(),
        referral_code: user.referral_code.clone(),
        is_staff: user.is_staff,
        is_admin: user.is_admin,
        is_email_verified: user.is_email_verified,
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/admin/users/",
    tag = "Admin",
    responses(
        (status = 200, description = "List users", body = AdminUsersListResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 403, description = "Forbidden", body = ErrorResponse)
    )
)]
pub async fn list_users(
    State(state): State<AppState>,
    AdminUser(_admin): AdminUser,
) -> Result<Json<AdminUsersListResponse>, AppError> {
    let users = user::Entity::find()
        .all(&state.db)
        .await
        .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    Ok(Json(AdminUsersListResponse {
        users: users.iter().map(to_admin_user_response).collect(),
    }))
}

#[utoipa::path(
    get,
    path = "/api/v1/admin/users/{id}/",
    tag = "Admin",
    params(("id" = Uuid, Path, description = "User ID")),
    responses(
        (status = 200, description = "Get user", body = AdminUserResponse),
        (status = 404, description = "User not found", body = ErrorResponse)
    )
)]
pub async fn get_user(
    State(state): State<AppState>,
    AdminUser(_admin): AdminUser,
    Path(id): Path<Uuid>,
) -> Result<Json<AdminUserResponse>, AppError> {
    let user_model = user::Entity::find_by_id(id)
        .one(&state.db)
        .await
        .map_err(|e| AppError::internal(format!("db error: {e}")))?
        .ok_or_else(|| AppError::not_found("User not found"))?;

    Ok(Json(to_admin_user_response(&user_model)))
}

#[utoipa::path(
    patch,
    path = "/api/v1/admin/users/{id}/",
    tag = "Admin",
    params(("id" = Uuid, Path, description = "User ID")),
    request_body = UpdateUserRoleRequest,
    responses(
        (status = 200, description = "User updated", body = AdminUserResponse),
        (status = 404, description = "User not found", body = ErrorResponse)
    )
)]
pub async fn update_user(
    State(state): State<AppState>,
    AdminUser(_admin): AdminUser,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateUserRoleRequest>,
) -> Result<Json<AdminUserResponse>, AppError> {
    let user_model = user::Entity::find_by_id(id)
        .one(&state.db)
        .await
        .map_err(|e| AppError::internal(format!("db error: {e}")))?
        .ok_or_else(|| AppError::not_found("User not found"))?;

    let mut active: user::ActiveModel = user_model.into();
    if let Some(is_staff) = payload.is_staff {
        active.is_staff = Set(is_staff);
    }
    if let Some(is_admin) = payload.is_admin {
        active.is_admin = Set(is_admin);
    }
    active.updated_at = Set(chrono::Utc::now());

    let updated = active
        .update(&state.db)
        .await
        .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    let _ = NotificationService::create_and_publish(
        &state.db,
        &state.notification_manager,
        updated.id,
        "Account role updated",
        "Your account roles were updated by an administrator.",
        "admin.role_updated",
    )
    .await;

    Ok(Json(to_admin_user_response(&updated)))
}
