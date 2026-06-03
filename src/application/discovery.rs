use crate::auth::user::AuthUser;
use crate::formbricks::responses::extract_answers_list;
use crate::http::errors::AppError;
use crate::storage::d1::{ApplicationForm, FormRepository};
use crate::validation::email::normalize_email;
use crate::validation::linkedin::normalize_linkedin_url;
use serde::Serialize;
use wasm_bindgen::prelude::*;

/// Result of discovering the current user's existing application
#[derive(Debug, Serialize)]
pub struct DiscoveryResult {
    /// Whether the current user already has a response
    pub exists: bool,
    /// FormBricks response ID if found
    pub response_id: Option<String>,
    /// Whether the response was marked as finished
    pub finished: Option<bool>,
    /// The email submitted in the form response
    pub submitted_email: Option<String>,
    /// The LinkedIn URL submitted in the form response (normalized)
    pub linkedin_url: Option<String>,
    /// Whether the user can still edit this response
    pub editable: bool,
    /// The timestamp when this application was submitted
    pub submitted_at: Option<String>,
}

/// Result of validating a proposed application
#[derive(Debug, Serialize)]
pub struct ValidationResult {
    pub ok: bool,
    pub code: Option<String>,
    pub message: Option<String>,
}

impl DiscoveryResult {
    pub fn not_found() -> Self {
        Self {
            exists: false,
            response_id: None,
            finished: None,
            submitted_email: None,
            linkedin_url: None,
            editable: false,
            submitted_at: None,
        }
    }
}

/// Discover the current user's existing application in a FormBricks survey.
/// Scans all responses looking for one where the email answer matches the Google email.
pub async fn discover_user_application(
    repo: &FormRepository,
    form: &ApplicationForm,
    user: &AuthUser,
    editable_until: Option<&str>,
) -> Result<DiscoveryResult, AppError> {
    let google_email = user.normalized_email();

    let index = repo
        .get_index_by_form_email(&form.id, &google_email)
        .await
        .map_err(|e| AppError::Internal(format!("Database error: {}", e)))?;

    if let Some(idx) = index {
        let editable = is_editable(editable_until);
        return Ok(DiscoveryResult {
            exists: true,
            response_id: Some(idx.formbricks_response_id),
            finished: Some(idx.finished),
            submitted_email: Some(idx.normalized_email),
            linkedin_url: idx.normalized_linkedin_url,
            editable,
            submitted_at: idx.submitted_at,
        });
    }

    Ok(DiscoveryResult::not_found())
}

/// Validate whether a proposed application submission is allowed.
/// Checks for duplicate LinkedIn URL across all responses in this survey.
pub async fn validate_application(
    repo: &FormRepository,
    form: &ApplicationForm,
    user: &AuthUser,
    proposed_linkedin: &str,
) -> Result<ValidationResult, AppError> {
    let normalized_linkedin = normalize_linkedin_url(proposed_linkedin)
        .map_err(|e| AppError::BadRequest(format!("Invalid LinkedIn URL: {}", e)))?;

    let index = repo
        .get_index_by_form_linkedin(&form.id, &normalized_linkedin)
        .await
        .map_err(|e| AppError::Internal(format!("Database error: {}", e)))?;

    if let Some(idx) = index {
        let google_email = user.normalized_email();
        if idx.normalized_email != google_email {
            return Ok(ValidationResult {
                ok: false,
                code: Some("duplicate_linkedin".to_string()),
                message: Some(
                    "This LinkedIn profile has already been used for this application form."
                        .to_string(),
                ),
            });
        }
    }

    Ok(ValidationResult {
        ok: true,
        code: None,
        message: None,
    })
}

/// Check if the form is still editable based on editable_until date.
fn is_editable(editable_until: Option<&str>) -> bool {
    is_editable_at(editable_until, &utc_now_iso_string())
}

/// Pure-Rust version of `is_editable` that accepts the current time as a parameter.
pub fn is_editable_at(editable_until: Option<&str>, now: &str) -> bool {
    match editable_until {
        None => true,
        Some(deadline) => deadline >= now,
    }
}

/// Check if a FormBricks response's email answer matches the given normalized email.
#[allow(dead_code)]
pub fn response_matches_email(
    response: &crate::formbricks::types::FormbricksResponse,
    email_question_id: &str,
    normalized_email: &str,
) -> bool {
    let email_answers = extract_answers_list(response, email_question_id);
    email_answers
        .iter()
        .any(|e| normalize_email(e) == normalized_email)
}

/// Extract and normalize the LinkedIn URL from a FormBricks response.
#[allow(dead_code)]
pub fn extract_linkedin_normalized(
    response: &crate::formbricks::types::FormbricksResponse,
    linkedin_question_id: &str,
) -> Option<String> {
    let linkedin_answers = extract_answers_list(response, linkedin_question_id);
    linkedin_answers
        .first()
        .and_then(|url| normalize_linkedin_url(url).ok())
}

/// Get current UTC time as ISO 8601 string using JS Date via wasm_bindgen.
/// Format: "YYYY-MM-DDTHH:mm:ss.sssZ"
fn utc_now_iso_string() -> String {
    utc_now_iso_string_js()
}

