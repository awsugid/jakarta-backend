use serde::{Deserialize, Serialize};
use worker::*;

use crate::auth::google::extract_user;
use crate::config::AppConfig;
use crate::http::errors::AppError;
use crate::http::response::{json_success, with_cors};
use crate::storage::d1::{Profile, ProfileRepository};
use crate::validation::profile::{
    normalize_links, normalize_text_field, ProfileLink, MAX_DISPLAY_NAME_LEN, MAX_TITLE_LEN,
};

#[derive(Debug, Deserialize)]
pub struct ProfilesLookupRequest {
    pub emails: Vec<String>,
}

/// Authenticated editor view (camelCase per frozen contract).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MyProfile {
    pub email: String,
    pub display_name: Option<String>,
    pub title: Option<String>,
    pub links: Vec<ProfileLink>,
    pub is_public: bool,
    pub picture: Option<String>,
    pub profile_updated_at: Option<String>,
}

impl MyProfile {
    fn from_row(email: String, row: &Profile) -> Self {
        Self {
            email,
            display_name: row.display_name.clone(),
            title: row.title.clone(),
            links: row.links(),
            is_public: row.is_public,
            picture: row.picture.clone(),
            profile_updated_at: row.profile_updated_at.clone(),
        }
    }

    /// Default private profile when no row exists yet.
    fn empty(email: String, picture: Option<String>) -> Self {
        Self {
            email,
            display_name: None,
            title: None,
            links: Vec::new(),
            is_public: false,
            picture,
            profile_updated_at: None,
        }
    }
}

/// Public lookup view (snake_case per frozen contract; no private fields).
#[derive(Debug, Serialize)]
pub struct PublicProfile {
    pub normalized_email: String,
    pub display_name: String,
    pub title: String,
    pub links: Vec<ProfileLink>,
    pub picture: Option<String>,
    pub profile_updated_at: String,
}

impl PublicProfile {
    fn from_row(row: &Profile) -> Self {
        Self {
            normalized_email: row.normalized_email.clone(),
            display_name: row.display_name.clone().unwrap_or_default(),
            title: row.title.clone().unwrap_or_default(),
            links: row.links(),
            picture: row.picture.clone(),
            profile_updated_at: row.profile_updated_at.clone().unwrap_or_default(),
        }
    }
}

/// PUT /api/profiles/me request body. Email is never accepted here;
/// row ownership comes exclusively from the verified token.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProfileUpdateRequest {
    pub display_name: Option<String>,
    pub title: Option<String>,
    #[serde(default)]
    pub links: Vec<ProfileLink>,
    pub is_public: bool,
}

/// GET /api/profiles/me — the caller's own profile.
pub async fn handle_profiles_me_get(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let config = AppConfig::from_env(&ctx.env).map_err(|e| AppError::Internal(e.to_string()))?;
    let db = ctx
        .d1("DB")
        .map_err(|e| AppError::Internal(e.to_string()))?;

    // extract_user snapshots name/picture first, so a row normally exists.
    let user = extract_user(&req, &config, Some(&db)).await?;
    let email = user.normalized_email();

    let my = match ProfileRepository::get_profile_by_email(&db, &email).await {
        Ok(Some(row)) => MyProfile::from_row(email, &row),
        Ok(None) => MyProfile::empty(email, user.picture.clone()),
        Err(e) => return Err(AppError::Internal(format!("Failed to load profile: {e}")).into()),
    };

    let resp = json_success(&my)?;
    with_cors(resp, &config.allowed_origins)
}

