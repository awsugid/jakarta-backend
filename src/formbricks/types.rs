use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A single response from FormBricks Management API.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FormbricksResponse {
    pub id: String,
    pub survey_id: String,
    pub created_at: String,
    pub updated_at: String,
    pub finished: bool,
    /// Map of question ID -> answer value.
    pub data: HashMap<String, serde_json::Value>,
    /// Contact info if available.
    pub contact: Option<FormbricksContact>,
}

/// Contact information associated with a FormBricks response.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FormbricksContact {
    pub id: String,
    pub attributes: HashMap<String, serde_json::Value>,
}

/// Paginated response from `GET /api/v2/management/responses`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FormbricksResponseList {
    pub data: Vec<FormbricksResponse>,
    pub meta: FormbricksMeta,
}

/// Pagination metadata from the FormBricks API.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FormbricksMeta {
    pub total: Option<u64>,
    pub limit: Option<u64>,
    pub offset: Option<u64>,
}

/// Survey info from FormBricks.
#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FormbricksSurvey {
    pub id: String,
    pub name: String,
    pub status: String,
}
