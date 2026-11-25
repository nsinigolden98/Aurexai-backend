use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Deserialize, ToSchema, Validate)]
pub struct SignupRequest {
    #[validate(length(min = 1, message = "Name is required"))]
    pub name: String,
    #[validate(length(min = 1, message = "Surname is required"))]
    pub surname: String,
    #[validate(email(message = "Invalid email address"))]
    pub email: String,
    #[validate(length(min = 8, message = "Password must be at least 8 characters"))]
    pub password: String,
    #[validate(length(min = 8, message = "Confirm password must be at least 8 characters"))]
    pub confirm_password: String,
}

#[derive(Debug, Deserialize, ToSchema, Validate)]
pub struct LoginRequest {
    #[validate(email(message = "Invalid email address"))]
    pub email: String,
    #[validate(length(min = 1, message = "Password is required"))]
    pub password: String,
}

#[derive(Debug, Deserialize, ToSchema, Validate)]
pub struct ConfirmOtpRequest {
    #[validate(email(message = "Invalid email address"))]
    pub email: String,
    #[validate(length(equal = 6, message = "OTP must be a 6-digit code"))]
    pub code: String,
}

#[derive(Debug, Deserialize, ToSchema, Validate)]
pub struct ResendOtpRequest {
    #[validate(email(message = "Invalid email address"))]
    pub email: String,
}

#[derive(Debug, Deserialize, ToSchema, Validate)]
pub struct ForgotPasswordRequest {
    #[validate(email(message = "Invalid email address"))]
    pub email: String,
}

#[derive(Debug, Deserialize, ToSchema, Validate)]
pub struct ResetPasswordRequest {
    #[validate(length(min = 1, message = "Reset token is required"))]
    pub token: String,
    #[validate(length(min = 8, message = "Password must be at least 8 characters"))]
    pub password: String,
    #[validate(length(min = 8, message = "Confirm password must be at least 8 characters"))]
    pub confirm_password: String,
}

#[derive(Debug, Deserialize, ToSchema, Validate)]
pub struct GoogleAuthRequest {
    #[validate(length(min = 1, message = "id_token is required"))]
    pub id_token: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct MessageResponse {
    pub message: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AuthUserResponse {
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
pub struct AuthResponse {
    pub message: String,
    pub user: AuthUserResponse,
}
