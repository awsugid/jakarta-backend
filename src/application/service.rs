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
    if policy::is_form_archived(form) {
        FormPolicyStatus::Archived
    } else if !policy::is_form_open(form) {
        let now = policy::utc_now_string();
        if let Some(ref opens_at) = form.opens_at {
            if opens_at > &now {
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
    client: &FormbricksClient,
    user: &AuthUser,
) -> Result<Vec<UserApplicationSummary>, AppError> {
    let forms = repo
        .list_forms(None)
        .await
        .map_err(|e| AppError::Internal(format!("Database error: {}", e)))?;

    let mut summaries = Vec::new();

    for form in &forms {
        let discovery =
            discover_user_application(client, form, user, form.editable_until.as_deref()).await?;

        if discovery.exists {
            if let Some(response_id) = discovery.response_id {
                let editable = policy::is_form_editable(form);
                summaries.push(UserApplicationSummary {
                    kind: form.kind.clone(),
                    slug: form.slug.clone(),
                    title: form.title.clone(),
                    description: form.description.clone(),
                    response_id,
                    finished: discovery.finished.unwrap_or(false),
                    editable,
                    submitted_at: discovery.submitted_at.unwrap_or_default(),
                });
            }
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
        discover_user_application(client, &form, user, form.editable_until.as_deref()).await?;

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
