use crate::application::discovery::discover_user_application;
use crate::application::forms::{
    FormInfo, FormInfoAuthed, FormPolicyStatus, FormStatus, FormStatusAuthed,
};
use crate::application::policy;
use crate::auth::user::AuthUser;
use crate::formbricks::client::FormbricksClient;
use crate::http::errors::AppError;
use crate::storage::d1::{ApplicationForm, FormRepository};
use serde::Serialize;

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

/// A single entry in the user's applications summary.
#[derive(Debug, Serialize)]
pub struct UserApplicationSummary {
    pub kind: String,
    pub slug: String,
    pub title: String,
    pub description: Option<String>,
    pub response_id: String,
    pub finished: bool,
    pub editable: bool,
    pub submitted_at: String,
}

/// Form metadata in the response-detail payload.
#[derive(Debug, Serialize)]
pub struct ResponseFormInfo {
    pub kind: String,
    pub slug: String,
    pub title: String,
}

/// Full response detail returned by `GET .../response`.
#[derive(Debug, Serialize)]
pub struct ApplicationResponseDetail {
    pub form: ResponseFormInfo,
    pub response: ApplicationResponse,
}

/// The FormBricks response portion.
#[derive(Debug, Serialize)]
pub struct ApplicationResponse {
    pub id: String,
    pub finished: bool,
    pub submitted_at: String,
    pub data: serde_json::Value,
}

/// Get form list without survey_id (public).
pub async fn list_forms(
    repo: &FormRepository,
    kind: Option<&str>,
) -> Result<Vec<FormInfo>, AppError> {
    let forms = repo
        .list_forms(kind)
        .await
        .map_err(|e| AppError::Internal(format!("Database error: {}", e)))?;

    Ok(forms.into_iter().map(FormInfo::from).collect())
}

/// Get form list with survey_id (authenticated).
pub async fn list_forms_authed(
    repo: &FormRepository,
    kind: Option<&str>,
) -> Result<Vec<FormInfoAuthed>, AppError> {
    let forms = repo
        .list_forms(kind)
        .await
        .map_err(|e| AppError::Internal(format!("Database error: {}", e)))?;

    Ok(forms.into_iter().map(FormInfoAuthed::from).collect())
}

/// Get a single form with policy status, without survey_id (public).
pub async fn get_form_status(
    repo: &FormRepository,
    kind: &str,
    slug: &str,
) -> Result<FormStatus, AppError> {
    let form = repo
        .get_form(kind, slug)
        .await
        .map_err(|e| AppError::Internal(format!("Database error: {}", e)))?
        .ok_or_else(|| AppError::NotFound(format!("Form {}/{} not found", kind, slug)))?;

    let status = determine_status(&form);
    Ok(FormStatus {
        form: FormInfo::from(form),
        status,
    })
}

/// Get a single form with policy status, including survey_id (authenticated).
pub async fn get_form_status_authed(
    repo: &FormRepository,
    kind: &str,
    slug: &str,
) -> Result<FormStatusAuthed, AppError> {
    let form = repo
        .get_form(kind, slug)
        .await
        .map_err(|e| AppError::Internal(format!("Database error: {}", e)))?
        .ok_or_else(|| AppError::NotFound(format!("Form {}/{} not found", kind, slug)))?;

    let status = determine_status(&form);
    Ok(FormStatusAuthed {
        form: FormInfoAuthed::from(form),
        status,
    })
}

fn determine_status(form: &ApplicationForm) -> FormPolicyStatus {
    determine_status_at(form, &policy::utc_now_string())
}

/// Pure-Rust core of `determine_status` (testable without the JS clock).
/// Archive wins first; acceptance is kind-aware so an inactive volunteer in
/// its date window reports Open (Talent Pool) while the link endpoint still
/// permits the submission — status and policy can never disagree.
fn determine_status_at(form: &ApplicationForm, now: &str) -> FormPolicyStatus {
    if policy::is_form_archived_at(form, now) {
        FormPolicyStatus::Archived
    } else if !policy::accepting_for_kind_at(form, now) {
        if let Some(ref opens_at) = form.opens_at {
            if opens_at.as_str() > now {
                return FormPolicyStatus::NotYetOpen;
            }
        }
        FormPolicyStatus::Closed
    } else {
        FormPolicyStatus::Open
    }
}

// ---------------------------------------------------------------------------
// Summary & response detail
// ---------------------------------------------------------------------------

/// List all active forms where the authenticated user has an existing response.
pub async fn list_user_applications(
    repo: &FormRepository,
    _client: &FormbricksClient, // Unused now but kept for signature consistency
    user: &AuthUser,
) -> Result<Vec<UserApplicationSummary>, AppError> {
    let forms = repo
        .list_forms(None)
        .await
        .map_err(|e| AppError::Internal(format!("Database error: {}", e)))?;

    let indexes = repo
        .list_indexes_by_email(&user.normalized_email())
        .await
        .map_err(|e| AppError::Internal(format!("Database error: {}", e)))?;

    let mut summaries = Vec::new();

    for form in forms {
        // Find ALL active response indexes for this form (e.g. for multiple speaker submissions)
        let matching_indexes: Vec<_> = indexes.iter().filter(|i| i.form_id == form.id).collect();

        for idx in matching_indexes {
            // Volunteer Talent Pool responses stay editable while inactive.
            let editable = policy::editable_for_kind(&form);
            summaries.push(UserApplicationSummary {
                kind: form.kind.clone(),
                slug: form.slug.clone(),
                title: form.title.clone(),
                description: form.description.clone(),
                response_id: idx.formbricks_response_id.clone(),
                finished: idx.finished,
                editable,
                submitted_at: idx.submitted_at.clone().unwrap_or_default(),
            });
        }
    }

    Ok(summaries)
}

