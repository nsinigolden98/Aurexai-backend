use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use axum_extra::extract::CookieJar;

use crate::{
    app::AppState,
    error::{AppError, ErrorResponse},
    serializer::auth::{
        AuthResponse, AuthUserResponse, ConfirmOtpRequest, ForgotPasswordRequest,
        GoogleAuthRequest, LoginRequest, MessageResponse, ResendOtpRequest, ResetPasswordRequest,
        SignupRequest,
    },
    services::auth::AuthService,
    utils::validation::validate_payload,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/signup/", post(signup))
        .route("/confirm-otp/", post(confirm_otp))
        .route("/resend-otp/", post(resend_otp))
        .route("/login/", post(login))
        .route("/refresh/", post(refresh))
        .route("/logout/", post(logout))
        .route("/forgot-password/", post(forgot_password))
        .route("/reset-password/", post(reset_password))
        .route("/google/", post(google_auth))
        .route("/me/", get(me))
        .route("/csrf/", get(csrf_token))
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/signup/",
    tag = "Auth",
    request_body = SignupRequest,
    responses(
        (status = 201, description = "Signup successful, OTP sent", body = MessageResponse),
        (status = 400, description = "Invalid input", body = ErrorResponse),
        (status = 409, description = "Email already registered", body = ErrorResponse)
    )
)]
pub async fn signup(
    State(state): State<AppState>,
    Json(payload): Json<SignupRequest>,
) -> Result<impl IntoResponse, AppError> {
    validate_payload(&payload)?;
    let res = AuthService::signup(&state.db, &state.config, &state.email_service, payload).await?;
    Ok((StatusCode::CREATED, Json(res)))
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/confirm-otp/",
    tag = "Auth",
    request_body = ConfirmOtpRequest,
    responses(
        (status = 200, description = "Email verified and tokens issued", body = AuthResponse),
        (status = 400, description = "Invalid OTP or request", body = ErrorResponse)
    )
)]
pub async fn confirm_otp(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(payload): Json<ConfirmOtpRequest>,
) -> Result<impl IntoResponse, AppError> {
    validate_payload(&payload)?;
    let (res, tokens) = AuthService::confirm_otp(
        &state.db,
        &state.config,
        &state.email_service,
        &state.notification_manager,
        payload,
    )
    .await?;
    let jar = jar
        .add(tokens.access_cookie)
        .add(tokens.refresh_cookie)
        .add(tokens.csrf_cookie);
    Ok((jar, Json(res)))
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/resend-otp/",
    tag = "Auth",
    request_body = ResendOtpRequest,
    responses(
        (status = 200, description = "OTP resent", body = MessageResponse),
        (status = 400, description = "Invalid request or cooldown active", body = ErrorResponse),
        (status = 404, description = "User not found", body = ErrorResponse)
    )
)]
pub async fn resend_otp(
    State(state): State<AppState>,
    Json(payload): Json<ResendOtpRequest>,
) -> Result<impl IntoResponse, AppError> {
    validate_payload(&payload)?;
    let res =
        AuthService::resend_otp(&state.db, &state.config, &state.email_service, payload).await?;
    Ok(Json(res))
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/login/",
    tag = "Auth",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Login successful", body = AuthResponse),
        (status = 401, description = "Invalid credentials", body = ErrorResponse),
        (status = 403, description = "Email not verified", body = ErrorResponse)
    )
)]
pub async fn login(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(payload): Json<LoginRequest>,
) -> Result<impl IntoResponse, AppError> {
    validate_payload(&payload)?;
    let (res, tokens) = AuthService::login(
        &state.db,
        &state.config,
        &state.notification_manager,
        payload,
    )
    .await?;
    let jar = jar
        .add(tokens.access_cookie)
        .add(tokens.refresh_cookie)
        .add(tokens.csrf_cookie);
    Ok((jar, Json(res)))
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/refresh/",
    tag = "Auth",
    responses(
        (status = 200, description = "Tokens refreshed", body = MessageResponse),
        (status = 401, description = "Invalid or missing refresh token", body = ErrorResponse)
    )
)]
pub async fn refresh(
    State(state): State<AppState>,
    jar: CookieJar,
) -> Result<impl IntoResponse, AppError> {
    let refresh_cookie = jar.get("refresh_token").map(|c| c.value().to_string());
    let tokens = AuthService::refresh(&state.db, &state.config, refresh_cookie).await?;
    let jar = jar
        .add(tokens.access_cookie)
        .add(tokens.refresh_cookie)
        .add(tokens.csrf_cookie);
    Ok((
        jar,
        Json(MessageResponse {
            message: "Token refreshed successfully.".to_string(),
        }),
    ))
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/logout/",
    tag = "Auth",
    responses(
        (status = 200, description = "Logged out", body = MessageResponse)
    )
)]
pub async fn logout(
    State(state): State<AppState>,
    jar: CookieJar,
) -> Result<impl IntoResponse, AppError> {
    let refresh_cookie = jar.get("refresh_token").map(|c| c.value().to_string());
    let (res, clear_access, clear_refresh, clear_csrf) =
        AuthService::logout(&state.db, &state.config, refresh_cookie).await?;
    let jar = jar.add(clear_access).add(clear_refresh).add(clear_csrf);
    Ok((jar, Json(res)))
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/forgot-password/",
    tag = "Auth",
    request_body = ForgotPasswordRequest,
    responses(
        (status = 200, description = "Reset email sent if account exists", body = MessageResponse)
    )
)]
pub async fn forgot_password(
    State(state): State<AppState>,
    Json(payload): Json<ForgotPasswordRequest>,
) -> Result<impl IntoResponse, AppError> {
    validate_payload(&payload)?;
    let res = AuthService::forgot_password(
        &state.db,
        &state.config,
        &state.email_service,
        &state.notification_manager,
        payload,
    )
    .await?;
    Ok(Json(res))
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/reset-password/",
    tag = "Auth",
    request_body = ResetPasswordRequest,
    responses(
        (status = 200, description = "Password reset successful", body = MessageResponse),
        (status = 400, description = "Invalid or expired token", body = ErrorResponse)
    )
)]
pub async fn reset_password(
    State(state): State<AppState>,
    Json(payload): Json<ResetPasswordRequest>,
) -> Result<impl IntoResponse, AppError> {
    validate_payload(&payload)?;
    let res = AuthService::reset_password(&state.db, &state.notification_manager, payload).await?;
    Ok(Json(res))
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/google/",
    tag = "Auth",
    request_body = GoogleAuthRequest,
    responses(
        (status = 200, description = "Google authentication successful", body = AuthResponse),
        (status = 401, description = "Invalid Google token", body = ErrorResponse)
    )
)]
pub async fn google_auth(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(payload): Json<GoogleAuthRequest>,
) -> Result<impl IntoResponse, AppError> {
    validate_payload(&payload)?;
    let (res, tokens) = AuthService::google_auth(
        &state.db,
        &state.config,
        &state.notification_manager,
        payload,
    )
    .await?;
    let jar = jar
        .add(tokens.access_cookie)
        .add(tokens.refresh_cookie)
        .add(tokens.csrf_cookie);
    Ok((jar, Json(res)))
}

#[utoipa::path(
    get,
    path = "/api/v1/auth/me/",
    tag = "Auth",
    responses(
        (status = 200, description = "Current user", body = AuthUserResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    )
)]
pub async fn me(
    State(state): State<AppState>,
    jar: CookieJar,
) -> Result<impl IntoResponse, AppError> {
    let access_cookie = jar.get("access_token").map(|c| c.value().to_string());
    let res = AuthService::me(&state.db, access_cookie, &state.config).await?;
    Ok(Json(res))
}

#[utoipa::path(
    get,
    path = "/api/v1/auth/csrf/",
    tag = "Auth",
    responses(
        (status = 200, description = "CSRF cookie issued", body = MessageResponse)
    )
)]
pub async fn csrf_token(State(state): State<AppState>, jar: CookieJar) -> impl IntoResponse {
    let jar = jar.add(crate::services::auth::issue_csrf_cookie(&state.config));
    (
        jar,
        Json(MessageResponse {
            message: "CSRF cookie set".to_string(),
        }),
    )
}
