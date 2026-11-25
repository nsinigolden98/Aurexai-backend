use std::env;

#[derive(Clone, Debug)]
pub struct Config {
    pub app_env: String,
    pub app_host: String,
    pub app_port: u16,
    pub database_url: String,

    pub frontend_origin: String,
    pub frontend_reset_password_url: String,
    pub allowed_origins: Vec<String>,
    pub allowed_hosts: Vec<String>,

    pub access_token_secret: String,
    pub refresh_token_secret: String,
    pub access_token_ttl_minutes: i64,
    pub refresh_token_ttl_days: i64,

    pub cookie_secure: bool,
    pub cookie_samesite: String,
    pub cookie_domain: Option<String>,

    pub brevo_api_key: String,
    pub brevo_sender_email: String,
    pub brevo_sender_name: String,

    pub google_client_id: String,

    pub reset_token_ttl_minutes: i64,
    pub otp_ttl_minutes: i64,
    pub otp_resend_cooldown_seconds: i64,
    pub otp_max_attempts: i32,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        let app_env = env::var("APP_ENV").unwrap_or_else(|_| "development".to_string());
        let app_host = env::var("APP_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
        let app_port = env::var("APP_PORT")
            .unwrap_or_else(|_| "8000".to_string())
            .parse::<u16>()
            .map_err(|_| "APP_PORT must be a valid u16".to_string())?;

        let database_url = required("DATABASE_URL")?;
        let frontend_origin =
            env::var("FRONTEND_ORIGIN").unwrap_or_else(|_| "http://localhost:3000".to_string());
        let frontend_reset_password_url = env::var("FRONTEND_RESET_PASSWORD_URL")
            .unwrap_or_else(|_| "http://localhost:3000/reset-password".to_string());

        let allowed_origins =
            csv_env("ALLOWED_ORIGINS").unwrap_or_else(|| vec![frontend_origin.clone()]);
        let allowed_hosts = csv_env("ALLOWED_HOSTS").unwrap_or_else(|| {
            vec![
                format!("localhost:{app_port}"),
                format!("127.0.0.1:{app_port}"),
            ]
        });

        let access_token_secret = required("ACCESS_TOKEN_SECRET")?;
        let refresh_token_secret = required("REFRESH_TOKEN_SECRET")?;
        let access_token_ttl_minutes = env_i64("ACCESS_TOKEN_TTL_MINUTES", 15)?;
        let refresh_token_ttl_days = env_i64("REFRESH_TOKEN_TTL_DAYS", 7)?;

        let cookie_secure = env_bool("COOKIE_SECURE", false)?;
        let cookie_samesite = env::var("COOKIE_SAMESITE").unwrap_or_else(|_| "None".to_string());
        let cookie_domain = env::var("COOKIE_DOMAIN").ok().filter(|v| !v.is_empty());

        let brevo_api_key = env::var("BREVO_API_KEY").unwrap_or_default();
        let brevo_sender_email =
            env::var("BREVO_SENDER_EMAIL").unwrap_or_else(|_| "no-reply@example.com".to_string());
        let brevo_sender_name =
            env::var("BREVO_SENDER_NAME").unwrap_or_else(|_| "Aurexai".to_string());

        let google_client_id = env::var("GOOGLE_CLIENT_ID").unwrap_or_default();

        let reset_token_ttl_minutes = env_i64("RESET_TOKEN_TTL_MINUTES", 30)?;
        let otp_ttl_minutes = env_i64("OTP_TTL_MINUTES", 10)?;
        let otp_resend_cooldown_seconds = env_i64("OTP_RESEND_COOLDOWN_SECONDS", 60)?;
        let otp_max_attempts = env_i32("OTP_MAX_ATTEMPTS", 5)?;

        Ok(Self {
            app_env,
            app_host,
            app_port,
            database_url,
            frontend_origin,
            frontend_reset_password_url,
            allowed_origins,
            allowed_hosts,
            access_token_secret,
            refresh_token_secret,
            access_token_ttl_minutes,
            refresh_token_ttl_days,
            cookie_secure,
            cookie_samesite,
            cookie_domain,
            brevo_api_key,
            brevo_sender_email,
            brevo_sender_name,
            google_client_id,
            reset_token_ttl_minutes,
            otp_ttl_minutes,
            otp_resend_cooldown_seconds,
            otp_max_attempts,
        })
    }

    pub fn is_production(&self) -> bool {
        self.app_env == "production"
    }
}

fn required(key: &str) -> Result<String, String> {
    env::var(key).map_err(|_| format!("{key} must be set"))
}

fn csv_env(key: &str) -> Option<Vec<String>> {
    env::var(key).ok().map(|v| {
        v.split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    })
}

fn env_i64(key: &str, default: i64) -> Result<i64, String> {
    match env::var(key) {
        Ok(v) => v
            .parse::<i64>()
            .map_err(|_| format!("{key} must be a valid integer")),
        Err(_) => Ok(default),
    }
}

fn env_i32(key: &str, default: i32) -> Result<i32, String> {
    match env::var(key) {
        Ok(v) => v
            .parse::<i32>()
            .map_err(|_| format!("{key} must be a valid integer")),
        Err(_) => Ok(default),
    }
}

fn env_bool(key: &str, default: bool) -> Result<bool, String> {
    match env::var(key) {
        Ok(v) => match v.to_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => Ok(true),
            "0" | "false" | "no" | "off" => Ok(false),
            _ => Err(format!("{key} must be a boolean")),
        },
        Err(_) => Ok(default),
    }
}