/// Fetch the full response data for a user's existing application.
pub async fn get_user_response(
    repo: &FormRepository,
    client: &FormbricksClient,
    user: &AuthUser,
    kind: &str,
    slug: &str,
) -> Result<ApplicationResponseDetail, AppError> {
    let form = repo
        .get_form(kind, slug)
        .await
        .map_err(|e| AppError::Internal(format!("Database error: {}", e)))?
        .ok_or_else(|| AppError::NotFound(format!("Form {}/{} not found", kind, slug)))?;

    let discovery =
        discover_user_application(repo, &form, user, form.editable_until.as_deref()).await?;

    let response_id = discovery.response_id.ok_or_else(|| {
        AppError::NotFound("No existing response found for this user and form".to_string())
    })?;

    let fb_response = client
        .get_response(&response_id)
        .await
        .map_err(|e| AppError::FormBricksError(format!("Failed to fetch response: {}", e)))?;

    Ok(ApplicationResponseDetail {
        form: ResponseFormInfo {
            kind: form.kind.clone(),
            slug: form.slug.clone(),
            title: form.title.clone(),
        },
        response: ApplicationResponse {
            id: fb_response.id,
            finished: fb_response.finished,
            submitted_at: fb_response.created_at,
            data: serde_json::Value::Object(fb_response.data.into_iter().collect()),
        },
    })
}

/// Build a prefilled FormBricks URL from an existing response.
pub fn build_prefilled_url(
    public_url: &str,
    response_data: &std::collections::HashMap<String, serde_json::Value>,
) -> String {
    let mut params: Vec<String> = Vec::new();
    for (question_id, value) in response_data {
        if value.is_null() {
            continue;
        }
        let answer = if let Some(s) = value.as_str() {
            s.to_string()
        } else {
            value.to_string()
        };
        if answer.is_empty() {
            continue;
        }
        params.push(format!(
            "{}={}",
            url_encode(question_id),
            url_encode(&answer)
        ));
    }
    params.push("skipPrefilled=true".to_string());

    let separator = if public_url.contains('?') { '&' } else { '?' };
    format!("{}{}{}", public_url, separator, params.join("&"))
}

/// Minimal percent-encoding for URL query parameters.
fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            _ => {
                out.push_str(&format!("%{:02X}", byte));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::d1::ApplicationForm;

    fn form(kind: &str, is_active: bool) -> ApplicationForm {
        ApplicationForm {
            id: format!("f-{kind}"),
            kind: kind.to_string(),
            slug: kind.to_string(),
            title: kind.to_string(),
            description: None,
            formbricks_survey_id: "svy".to_string(),
            formbricks_public_url: None,
            email_question_id: "q-email".to_string(),
            linkedin_question_id: "q-linkedin".to_string(),
            is_active,
            opens_at: None,
            closes_at: None,
            editable_until: None,
            archive_after: None,
            display_order: 0,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    const NOW: &str = "2026-06-01T00:00:00Z";

    #[test]
    fn inactive_volunteer_in_window_reports_open_talent_pool() {
        let f = form("volunteer", false);
        assert_eq!(determine_status_at(&f, NOW), FormPolicyStatus::Open);
    }

    #[test]
    fn inactive_speaker_reports_closed() {
        let f = form("speaker", false);
        assert_eq!(determine_status_at(&f, NOW), FormPolicyStatus::Closed);
    }

    #[test]
    fn active_forms_report_open() {
        assert_eq!(
            determine_status_at(&form("volunteer", true), NOW),
            FormPolicyStatus::Open
        );
        assert_eq!(
            determine_status_at(&form("speaker", true), NOW),
            FormPolicyStatus::Open
        );
    }

    #[test]
    fn talent_pool_dates_still_gate_status() {
        let mut f = form("volunteer", false);
        f.opens_at = Some("2026-07-01T00:00:00Z".to_string());
        assert_eq!(determine_status_at(&f, NOW), FormPolicyStatus::NotYetOpen);
        f.opens_at = None;
        f.closes_at = Some("2026-05-01T00:00:00Z".to_string());
        assert_eq!(determine_status_at(&f, NOW), FormPolicyStatus::Closed);
    }

    #[test]
    fn archive_wins_over_talent_pool() {
        let mut f = form("volunteer", false);
        f.archive_after = Some("2026-05-01T00:00:00Z".to_string());
        assert_eq!(determine_status_at(&f, NOW), FormPolicyStatus::Archived);
    }

    #[test]
    fn active_form_past_closes_is_closed() {
        let mut f = form("volunteer", true);
        f.closes_at = Some("2026-05-01T00:00:00Z".to_string());
        assert_eq!(determine_status_at(&f, NOW), FormPolicyStatus::Closed);
    }
}
