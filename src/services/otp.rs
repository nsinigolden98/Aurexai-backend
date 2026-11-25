use chrono::{Duration, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};
use uuid::Uuid;

use crate::{
    error::AppError,
    models::otp,
    utils::{hash::sha256_hex, token::generate_otp_code},
};

pub const PURPOSE_EMAIL_VERIFICATION: &str = "email_verification";

pub struct OtpService;

impl OtpService {
    pub async fn create_email_verification_otp(
        db: &DatabaseConnection,
        user_id: Uuid,
        ttl_minutes: i64,
        max_attempts: i32,
    ) -> Result<String, AppError> {
        let now = Utc::now();

        // Invalidate existing active OTPs for this purpose
        let active_otps = otp::Entity::find()
            .filter(otp::Column::UserId.eq(user_id))
            .filter(otp::Column::Purpose.eq(PURPOSE_EMAIL_VERIFICATION))
            .filter(otp::Column::ConsumedAt.is_null())
            .all(db)
            .await
            .map_err(|e| AppError::internal(format!("db error: {e}")))?;

        for otp_model in active_otps {
            let mut active: otp::ActiveModel = otp_model.into();
            active.consumed_at = Set(Some(now));
            active
                .update(db)
                .await
                .map_err(|e| AppError::internal(format!("db error: {e}")))?;
        }

        let code = generate_otp_code();
        let model = otp::ActiveModel {
            id: Set(Uuid::new_v4()),
            user_id: Set(user_id),
            code_hash: Set(sha256_hex(&code)),
            purpose: Set(PURPOSE_EMAIL_VERIFICATION.to_string()),
            expires_at: Set(now + Duration::minutes(ttl_minutes)),
            consumed_at: Set(None),
            attempts: Set(0),
            max_attempts: Set(max_attempts),
            created_at: Set(now),
        };

        model
            .insert(db)
            .await
            .map_err(|e| AppError::internal(format!("db error: {e}")))?;

        Ok(code)
    }

    pub async fn verify_email_otp(
        db: &DatabaseConnection,
        user_id: Uuid,
        code: &str,
        max_attempts: i32,
    ) -> Result<(), AppError> {
        let now = Utc::now();
        let otp_model = otp::Entity::find()
            .filter(otp::Column::UserId.eq(user_id))
            .filter(otp::Column::Purpose.eq(PURPOSE_EMAIL_VERIFICATION))
            .filter(otp::Column::ConsumedAt.is_null())
            .order_by_desc(otp::Column::CreatedAt)
            .one(db)
            .await
            .map_err(|e| AppError::internal(format!("db error: {e}")))?
            .ok_or_else(|| {
                AppError::bad_request("No active OTP found. Please request a new code.")
            })?;

        if otp_model.expires_at < now {
            let mut active: otp::ActiveModel = otp_model.into();
            active.consumed_at = Set(Some(now));
            let _ = active.update(db).await;
            return Err(AppError::bad_request(
                "OTP has expired. Please request a new code.",
            ));
        }

        if otp_model.attempts >= max_attempts || otp_model.attempts >= otp_model.max_attempts {
            let mut active: otp::ActiveModel = otp_model.into();
            active.consumed_at = Set(Some(now));
            let _ = active.update(db).await;
            return Err(AppError::bad_request(
                "Too many invalid OTP attempts. Please request a new code.",
            ));
        }

        let provided_hash = sha256_hex(code);
        if provided_hash != otp_model.code_hash {
            let attempts = otp_model.attempts + 1;
            let mut active: otp::ActiveModel = otp_model.into();
            active.attempts = Set(attempts);
            if attempts >= max_attempts {
                active.consumed_at = Set(Some(now));
            }
            active
                .update(db)
                .await
                .map_err(|e| AppError::internal(format!("db error: {e}")))?;
            return Err(AppError::bad_request("Invalid OTP code."));
        }

        let mut active: otp::ActiveModel = otp_model.into();
        active.consumed_at = Set(Some(now));
        active
            .update(db)
            .await
            .map_err(|e| AppError::internal(format!("db error: {e}")))?;

        Ok(())
    }

    pub async fn can_resend(
        db: &DatabaseConnection,
        user_id: Uuid,
        cooldown_seconds: i64,
    ) -> Result<bool, AppError> {
        let now = Utc::now();
        let latest = otp::Entity::find()
            .filter(otp::Column::UserId.eq(user_id))
            .filter(otp::Column::Purpose.eq(PURPOSE_EMAIL_VERIFICATION))
            .order_by_desc(otp::Column::CreatedAt)
            .one(db)
            .await
            .map_err(|e| AppError::internal(format!("db error: {e}")))?;

        if let Some(otp_model) = latest
            && otp_model.created_at + Duration::seconds(cooldown_seconds) > now
        {
            return Ok(false);
        }
        Ok(true)
    }
}
