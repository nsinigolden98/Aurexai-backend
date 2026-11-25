use axum::{extract::FromRequestParts, http::request::Parts};
use axum_extra::extract::CookieJar;

use crate::{
    app::AppState, error::AppError, models::user, services::auth::authenticate_access_token,
};

pub struct CurrentUser(pub user::Model);

impl FromRequestParts<AppState> for CurrentUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let jar = CookieJar::from_request_parts(parts, state)
            .await
            .map_err(|_| AppError::unauthorized("Invalid cookies"))?;
        let access_cookie = jar.get("access_token").map(|c| c.value().to_string());
        let user = authenticate_access_token(&state.db, &state.config, access_cookie).await?;
        Ok(CurrentUser(user))
    }
}

pub struct AdminUser(pub user::Model);

impl FromRequestParts<AppState> for AdminUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let CurrentUser(user) = CurrentUser::from_request_parts(parts, state).await?;
        if !user.is_admin && !user.is_staff {
            return Err(AppError::forbidden("Admin access required"));
        }
        Ok(AdminUser(user))
    }
}
