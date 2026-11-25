use validator::Validate;

use crate::error::AppError;

pub fn validate_payload<T: Validate>(payload: &T) -> Result<(), AppError> {
    payload.validate().map_err(|e| {
        let message = e
            .field_errors()
            .values()
            .flat_map(|errs| errs.iter())
            .filter_map(|err| err.message.as_ref().map(|m| m.to_string()))
            .next()
            .unwrap_or_else(|| "Invalid request payload".to_string());
        AppError::bad_request(message)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::serializer::auth::{LoginRequest, SignupRequest};

    #[test]
    fn signup_rejects_invalid_email() {
        let payload = SignupRequest {
            name: "A".to_string(),
            surname: "B".to_string(),
            email: "bad-email".to_string(),
            password: "password123".to_string(),
            confirm_password: "password123".to_string(),
        };
        assert!(validate_payload(&payload).is_err());
    }

    #[test]
    fn signup_rejects_short_password() {
        let payload = SignupRequest {
            name: "A".to_string(),
            surname: "B".to_string(),
            email: "a@b.com".to_string(),
            password: "short".to_string(),
            confirm_password: "short".to_string(),
        };
        assert!(validate_payload(&payload).is_err());
    }

    #[test]
    fn login_accepts_valid_payload() {
        let payload = LoginRequest {
            email: "user@example.com".to_string(),
            password: "password123".to_string(),
        };
        assert!(validate_payload(&payload).is_ok());
    }
}
