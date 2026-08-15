use serde::{Deserialize, Serialize};
use worker::*;

use crate::auth::google::extract_user;
use crate::config::AppConfig;
use crate::http::errors::AppError;
use crate::http::response::{json_success, with_cors};
use crate::storage::d1::{Profile, ProfileRepository};
use crate::validation::profile::{
    normalize_links, normalize_text_field, validate_username, ProfileLink, MAX_DISPLAY_NAME_LEN,
    MAX_TITLE_LEN,
};

#[derive(Debug, Deserialize)]
pub struct ProfilesLookupRequest {
    #[serde(default)]
    pub emails: Vec<String>,
    #[serde(default)]
    pub usernames: Vec<String>,
}

/// Authenticated editor view (camelCase per frozen contract).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MyProfile {
    pub email: String,
    pub username: Option<String>,
    pub display_name: Option<String>,
    pub title: Option<String>,
    pub links: Vec<ProfileLink>,
    pub is_public: bool,
    pub picture: Option<String>,
    pub profile_updated_at: Option<String>,
}

impl MyProfile {
    pub fn from_row(email: String, row: &Profile) -> Self {
        Self {
            email,
            username: row.username.clone(),
            display_name: row.display_name.clone(),
            title: row.title.clone(),
            links: row.links(),
            is_public: row.is_public,
            picture: row.picture.clone(),
            profile_updated_at: row.profile_updated_at.clone(),
        }
    }

    /// Default private profile when no row exists yet.
    pub fn empty(email: String, picture: Option<String>) -> Self {
        Self {
            email,
            username: None,
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
    pub username: String,
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
            username: row.username.clone().unwrap_or_default(),
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
    #[serde(default)]
    pub username: Option<String>,
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
    let username = validate_username(body.username.as_deref())
        .map_err(|e| AppError::BadRequest(e.to_string()))?;
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

    // Check username uniqueness if provided
    if let Some(ref u) = username {
        if let Some(existing) = ProfileRepository::get_profile_by_username(&db, u)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to check username: {e}")))?
        {
            if existing.normalized_email != email {
                return Err(AppError::Conflict("Username is already taken.".to_string()).into());
            }
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
        username.as_deref(),
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

/// POST /api/profiles/lookup — batch lookup published profiles by email or username (max 50).
pub async fn handle_profiles_lookup(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let config = AppConfig::from_env(&ctx.env).map_err(|e| AppError::Internal(e.to_string()))?;
    let db = ctx
        .d1("DB")
        .map_err(|e| AppError::Internal(e.to_string()))?;

    // Save requester's profile snapshot if valid Google token is provided
    let _ = extract_user(&req, &config, Some(&db)).await;

    let req_body: ProfilesLookupRequest = match req.json().await {
        Ok(body) => body,
        Err(_) => return Err(AppError::BadRequest(
            "Invalid JSON body. Expected { \"usernames\": string[] } or { \"emails\": string[] }."
                .to_string(),
        )
        .into()),
    };

    if req_body.emails.len() + req_body.usernames.len() > 50 {
        return Err(AppError::BadRequest(
            "Maximum 50 items allowed per lookup request.".to_string(),
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

    let normalized_usernames: Vec<String> = req_body
        .usernames
        .into_iter()
        .map(|u| u.trim().to_lowercase())
        .filter(|u| !u.is_empty())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    let profiles = if normalized_emails.is_empty() && normalized_usernames.is_empty() {
        Vec::new()
    } else {
        ProfileRepository::lookup_profiles_by_identifiers(
            &db,
            &normalized_emails,
            &normalized_usernames,
        )
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
            username: "avei".into(),
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
            "username",
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
    fn my_profile_serializes_username_when_present() {
        let mut profile = MyProfile::empty("a@b.com".into(), None);
        profile.username = Some("avei".to_string());
        let json = serde_json::to_string(&profile).unwrap();
        assert!(json.contains("\"username\":\"avei\""));
    }

    #[test]
    fn update_request_rejects_snake_case() {
        let raw = r#"{ "display_name": "A", "title": "T", "links": [], "isPublic": true }"#;
        assert!(serde_json::from_str::<ProfileUpdateRequest>(raw).is_err());
    }

    #[test]
    fn update_request_accepts_username() {
        let raw = r#"{ "username": "avei_dev", "displayName": "Avei", "title": "Engineer", "links": [], "isPublic": true }"#;
        let parsed: ProfileUpdateRequest = serde_json::from_str(raw).unwrap();
        assert_eq!(parsed.username.as_deref(), Some("avei_dev"));
        assert_eq!(parsed.display_name.as_deref(), Some("Avei"));
    }

    #[test]
    fn update_request_allows_omitted_username() {
        let raw =
            r#"{ "displayName": "Avei", "title": "Engineer", "links": [], "isPublic": false }"#;
        let parsed: ProfileUpdateRequest = serde_json::from_str(raw).unwrap();
        assert_eq!(parsed.username, None);
    }

    #[test]
    fn lookup_request_deserializes_emails_and_usernames() {
        let raw_both = r#"{ "emails": ["a@b.com"], "usernames": ["avei"] }"#;
        let parsed_both: ProfilesLookupRequest = serde_json::from_str(raw_both).unwrap();
        assert_eq!(parsed_both.emails, vec!["a@b.com"]);
        assert_eq!(parsed_both.usernames, vec!["avei"]);

        let raw_emails = r#"{ "emails": ["a@b.com"] }"#;
        let parsed_emails: ProfilesLookupRequest = serde_json::from_str(raw_emails).unwrap();
        assert_eq!(parsed_emails.emails, vec!["a@b.com"]);
        assert!(parsed_emails.usernames.is_empty());

        let raw_usernames = r#"{ "usernames": ["avei"] }"#;
        let parsed_usernames: ProfilesLookupRequest = serde_json::from_str(raw_usernames).unwrap();
        assert!(parsed_usernames.emails.is_empty());
        assert_eq!(parsed_usernames.usernames, vec!["avei"]);
    }
}
