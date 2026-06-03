use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A single response from FormBricks Management API.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
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
#[serde(rename_all = "camelCase")]
pub struct FormbricksContact {
    pub id: String,
    pub attributes: HashMap<String, serde_json::Value>,
}

/// Paginated response from `GET /api/v2/management/responses`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FormbricksResponseList {
    pub data: Vec<FormbricksResponse>,
    #[serde(default)]
    pub meta: Option<FormbricksMeta>,
}

/// Pagination metadata from the FormBricks API.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FormbricksMeta {
    pub total: Option<u64>,
    pub limit: Option<u64>,
    pub offset: Option<u64>,
}

/// Survey info from FormBricks.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FormbricksSurvey {
    pub id: String,
    pub name: String,
    pub status: String,
    /// Legacy flat questions array (older surveys).
    #[serde(default)]
    pub questions: Vec<FormbricksQuestion>,
    /// Newer block-based structure — elements live inside each block.
    #[serde(default)]
    pub blocks: Vec<FormbricksBlock>,
}

/// A single question / element in a FormBricks survey.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FormbricksQuestion {
    pub id: String,
    /// Headline is a localized object: { "default": "..." } — may contain HTML.
    pub headline: serde_json::Value,
    #[serde(rename = "type")]
    pub question_type: String,
}

impl FormbricksQuestion {
    /// Extract the default headline string and strip any HTML tags.
    pub fn headline_text(&self) -> String {
        let raw = match &self.headline {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Object(map) => map
                .get("default")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            _ => return String::new(),
        };
        strip_html(&raw)
    }
}

/// Remove HTML tags from a string, returning plain text.
fn strip_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for ch in s.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    // Collapse whitespace
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A block containing a list of survey elements.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FormbricksBlock {
    pub id: String,
    #[serde(default)]
    pub elements: Vec<FormbricksQuestion>,
}
