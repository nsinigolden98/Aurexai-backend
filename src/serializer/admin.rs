use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Serialize, ToSchema)]
pub struct AdminUserResponse {
    pub id: Uuid,
    pub name: String,
    pub surname: String,
    pub email: String,
    pub referral_code: Option<String>,
    pub is_staff: bool,
    pub is_admin: bool,
    pub is_email_verified: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AdminUsersListResponse {
    pub users: Vec<AdminUserResponse>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateUserRoleRequest {
    pub is_staff: Option<bool>,
    pub is_admin: Option<bool>,
}