#[wasm_bindgen(
    inline_js = "export function utc_now_iso_string_js() { return new Date().toISOString(); }"
)]
extern "C" {
    fn utc_now_iso_string_js() -> String;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formbricks::types::FormbricksResponse;
    use std::collections::HashMap;

    fn make_response(id: &str, email: Option<&str>, linkedin: Option<&str>) -> FormbricksResponse {
        let mut data = HashMap::new();
        if let Some(e) = email {
            data.insert(
                "q-email".to_string(),
                serde_json::Value::String(e.to_string()),
            );
        }
        if let Some(l) = linkedin {
            data.insert(
                "q-linkedin".to_string(),
                serde_json::Value::String(l.to_string()),
            );
        }
        FormbricksResponse {
            id: id.to_string(),
            survey_id: "survey-123".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
            finished: true,
            data,
            contact: None,
        }
    }

    // ---- is_editable_at ----

    #[test]
    fn test_editable_no_deadline() {
        assert!(is_editable_at(None, "2026-06-01T00:00:00Z"));
    }

    #[test]
    fn test_editable_deadline_in_future() {
        assert!(is_editable_at(
            Some("2026-07-01T00:00:00Z"),
            "2026-06-01T00:00:00Z"
        ));
    }

    #[test]
    fn test_editable_deadline_in_past() {
        assert!(!is_editable_at(
            Some("2026-05-01T00:00:00Z"),
            "2026-06-01T00:00:00Z"
        ));
    }

    #[test]
    fn test_editable_deadline_exact_now() {
        assert!(is_editable_at(
            Some("2026-06-01T00:00:00Z"),
            "2026-06-01T00:00:00Z"
        ));
    }

    // ---- response_matches_email ----

    #[test]
    fn test_email_match_exact() {
        let resp = make_response("r1", Some("user@example.com"), None);
        assert!(response_matches_email(&resp, "q-email", "user@example.com"));
    }

    #[test]
    fn test_email_match_case_insensitive() {
        let resp = make_response("r1", Some("User@Example.COM"), None);
        assert!(response_matches_email(&resp, "q-email", "user@example.com"));
    }

    #[test]
    fn test_email_match_with_whitespace() {
        let resp = make_response("r1", Some("  user@example.com  "), None);
        assert!(response_matches_email(&resp, "q-email", "user@example.com"));
    }

    #[test]
    fn test_email_no_match() {
        let resp = make_response("r1", Some("other@example.com"), None);
        assert!(!response_matches_email(
            &resp,
            "q-email",
            "user@example.com"
        ));
    }

    #[test]
    fn test_email_missing_question() {
        let resp = make_response("r1", Some("user@example.com"), None);
        assert!(!response_matches_email(
            &resp,
            "q-nonexistent",
            "user@example.com"
        ));
    }

    #[test]
    fn test_email_empty_answer() {
        let mut data = HashMap::new();
        data.insert(
            "q-email".to_string(),
            serde_json::Value::String(String::new()),
        );
        let resp = FormbricksResponse {
            id: "r1".to_string(),
            survey_id: "survey-123".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
            finished: true,
            data,
            contact: None,
        };
        // Empty email normalizes to "", which won't match a real email
        assert!(!response_matches_email(
            &resp,
            "q-email",
            "user@example.com"
        ));
    }

    #[test]
    fn test_email_null_answer() {
        let mut data = HashMap::new();
        data.insert("q-email".to_string(), serde_json::Value::Null);
        let resp = FormbricksResponse {
            id: "r1".to_string(),
            survey_id: "survey-123".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
            finished: true,
            data,
            contact: None,
        };
        assert!(!response_matches_email(
            &resp,
            "q-email",
            "user@example.com"
        ));
    }

    // ---- extract_linkedin_normalized ----

    #[test]
    fn test_linkedin_extract_valid() {
        let resp = make_response("r1", None, Some("https://linkedin.com/in/johndoe"));
        assert_eq!(
            extract_linkedin_normalized(&resp, "q-linkedin"),
            Some("linkedin.com/in/johndoe".to_string())
        );
    }

    #[test]
    fn test_linkedin_extract_with_www() {
        let resp = make_response("r1", None, Some("https://www.linkedin.com/in/janedoe"));
        assert_eq!(
            extract_linkedin_normalized(&resp, "q-linkedin"),
            Some("linkedin.com/in/janedoe".to_string())
        );
    }

    #[test]
    fn test_linkedin_extract_invalid_url() {
        let resp = make_response("r1", None, Some("https://example.com/in/johndoe"));
        assert_eq!(extract_linkedin_normalized(&resp, "q-linkedin"), None);
    }

    #[test]
    fn test_linkedin_extract_missing_question() {
        let resp = make_response("r1", None, Some("https://linkedin.com/in/johndoe"));
        assert_eq!(extract_linkedin_normalized(&resp, "q-nonexistent"), None);
    }

    #[test]
    fn test_linkedin_extract_no_answer() {
        let resp = make_response("r1", Some("user@example.com"), None);
        assert_eq!(extract_linkedin_normalized(&resp, "q-linkedin"), None);
    }

    #[test]
    fn test_linkedin_extract_normalizes_case() {
        let resp = make_response("r1", None, Some("https://LinkedIn.com/in/JohnDoe"));
        assert_eq!(
            extract_linkedin_normalized(&resp, "q-linkedin"),
            Some("linkedin.com/in/johndoe".to_string())
        );
    }
}
