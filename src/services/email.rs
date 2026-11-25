use std::sync::Arc;

use minijinja::{Environment, context};
use reqwest::Client;
use serde_json::json;

use crate::{config::Config, error::AppError};

#[derive(Clone)]
pub struct EmailService {
    client: Client,
    api_key: String,
    sender_email: String,
    sender_name: String,
    templates: Arc<Environment<'static>>,
}

impl EmailService {
    pub fn new(config: &Config) -> Self {
        let mut env = Environment::new();
        env.add_template("base.html", include_str!("../../templates/base.html"))
            .expect("valid base template");
        env.add_template("otp.html", include_str!("../../templates/otp.html"))
            .expect("valid otp template");
        env.add_template("welcome.html", include_str!("../../templates/welcome.html"))
            .expect("valid welcome template");
        env.add_template(
            "reset_password.html",
            include_str!("../../templates/reset_password.html"),
        )
        .expect("valid reset password template");

        Self {
            client: Client::new(),
            api_key: config.brevo_api_key.clone(),
            sender_email: config.brevo_sender_email.clone(),
            sender_name: config.brevo_sender_name.clone(),
            templates: Arc::new(env),
        }
    }

    pub async fn send_otp_email(
        &self,
        to_email: &str,
        to_name: &str,
        code: &str,
        expires_in_minutes: i64,
    ) -> Result<(), AppError> {
        let subject = "Verify your Aurexai account";
        let content = self.render(
            "otp.html",
            context! { code => code, expires_in_minutes => expires_in_minutes },
        )?;
        let html = self.render_base("Verify your email", to_name, &content)?;
        self.send(to_email, to_name, subject, &html).await
    }

    pub async fn send_welcome_email(&self, to_email: &str, to_name: &str) -> Result<(), AppError> {
        let subject = "Welcome to Aurexai";
        let content = self.render("welcome.html", context! {})?;
        let html = self.render_base("Welcome to Aurexai", to_name, &content)?;
        self.send(to_email, to_name, subject, &html).await
    }

    pub async fn send_password_reset_email(
        &self,
        to_email: &str,
        to_name: &str,
        reset_link: &str,
        expires_in_minutes: i64,
    ) -> Result<(), AppError> {
        let subject = "Reset your Aurexai password";
        let content = self.render(
            "reset_password.html",
            context! { reset_link => reset_link, expires_in_minutes => expires_in_minutes },
        )?;
        let html = self.render_base("Reset your password", to_name, &content)?;
        self.send(to_email, to_name, subject, &html).await
    }

    fn render_base(&self, title: &str, name: &str, content: &str) -> Result<String, AppError> {
        self.render(
            "base.html",
            context! {
                title => title,
                name => name,
                content => content,
                year => 2026,
            },
        )
    }

    fn render(&self, template: &str, ctx: minijinja::Value) -> Result<String, AppError> {
        let tmpl = self
            .templates
            .get_template(template)
            .map_err(|e| AppError::internal(format!("template error: {e}")))?;
        tmpl.render(ctx)
            .map_err(|e| AppError::internal(format!("template render error: {e}")))
    }

    async fn send(
        &self,
        to_email: &str,
        to_name: &str,
        subject: &str,
        html: &str,
    ) -> Result<(), AppError> {
        if self.api_key.is_empty() {
            tracing::warn!(
                "BREVO_API_KEY is not set; skipping email to {to_email} with subject '{subject}'"
            );
            return Ok(());
        }

        let payload = json!({
            "sender": {
                "name": self.sender_name,
                "email": self.sender_email,
            },
            "to": [
                {
                    "email": to_email,
                    "name": to_name,
                }
            ],
            "subject": subject,
            "htmlContent": html,
        });

        let res = self
            .client
            .post("https://api.brevo.com/v3/smtp/email")
            .header("api-key", &self.api_key)
            .header("accept", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| AppError::internal(format!("failed to send email: {e}")))?;

        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            tracing::error!(%status, body = %body, "Brevo email send failed");
            return Err(AppError::internal("Failed to send email"));
        }

        Ok(())
    }
}
