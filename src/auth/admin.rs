use super::AuthUser;
use crate::config::AppConfig;
use crate::http::errors::AppError;
use worker::Request;

/// Require an authenticated user (any valid Google ID token).
pub async fn require_user(req: &Request, config: &AppConfig) -> Result<AuthUser, AppError> {
    crate::auth::google::extract_user(req, config).await
}

/// Require an authenticated admin (valid Google ID token + email in ADMIN_EMAILS).
pub async fn require_admin(req: &Request, config: &AppConfig) -> Result<AuthUser, AppError> {
    let user = require_user(req, config).await?;
    if !config.is_admin(&user.email) {
        return Err(AppError::Forbidden("Admin access required.".to_string()));
    }
    Ok(user)
}