/// PUT /api/profiles/me — atomically replace the caller's editable fields.
pub async fn handle_profiles_me_put(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let config = AppConfig::from_env(&ctx.env).map_err(|e| AppError::Internal(e.to_string()))?;
    let db = ctx
        .d1("DB")
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let user = extract_user(&req, &config, Some(&db)).await?;
    let email = user.normalized_email();

    let body: ProfileUpdateRequest = req
        .json()
        .await
        .map_err(|_| AppError::BadRequest("Invalid JSON body.".to_string()))?;

    // Validate everything before writing; no partial updates.
    let display_name =
        normalize_text_field(&body.display_name, MAX_DISPLAY_NAME_LEN, "displayName")
            .map_err(AppError::BadRequest)?;
    let title =
        normalize_text_field(&body.title, MAX_TITLE_LEN, "title").map_err(AppError::BadRequest)?;
    let links = normalize_links(body.links).map_err(AppError::BadRequest)?;

    if body.is_public {
        if display_name.is_none() {
            return Err(AppError::BadRequest(
                "displayName is required to publish a profile.".to_string(),
            )
            .into());
        }
        if title.is_none() {
            return Err(AppError::BadRequest(
                "title is required to publish a profile.".to_string(),
            )
            .into());
        }
    }

    let links_json = serde_json::to_string(&links)
        .map_err(|e| AppError::Internal(format!("Failed to serialize links: {e}")))?;

    ProfileRepository::update_profile_details(
        &db,
        &email,
        display_name.as_deref(),
        title.as_deref(),
        &links_json,
        body.is_public,
    )
    .await
    .map_err(|e| AppError::Internal(format!("Failed to save profile: {e}")))?;

    let row = ProfileRepository::get_profile_by_email(&db, &email)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to load profile: {e}")))?
        .ok_or_else(|| AppError::Internal("Profile row missing after save".to_string()))?;

    let resp = json_success(&MyProfile::from_row(email, &row))?;
    with_cors(resp, &config.allowed_origins)
}

/// POST /api/profiles/lookup — batch lookup published profiles by email (max 50).
pub async fn handle_profiles_lookup(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let config = AppConfig::from_env(&ctx.env).map_err(|e| AppError::Internal(e.to_string()))?;
    let db = ctx
        .d1("DB")
        .map_err(|e| AppError::Internal(e.to_string()))?;

    // Save requester's profile snapshot if valid Google token is provided
    let _ = extract_user(&req, &config, Some(&db)).await;

    let req_body: ProfilesLookupRequest = match req.json().await {
        Ok(body) => body,
        Err(_) => {
            return Err(AppError::BadRequest(
                "Invalid JSON body. Expected { \"emails\": string[] }.".to_string(),
            )
            .into())
        }
    };

    if req_body.emails.len() > 50 {
        return Err(AppError::BadRequest(
            "Maximum 50 emails allowed per lookup request.".to_string(),
        )
        .into());
    }

    let normalized_emails: Vec<String> = req_body
        .emails
        .into_iter()
        .map(|e| e.trim().to_lowercase())
        .filter(|e| !e.is_empty())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    let profiles = if normalized_emails.is_empty() {
        Vec::new()
    } else {
        ProfileRepository::lookup_profiles(&db, &normalized_emails)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to lookup profiles: {e}")))?
    };

    let public: Vec<PublicProfile> = profiles.iter().map(PublicProfile::from_row).collect();
    let resp = json_success(&serde_json::json!({ "profiles": public }))?;
    with_cors(resp, &config.allowed_origins)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_profile_omits_private_fields() {
        let json = serde_json::to_string(&PublicProfile {
            normalized_email: "a@b.com".into(),
            display_name: "A".into(),
            title: "T".into(),
            links: vec![],
            picture: None,
            profile_updated_at: "2026-01-01".into(),
        })
        .unwrap();
        let keys: std::collections::HashSet<String> =
            serde_json::from_str::<std::collections::HashMap<String, serde_json::Value>>(&json)
                .unwrap()
                .into_keys()
                .collect();
        let expected: std::collections::HashSet<String> = [
            "normalized_email",
            "display_name",
            "title",
            "links",
            "picture",
            "profile_updated_at",
        ]
        .into_iter()
        .map(String::from)
        .collect();
        assert_eq!(keys, expected);
    }

    #[test]
    fn my_profile_is_camel_case() {
        let json = serde_json::to_string(&MyProfile::empty("a@b.com".into(), None)).unwrap();
        assert!(json.contains("displayName"));
        assert!(json.contains("isPublic"));
        assert!(json.contains("profileUpdatedAt"));
    }

    #[test]
    fn update_request_rejects_snake_case() {
        let raw = r#"{ "display_name": "A", "title": "T", "links": [], "isPublic": true }"#;
        assert!(serde_json::from_str::<ProfileUpdateRequest>(raw).is_err());
    }
}
