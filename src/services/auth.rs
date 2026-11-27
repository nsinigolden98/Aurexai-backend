use axum_extra::extract::cookie::{Cookie, SameSite};
use chrono::{Duration, Utc};
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

use crate::{
    config::Config,
    error::AppError,
    models::{password_reset_token, refresh_token, user},
    serializer::auth::{
        AuthResponse, AuthUserResponse, ConfirmOtpRequest, ForgotPasswordRequest,
        GoogleAuthRequest, LoginRequest, MessageResponse, ResendOtpRequest, ResetPasswordRequest,
        SignupRequest,
    },
    services::{
        email::EmailService,
        notification::{NotificationManager, NotificationService},
        otp::OtpService,
    },
    utils::{
        hash::{hash_password, sha256_hex, verify_password},
        token::{
            create_access_token, decode_access_token, generate_random_token, generate_referral_code,
        },
    },
};

const ACCESS_COOKIE: &str = "access_token";
const REFRESH_COOKIE: &str = "refresh_token";
const CSRF_COOKIE: &str = "csrf_token";

pub struct AuthTokens {
    pub access_token: String,
    pub refresh_token: String,
    pub access_cookie: Cookie<'static>,
    pub refresh_cookie: Cookie<'static>,
    pub csrf_cookie: Cookie<'static>,
}

pub struct AuthService;

impl AuthService {
    pub async fn signup(
        db: &DatabaseConnection,
        config: &Config,
        email_service: &EmailService,
        payload: SignupRequest,
    ) -> Result<MessageResponse, AppError> {
        let name = payload.name.trim();
        let surname = payload.surname.trim();
        let email = normalize_email(&payload.email);

        if name.is_empty() || surname.is_empty() {
            return Err(AppError::bad_request("Name and surname are required"));
        }
        if !is_valid_email(&email) {
            return Err(AppError::bad_request("Invalid email address"));
        }
        if payload.password.len() < 8 {
            return Err(AppError::bad_request(
                "Password must be at least 8 characters",
            ));
        }
        if payload.password != payload.confirm_password {
            return Err(AppError::bad_request("Passwords do not match"));
        }

        let existing = user::Entity::find()
            .filter(user::Column::Email.eq(&email))
            .one(db)
            .await
            .map_err(|e| AppError::internal(format!("db error: {e}")))?;

        if let Some(existing_user) = existing {
            if existing_user.is_email_verified {
                return Err(AppError::conflict("Email is already registered"));
            }

            // Unverified account: update details and resend OTP
            let mut active: user::ActiveModel = existing_user.clone().into();
            active.name = Set(name.to_string());
            active.surname = Set(surname.to_string());
            active.password_hash = Set(Some(hash_password(&payload.password)?));
            if existing_user.referral_code.is_none() {
                active.referral_code = Set(Some(generate_unique_referral_code(db).await?));
            }
            active.updated_at = Set(Utc::now());
            let updated = active
                .update(db)
                .await
                .map_err(|e| AppError::internal(format!("db error: {e}")))?;

            Self::send_verification_otp(db, config, email_service, &updated).await?;

            return Ok(MessageResponse {
                message: "Account exists but is not verified. A new verification code has been sent to your email.".to_string(),
            });
        }

        let now = Utc::now();
        let referral_code = generate_unique_referral_code(db).await?;
        let user_model = user::ActiveModel {
            id: Set(Uuid::new_v4()),
            name: Set(name.to_string()),
            surname: Set(surname.to_string()),
            email: Set(email.clone()),
            password_hash: Set(Some(hash_password(&payload.password)?)),
            is_staff: Set(false),
            is_admin: Set(false),
            is_email_verified: Set(false),
            google_sub: Set(None),
            referral_code: Set(Some(referral_code)),
            created_at: Set(now),
            updated_at: Set(now),
        };

        let created = user_model
            .insert(db)
            .await
            .map_err(|e| AppError::internal(format!("db error: {e}")))?;

        Self::send_verification_otp(db, config, email_service, &created).await?;

        Ok(MessageResponse {
            message: "Signup successful. Please verify the OTP sent to your email.".to_string(),
        })
    }

