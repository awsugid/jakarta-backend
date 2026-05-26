use crate::storage::d1::ApplicationForm;
use serde::Serialize;

/// Public form info returned by the API.
#[derive(Debug, Serialize)]
pub struct FormInfo {
    pub kind: String,
    pub slug: String,
    pub title: String,
    pub description: Option<String>,
    pub survey_id: String,
    pub is_active: bool,
    pub opens_at: Option<String>,
    pub closes_at: Option<String>,
    pub editable_until: Option<String>,
}

impl From<ApplicationForm> for FormInfo {
    fn from(f: ApplicationForm) -> Self {
        Self {
            kind: f.kind,
            slug: f.slug,
            title: f.title,
            description: f.description,
            survey_id: f.formbricks_survey_id,
            is_active: f.is_active,
            opens_at: f.opens_at,
            closes_at: f.closes_at,
            editable_until: f.editable_until,
        }
    }
}

/// Form status response with policy info.
#[derive(Debug, Serialize)]
pub struct FormStatus {
    pub form: FormInfo,
    pub status: FormPolicyStatus,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FormPolicyStatus {
    Open,
    Closed,
    NotYetOpen,
    Archived,
}
