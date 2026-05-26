use crate::application::forms::{FormInfo, FormPolicyStatus, FormStatus};
use crate::application::policy;
use crate::http::errors::AppError;
use crate::storage::d1::{ApplicationForm, FormRepository};

/// Get form list, optionally filtered by kind.
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

/// Get a single form with policy status.
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