    pub async fn confirm_otp(
        db: &DatabaseConnection,
        config: &Config,
        email_service: &EmailService,
        notification_manager: &NotificationManager,
        payload: ConfirmOtpRequest,
    ) -> Result<(AuthResponse, AuthTokens), AppError> {
        let email = normalize_email(&payload.email);
        let code = payload.code.trim();

        if code.len() != 6 || !code.chars().all(|c| c.is_ascii_digit()) {
            return Err(AppError::bad_request("OTP must be a 6-digit code"));
        }

        let user_model = find_user_by_email(db, &email)
            .await?
            .ok_or_else(|| AppError::bad_request("Invalid email or OTP"))?;

        if user_model.is_email_verified {
            return Err(AppError::bad_request("Email is already verified"));
        }

        OtpService::verify_email_otp(db, user_model.id, code, config.otp_max_attempts).await?;

        let mut active: user::ActiveModel = user_model.clone().into();
        active.is_email_verified = Set(true);
        active.updated_at = Set(Utc::now());
        let verified_user = active
            .update(db)
            .await
            .map_err(|e| AppError::internal(format!("db error: {e}")))?;

        let to_name = format!("{} {}", verified_user.name, verified_user.surname);
        let _ = email_service
            .send_welcome_email(&verified_user.email, &to_name)
            .await;

        let tokens = issue_tokens(db, config, verified_user.id).await?;
        let _ = NotificationService::create_and_publish(
            db,
            notification_manager,
            verified_user.id,
            "Email verified",
            "Your email has been verified successfully. Welcome to Aurexai!",
            "auth.email_verified",
        )
        .await;

        Ok((
            AuthResponse {
                message: "Email verified successfully.".to_string(),
                user: to_user_response(&verified_user),
            },
            tokens,
        ))
    }

    pub async fn resend_otp(
        db: &DatabaseConnection,
        config: &Config,
        email_service: &EmailService,
        payload: ResendOtpRequest,
    ) -> Result<MessageResponse, AppError> {
        let email = normalize_email(&payload.email);
        let user_model = find_user_by_email(db, &email)
            .await?
            .ok_or_else(|| AppError::not_found("User not found"))?;

        if user_model.is_email_verified {
            return Err(AppError::bad_request("Email is already verified"));
        }

        let can_resend =
            OtpService::can_resend(db, user_model.id, config.otp_resend_cooldown_seconds).await?;
        if !can_resend {
            return Err(AppError::bad_request(
                "Please wait before requesting another OTP.",
            ));
        }

        Self::send_verification_otp(db, config, email_service, &user_model).await?;

        Ok(MessageResponse {
            message: "A new verification code has been sent to your email.".to_string(),
        })
    }

    pub async fn login(
        db: &DatabaseConnection,
        config: &Config,
        notification_manager: &NotificationManager,
        payload: LoginRequest,
    ) -> Result<(AuthResponse, AuthTokens), AppError> {
        let email = normalize_email(&payload.email);
        let user_model = find_user_by_email(db, &email)
            .await?
            .ok_or_else(|| AppError::unauthorized("Invalid email or password"))?;

        if !user_model.is_email_verified {
            return Err(AppError::forbidden(
                "Please verify your email before logging in",
            ));
        }

        let password_hash = user_model.password_hash.as_ref().ok_or_else(|| {
            AppError::bad_request(
                "This account uses Google sign-in. Please sign in with Google or reset your password.",
            )
        })?;

        if !verify_password(&payload.password, password_hash)? {
            return Err(AppError::unauthorized("Invalid email or password"));
        }

        let tokens = issue_tokens(db, config, user_model.id).await?;
        let _ = NotificationService::create_and_publish(
            db,
            notification_manager,
            user_model.id,
            "New login",
            "Your account was signed in successfully.",
            "auth.login",
        )
        .await;

        Ok((
            AuthResponse {
                message: "Login successful.".to_string(),
                user: to_user_response(&user_model),
            },
            tokens,
        ))
    }

