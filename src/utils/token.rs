use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{Duration, Utc};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use rand::{RngCore, rngs::OsRng};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AppError;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AccessTokenClaims {
    pub sub: String,
    pub exp: usize,
    pub iat: usize,
    pub token_type: String,
}

pub fn generate_random_token() -> String {
    let mut bytes = [0u8; 64];
    OsRng.fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

pub fn generate_otp_code() -> String {
    let mut bytes = [0u8; 4];
    OsRng.fill_bytes(&mut bytes);
    let n = u32::from_be_bytes(bytes) % 1_000_000;
    format!("{n:06}")
}

pub fn generate_referral_code() -> String {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let mut bytes = [0u8; 8];
    OsRng.fill_bytes(&mut bytes);
    bytes
        .iter()
        .map(|b| {
            let idx = (*b as usize) % CHARSET.len();
            CHARSET[idx] as char
        })
        .collect()
}

pub fn create_access_token(
    user_id: Uuid,
    secret: &str,
    ttl_minutes: i64,
) -> Result<String, AppError> {
    let now = Utc::now();
    let exp = now + Duration::minutes(ttl_minutes);
    let claims = AccessTokenClaims {
        sub: user_id.to_string(),
        iat: now.timestamp() as usize,
        exp: exp.timestamp() as usize,
        token_type: "access".to_string(),
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| AppError::internal(format!("failed to create access token: {e}")))
}

pub fn decode_access_token(token: &str, secret: &str) -> Result<AccessTokenClaims, AppError> {
    let mut validation = Validation::default();
    validation.validate_exp = true;
    let data = decode::<AccessTokenClaims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .map_err(|_| AppError::unauthorized("Invalid or expired access token"))?;
    if data.claims.token_type != "access" {
        return Err(AppError::unauthorized("Invalid access token"));
    }
    Ok(data.claims)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn otp_code_is_6_digits() {
        for _ in 0..100 {
            let code = generate_otp_code();
            assert_eq!(code.len(), 6);
            assert!(code.chars().all(|c| c.is_ascii_digit()));
        }
    }

    #[test]
    fn referral_code_is_8_alphanumeric() {
        for _ in 0..100 {
            let code = generate_referral_code();
            assert_eq!(code.len(), 8);
            assert!(
                code.chars()
                    .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
            );
        }
    }

    #[test]
    fn access_token_round_trip() {
        let user_id = Uuid::new_v4();
        let secret = "test-secret";
        let token = create_access_token(user_id, secret, 15).expect("token should be created");
        let claims = decode_access_token(&token, secret).expect("token should decode");
        assert_eq!(claims.sub, user_id.to_string());
        assert_eq!(claims.token_type, "access");
    }

    #[test]
    fn access_token_rejects_wrong_secret() {
        let user_id = Uuid::new_v4();
        let token = create_access_token(user_id, "secret-a", 15).expect("token should be created");
        let err = decode_access_token(&token, "secret-b").expect_err("should fail");
        assert_eq!(err.status, axum::http::StatusCode::UNAUTHORIZED);
    }
}
