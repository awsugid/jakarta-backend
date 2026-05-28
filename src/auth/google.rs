use worker::{Env, Request};

use super::AuthUser;
use crate::http::errors::AppError;

/// Extract and validate authenticated user from the request.
///
/// In phase 1, we support two approaches:
/// 1. Cloudflare Access JWT (preferred) - validate the Cf-Access-Jwt-Assertion header
/// 2. Google ID token from Authorization: Bearer header (alternative)
///
/// For now, implement a stub that:
/// - Checks for a dev-mode header for local development
/// - Checks for Authorization header with Bearer token
/// - Returns AppError::Unauthorized if no auth header found
pub fn extract_user(req: &Request, _env: &Env) -> Result<AuthUser, AppError> {
    let headers = req.headers();

    // Check for dev-mode header first
    if let Ok(Some(email)) = headers.get("X-Debug-User-Email") {
        return Ok(AuthUser {
            sub: format!("debug-{email}"),
            email,
            name: Some("Debug User".to_string()),
            picture: None,
        });
    }

    // Check for Authorization: Bearer <token>
    if let Ok(Some(auth_header)) = headers.get("Authorization") {
        if let Some(_token) = auth_header.strip_prefix("Bearer ") {
            // TODO: Validate JWT token with Google JWKS or Cloudflare Access
            // For now, return unauthorized with a message
            return Err(AppError::Unauthorized(
                "JWT validation not yet implemented. Use X-Debug-User-Email header for development."
                    .to_string(),
            ));
        }
    }

    Err(AppError::Unauthorized(
        "Authentication required".to_string(),
    ))
}