    pub async fn refresh(
        db: &DatabaseConnection,
        config: &Config,
        refresh_cookie: Option<String>,
    ) -> Result<AuthTokens, AppError> {
        let refresh_token_value =
            refresh_cookie.ok_or_else(|| AppError::unauthorized("Missing refresh token"))?;

        let token_hash = sha256_hex(&refresh_token_value);
        let now = Utc::now();

        let stored = refresh_token::Entity::find()
            .filter(refresh_token::Column::TokenHash.eq(&token_hash))
            .one(db)
            .await
            .map_err(|e| AppError::internal(format!("db error: {e}")))?
            .ok_or_else(|| AppError::unauthorized("Invalid refresh token"))?;

        if stored.revoked_at.is_some() || stored.expires_at < now {
            return Err(AppError::unauthorized(
                "Refresh token is expired or revoked",
            ));
        }

        let user_model = user::Entity::find_by_id(stored.user_id)
            .one(db)
            .await
            .map_err(|e| AppError::internal(format!("db error: {e}")))?
            .ok_or_else(|| AppError::unauthorized("User not found"))?;

        let new_tokens = issue_tokens(db, config, user_model.id).await?;

        // Revoke old token and link to new one
        let new_refresh_hash = sha256_hex(&new_tokens.refresh_token);
        let new_stored = refresh_token::Entity::find()
            .filter(refresh_token::Column::TokenHash.eq(&new_refresh_hash))
            .one(db)
            .await
            .map_err(|e| AppError::internal(format!("db error: {e}")))?;

        let mut old_active: refresh_token::ActiveModel = stored.into();
        old_active.revoked_at = Set(Some(now));
        old_active.replaced_by = Set(new_stored.map(|t| t.id));
        old_active
            .update(db)
            .await
            .map_err(|e| AppError::internal(format!("db error: {e}")))?;

        Ok(new_tokens)
    }

    pub async fn logout(
        db: &DatabaseConnection,
        config: &Config,
        refresh_cookie: Option<String>,
    ) -> Result<
        (
            MessageResponse,
            Cookie<'static>,
            Cookie<'static>,
            Cookie<'static>,
        ),
        AppError,
    > {
        if let Some(refresh_token_value) = refresh_cookie {
            let token_hash = sha256_hex(&refresh_token_value);
            if let Some(stored) = refresh_token::Entity::find()
                .filter(refresh_token::Column::TokenHash.eq(&token_hash))
                .one(db)
                .await
                .map_err(|e| AppError::internal(format!("db error: {e}")))?
            {
                let mut active: refresh_token::ActiveModel = stored.into();
                active.revoked_at = Set(Some(Utc::now()));
                active
                    .update(db)
                    .await
                    .map_err(|e| AppError::internal(format!("db error: {e}")))?;
            }
        }

        Ok((
            MessageResponse {
                message: "Logged out successfully.".to_string(),
            },
            clear_cookie(config, ACCESS_COOKIE, "/", true),
            clear_cookie(config, REFRESH_COOKIE, "/api/v1/auth/", true),
            clear_cookie(config, CSRF_COOKIE, "/", false),
        ))
    }

    pub async fn me(
        db: &DatabaseConnection,
        access_cookie: Option<String>,
        config: &Config,
    ) -> Result<AuthUserResponse, AppError> {
        let user_model = authenticate_access_token(db, config, access_cookie).await?;
        Ok(to_user_response(&user_model))
    }

    pub async fn forgot_password(
        db: &DatabaseConnection,
        config: &Config,
        email_service: &EmailService,
        notification_manager: &NotificationManager,
        payload: ForgotPasswordRequest,
    ) -> Result<MessageResponse, AppError> {
        let email = normalize_email(&payload.email);
        if let Some(user_model) = find_user_by_email(db, &email).await? {
            let now = Utc::now();

            // Invalidate old unused reset tokens
            let old_tokens = password_reset_token::Entity::find()
                .filter(password_reset_token::Column::UserId.eq(user_model.id))
                .filter(password_reset_token::Column::UsedAt.is_null())
                .all(db)
                .await
                .map_err(|e| AppError::internal(format!("db error: {e}")))?;

            for t in old_tokens {
                let mut active: password_reset_token::ActiveModel = t.into();
                active.used_at = Set(Some(now));
                active
                    .update(db)
                    .await
                    .map_err(|e| AppError::internal(format!("db error: {e}")))?;
            }

            let raw_token = generate_random_token();
            let token_hash = sha256_hex(&raw_token);
            let reset_model = password_reset_token::ActiveModel {
                id: Set(Uuid::new_v4()),
                user_id: Set(user_model.id),
                token_hash: Set(token_hash),
                expires_at: Set(now + Duration::minutes(config.reset_token_ttl_minutes)),
                used_at: Set(None),
                created_at: Set(now),
            };
            reset_model
                .insert(db)
                .await
                .map_err(|e| AppError::internal(format!("db error: {e}")))?;

            let reset_link = format!(
                "{}?token={}",
                config.frontend_reset_password_url.trim_end_matches('/'),
                raw_token
            );
            let to_name = format!("{} {}", user_model.name, user_model.surname);
            email_service
                .send_password_reset_email(
                    &user_model.email,
                    &to_name,
                    &reset_link,
                    config.reset_token_ttl_minutes,
                )
                .await?;
            let _ = NotificationService::create_and_publish(
                db,
                notification_manager,
                user_model.id,
                "Password reset requested",
                "A password reset link was requested for your account.",
                "auth.password_reset_requested",
            )
            .await;
        }

        // As requested: only send if email exists in DB. We still return a generic message.
        Ok(MessageResponse {
            message: "If the email exists, a password reset link has been sent.".to_string(),
        })
    }

