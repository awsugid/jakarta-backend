use jsonwebtoken::jwk::JwkSet;
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use serde::Deserialize;
use worker::{D1Database, Fetch, Headers, Method, Request, RequestInit};

use super::AuthUser;
use crate::config::AppConfig;
use crate::http::errors::AppError;
use crate::storage::d1::ProfileRepository;

const GOOGLE_JWKS_URL: &str = "https://www.googleapis.com/oauth2/v3/certs";

#[derive(Debug, Deserialize)]
struct GoogleClaims {
    sub: String,
    email: String,
    email_verified: bool,
    name: Option<String>,
    picture: Option<String>,
}

/// Save user profile snapshot to D1 database.
pub async fn save_profile_snapshot(db: &D1Database, user: &AuthUser) {
    let _ = ProfileRepository::upsert_profile(
        db,
        &user.normalized_email(),
        user.name.as_deref(),
        user.picture.as_deref(),
    )
    .await;
}

/// Extract and validate authenticated user from the request, saving profile snapshot if DB is provided.
pub async fn extract_user(
    req: &Request,
    config: &AppConfig,
    db: Option<&D1Database>,
) -> Result<AuthUser, AppError> {
    let user = extract_user_inner(req, config).await?;
    if let Some(db) = db {
        save_profile_snapshot(db, &user).await;
    }
    Ok(user)
}

async fn extract_user_inner(req: &Request, config: &AppConfig) -> Result<AuthUser, AppError> {
    let headers = req.headers();

    if config.enable_debug_auth {
        if let Ok(Some(email)) = headers.get("X-Debug-User-Email") {
            return Ok(AuthUser {
                sub: format!("debug-{email}"),
                email,
                name: Some("Debug User".to_string()),
                picture: None,
            });
        }
    }

    if let Ok(Some(auth_header)) = headers.get("Authorization") {
        if let Some(token) = auth_header.strip_prefix("Bearer ") {
            return validate_google_id_token(token, config).await;
        }
    }

    Err(AppError::Unauthorized(
        "Authentication required".to_string(),
    ))
}

#[wasm_bindgen::prelude::wasm_bindgen(inline_js = "export function atob_js(s) { return atob(s); }")]
extern "C" {
    fn atob_js(s: &str) -> String;
}

async fn validate_google_id_token(token: &str, config: &AppConfig) -> Result<AuthUser, AppError> {
    if config.enable_debug_auth && token.starts_with("dummy.") {
        let parts: Vec<&str> = token.split('.').collect();
        if parts.len() == 3 {
            let decoded = atob_js(parts[1]);
            if let Ok(claims) = serde_json::from_str::<GoogleClaims>(&decoded) {
                return Ok(AuthUser {
                    sub: claims.sub,
                    email: claims.email,
                    name: claims.name,
                    picture: claims.picture,
                });
            }
        }
    }

    if config.google_client_id.is_empty() {
        return Err(AppError::Unauthorized(
            "Google authentication is not configured.".to_string(),
        ));
    }

    let header = decode_header(token)
        .map_err(|_| AppError::Unauthorized("Invalid Google ID token header.".to_string()))?;

    if header.alg != Algorithm::RS256 {
        return Err(AppError::Unauthorized(
            "Unsupported Google ID token algorithm.".to_string(),
        ));
    }

    let kid = header
        .kid
        .ok_or_else(|| AppError::Unauthorized("Google ID token is missing key id.".to_string()))?;
    let jwks = fetch_google_jwks().await?;
    let jwk = jwks
        .find(&kid)
        .ok_or_else(|| AppError::Unauthorized("Google signing key was not found.".to_string()))?;

    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_audience(&[config.google_client_id.as_str()]);
    validation.set_issuer(&["https://accounts.google.com", "accounts.google.com"]);
    validation.set_required_spec_claims(&["exp", "sub", "iss", "aud"]);
    validation.leeway = 60;

    let decoding_key = DecodingKey::from_jwk(jwk)
        .map_err(|_| AppError::Unauthorized("Invalid Google signing key.".to_string()))?;
    let token_data = decode::<GoogleClaims>(token, &decoding_key, &validation)
        .map_err(|_| AppError::Unauthorized("Invalid Google ID token.".to_string()))?;
    let claims = token_data.claims;

    if !claims.email_verified {
        return Err(AppError::Unauthorized(
            "Google account email is not verified.".to_string(),
        ));
    }

    Ok(AuthUser {
        sub: claims.sub,
        email: claims.email,
        name: claims.name,
        picture: claims.picture,
    })
}

async fn fetch_google_jwks() -> Result<JwkSet, AppError> {
    let headers = Headers::new();
    headers
        .set("Accept", "application/json")
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let req = Request::new_with_init(
        GOOGLE_JWKS_URL,
        &RequestInit {
            headers,
            method: Method::Get,
            ..Default::default()
        },
    )
    .map_err(|e| AppError::Internal(e.to_string()))?;

    let mut resp = Fetch::Request(req)
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to fetch Google JWKS: {e}")))?;

    if resp.status_code() != 200 {
        return Err(AppError::Unauthorized(
            "Unable to fetch Google signing keys.".to_string(),
        ));
    }

    resp.json::<JwkSet>()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to parse Google JWKS: {e}")))
}