    pub async fn reset_password(
        db: &DatabaseConnection,
        notification_manager: &NotificationManager,
        payload: ResetPasswordRequest,
    ) -> Result<MessageResponse, AppError> {
        if payload.password.len() < 8 {
            return Err(AppError::bad_request(
                "Password must be at least 8 characters",
            ));
        }
        if payload.password != payload.confirm_password {
            return Err(AppError::bad_request("Passwords do not match"));
        }

        let token_hash = sha256_hex(payload.token.trim());
        let now = Utc::now();
        let stored = password_reset_token::Entity::find()
            .filter(password_reset_token::Column::TokenHash.eq(&token_hash))
            .one(db)
            .await
            .map_err(|e| AppError::internal(format!("db error: {e}")))?
            .ok_or_else(|| AppError::bad_request("Invalid or expired reset token"))?;
        let stored_user_id = stored.user_id;

        if stored.used_at.is_some() || stored.expires_at < now {
            return Err(AppError::bad_request("Invalid or expired reset token"));
        }

        let user_model = user::Entity::find_by_id(stored.user_id)
            .one(db)
            .await
            .map_err(|e| AppError::internal(format!("db error: {e}")))?
            .ok_or_else(|| AppError::bad_request("User not found"))?;

        let mut user_active: user::ActiveModel = user_model.into();
        user_active.password_hash = Set(Some(hash_password(&payload.password)?));
        user_active.updated_at = Set(now);
        user_active
            .update(db)
            .await
            .map_err(|e| AppError::internal(format!("db error: {e}")))?;

        let mut token_active: password_reset_token::ActiveModel = stored.into();
        token_active.used_at = Set(Some(now));
        token_active
            .update(db)
            .await
            .map_err(|e| AppError::internal(format!("db error: {e}")))?;

        let _ = NotificationService::create_and_publish(
            db,
            notification_manager,
            stored_user_id,
            "Password reset successful",
            "Your password was reset successfully.",
            "auth.password_reset_success",
        )
        .await;

        Ok(MessageResponse {
            message: "Password reset successful. You can now log in.".to_string(),
        })
    }

    pub async fn google_auth(
        db: &DatabaseConnection,
        config: &Config,
        notification_manager: &NotificationManager,
        payload: GoogleAuthRequest,
    ) -> Result<(AuthResponse, AuthTokens), AppError> {
        let google_user =
            verify_google_id_token(&payload.id_token, &config.google_client_id).await?;
        let email = normalize_email(&google_user.email);

        if !google_user.email_verified {
            return Err(AppError::bad_request("Google email is not verified"));
        }

        let now = Utc::now();
        let user_model = if let Some(existing_by_sub) = user::Entity::find()
            .filter(user::Column::GoogleSub.eq(&google_user.sub))
            .one(db)
            .await
            .map_err(|e| AppError::internal(format!("db error: {e}")))?
        {
            existing_by_sub
        } else if let Some(existing_by_email) = find_user_by_email(db, &email).await? {
            // Link accounts by email
            let mut active: user::ActiveModel = existing_by_email.clone().into();
            active.google_sub = Set(Some(google_user.sub.clone()));
            active.is_email_verified = Set(true);
            if existing_by_email.name.trim().is_empty() {
                active.name = Set(google_user
                    .given_name
                    .clone()
                    .unwrap_or_else(|| "User".to_string()));
            }
            if existing_by_email.surname.trim().is_empty() {
                active.surname = Set(google_user
                    .family_name
                    .clone()
                    .unwrap_or_else(|| "User".to_string()));
            }
            if existing_by_email.referral_code.is_none() {
                active.referral_code = Set(Some(generate_unique_referral_code(db).await?));
            }
            active.updated_at = Set(now);
            active
                .update(db)
                .await
                .map_err(|e| AppError::internal(format!("db error: {e}")))?
        } else {
            let name = google_user
                .given_name
                .clone()
                .or_else(|| google_user.name.clone())
                .unwrap_or_else(|| "User".to_string());
            let surname = google_user
                .family_name
                .clone()
                .unwrap_or_else(|| "User".to_string());

            let referral_code = generate_unique_referral_code(db).await?;
            let new_user = user::ActiveModel {
                id: Set(Uuid::new_v4()),
                name: Set(name),
                surname: Set(surname),
                email: Set(email),
                password_hash: Set(None),
                is_staff: Set(false),
                is_admin: Set(false),
                is_email_verified: Set(true),
                google_sub: Set(Some(google_user.sub)),
                referral_code: Set(Some(referral_code)),
                created_at: Set(now),
                updated_at: Set(now),
            };
            new_user
                .insert(db)
                .await
                .map_err(|e| AppError::internal(format!("db error: {e}")))?
        };

        let tokens = issue_tokens(db, config, user_model.id).await?;
        let _ = NotificationService::create_and_publish(
            db,
            notification_manager,
            user_model.id,
            "Google sign-in",
            "Your account was signed in with Google.",
            "auth.google_signin",
        )
        .await;

        Ok((
            AuthResponse {
                message: "Google authentication successful.".to_string(),
                user: to_user_response(&user_model),
            },
            tokens,
        ))
    }

    async fn send_verification_otp(
        db: &DatabaseConnection,
        config: &Config,
        email_service: &EmailService,
        user_model: &user::Model,
    ) -> Result<(), AppError> {
        let code = OtpService::create_email_verification_otp(
            db,
            user_model.id,
            config.otp_ttl_minutes,
            config.otp_max_attempts,
        )
        .await?;
        let to_name = format!("{} {}", user_model.name, user_model.surname);
        email_service
            .send_otp_email(&user_model.email, &to_name, &code, config.otp_ttl_minutes)
            .await
    }
}

pub async fn authenticate_access_token(
    db: &DatabaseConnection,
    config: &Config,
    access_cookie: Option<String>,
) -> Result<user::Model, AppError> {
    let token = access_cookie.ok_or_else(|| AppError::unauthorized("Missing access token"))?;
    let claims = decode_access_token(&token, &config.access_token_secret)?;
    let user_id =
        Uuid::parse_str(&claims.sub).map_err(|_| AppError::unauthorized("Invalid access token"))?;

    let user_model = user::Entity::find_by_id(user_id)
        .one(db)
        .await
        .map_err(|e| AppError::internal(format!("db error: {e}")))?
        .ok_or_else(|| AppError::unauthorized("User not found"))?;

    if !user_model.is_email_verified {
        return Err(AppError::forbidden("Email is not verified"));
    }

    Ok(user_model)
}

async fn issue_tokens(
    db: &DatabaseConnection,
    config: &Config,
    user_id: Uuid,
) -> Result<AuthTokens, AppError> {
    let access_token = create_access_token(
        user_id,
        &config.access_token_secret,
        config.access_token_ttl_minutes,
    )?;
    let refresh_token_value = generate_random_token();
    let refresh_hash = sha256_hex(&refresh_token_value);
    let now = Utc::now();

    let refresh_model = refresh_token::ActiveModel {
        id: Set(Uuid::new_v4()),
        user_id: Set(user_id),
        token_hash: Set(refresh_hash),
        expires_at: Set(now + Duration::days(config.refresh_token_ttl_days)),
        revoked_at: Set(None),
        replaced_by: Set(None),
        created_at: Set(now),
    };
    refresh_model
        .insert(db)
        .await
        .map_err(|e| AppError::internal(format!("db error: {e}")))?;

    let csrf_token = generate_random_token();
    let access_cookie = build_cookie(
        config,
        ACCESS_COOKIE,
        access_token.clone(),
        "/",
        config.access_token_ttl_minutes * 60,
        true,
    );
    let refresh_cookie = build_cookie(
        config,
        REFRESH_COOKIE,
        refresh_token_value.clone(),
        "/api/v1/auth/",
        config.refresh_token_ttl_days * 24 * 60 * 60,
        true,
    );
    let csrf_cookie = build_cookie(
        config,
        CSRF_COOKIE,
        csrf_token,
        "/",
        config.refresh_token_ttl_days * 24 * 60 * 60,
        false,
    );

    Ok(AuthTokens {
        access_token,
        refresh_token: refresh_token_value,
        access_cookie,
        refresh_cookie,
        csrf_cookie,
    })
}

fn build_cookie(
    config: &Config,
    name: &str,
    value: String,
    path: &str,
    max_age_seconds: i64,
    http_only: bool,
) -> Cookie<'static> {
    let mut cookie = Cookie::build((name.to_string(), value))
        .path(path.to_string())
        .http_only(http_only)
        .secure(config.cookie_secure)
        .max_age(time::Duration::seconds(max_age_seconds))
        .same_site(parse_same_site(&config.cookie_samesite))
        .build();

    if let Some(domain) = &config.cookie_domain {
        cookie.set_domain(domain.clone());
    }

    cookie
}

fn clear_cookie(config: &Config, name: &str, path: &str, http_only: bool) -> Cookie<'static> {
    let mut cookie = Cookie::build((name.to_string(), ""))
        .path(path.to_string())
        .http_only(http_only)
        .secure(config.cookie_secure)
        .max_age(time::Duration::seconds(0))
        .same_site(parse_same_site(&config.cookie_samesite))
        .build();
    if let Some(domain) = &config.cookie_domain {
        cookie.set_domain(domain.clone());
    }
    cookie
}

fn parse_same_site(value: &str) -> SameSite {
    match value.to_lowercase().as_str() {
        "lax" => SameSite::Lax,
        "strict" => SameSite::Strict,
        "none" => SameSite::None,
        _ => SameSite::None,
    }
}

async fn generate_unique_referral_code(db: &DatabaseConnection) -> Result<String, AppError> {
    for _ in 0..10 {
        let code = generate_referral_code();
        let exists = user::Entity::find()
            .filter(user::Column::ReferralCode.eq(&code))
            .one(db)
            .await
            .map_err(|e| AppError::internal(format!("db error: {e}")))?;
        if exists.is_none() {
            return Ok(code);
        }
    }
    Err(AppError::internal(
        "Failed to generate unique referral code",
    ))
}

fn normalize_email(email: &str) -> String {
    email.trim().to_lowercase()
}

fn is_valid_email(email: &str) -> bool {
    let Some((local, domain)) = email.split_once('@') else {
        return false;
    };
    !local.is_empty() && domain.contains('.') && !domain.starts_with('.') && !domain.ends_with('.')
}

async fn find_user_by_email(
    db: &DatabaseConnection,
    email: &str,
) -> Result<Option<user::Model>, AppError> {
    user::Entity::find()
        .filter(user::Column::Email.eq(email))
        .one(db)
        .await
        .map_err(|e| AppError::internal(format!("db error: {e}")))
}

pub fn issue_csrf_cookie(config: &Config) -> Cookie<'static> {
    build_cookie(
        config,
        CSRF_COOKIE,
        generate_random_token(),
        "/",
        config.refresh_token_ttl_days * 24 * 60 * 60,
        false,
    )
}

pub fn to_user_response(user: &user::Model) -> AuthUserResponse {
    AuthUserResponse {
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

#[derive(Debug, serde::Deserialize)]
struct GoogleTokenInfo {
    sub: String,
    email: String,
    email_verified: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    given_name: Option<String>,
    #[serde(default)]
    family_name: Option<String>,
    aud: String,
}

struct GoogleUser {
    sub: String,
    email: String,
    email_verified: bool,
    name: Option<String>,
    given_name: Option<String>,
    family_name: Option<String>,
}

async fn verify_google_id_token(
    id_token: &str,
    google_client_id: &str,
) -> Result<GoogleUser, AppError> {
    if google_client_id.is_empty() {
        return Err(AppError::internal("GOOGLE_CLIENT_ID is not configured"));
    }

    let client = reqwest::Client::new();
    let res = client
        .get("https://oauth2.googleapis.com/tokeninfo")
        .query(&[("id_token", id_token)])
        .send()
        .await
        .map_err(|e| AppError::internal(format!("failed to verify Google token: {e}")))?;

    if !res.status().is_success() {
        return Err(AppError::unauthorized("Invalid Google ID token"));
    }

    let info = res
        .json::<GoogleTokenInfo>()
        .await
        .map_err(|e| AppError::internal(format!("failed to parse Google token response: {e}")))?;

    if info.aud != google_client_id {
        return Err(AppError::unauthorized("Google token audience mismatch"));
    }

    let email_verified = info.email_verified == "true";

    Ok(GoogleUser {
        sub: info.sub,
        email: info.email,
        email_verified,
        name: info.name,
        given_name: info.given_name,
        family_name: info.family_name,
    })
}
