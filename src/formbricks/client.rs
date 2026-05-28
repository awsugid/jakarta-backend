use super::types::{FormbricksResponse, FormbricksResponseList};
use crate::config::AppConfig;
use worker::{Fetch, Headers, Method, Request, RequestInit};

/// Client for the FormBricks Management API v2.
pub struct FormbricksClient {
    base_url: String,
    api_key: String,
}

impl FormbricksClient {
    pub fn new(config: &AppConfig) -> Self {
        Self {
            base_url: config.formbricks_base_url.trim_end_matches('/').to_string(),
            api_key: config.formbricks_api_key.clone(),
        }
    }

    /// List responses for a survey with pagination.
    ///
    /// Calls `GET /api/v2/management/responses?surveyId=xxx&limit=xx&offset=xx`.
    pub async fn list_responses(
        &self,
        survey_id: &str,
        limit: u32,
        offset: u32,
    ) -> Result<FormbricksResponseList, String> {
        let url = format!(
            "{}/api/v2/management/responses?surveyId={}&limit={}&offset={}",
            self.base_url, survey_id, limit, offset
        );

        let headers = Headers::new();
        headers
            .set("x-api-key", &self.api_key)
            .map_err(|e| format!("failed to set header: {e}"))?;
        headers
            .set("Accept", "application/json")
            .map_err(|e| format!("failed to set header: {e}"))?;

        let req = Request::new_with_init(
            &url,
            &RequestInit {
                headers,
                method: Method::Get,
                ..Default::default()
            },
        )
        .map_err(|e| format!("failed to build request: {e}"))?;

        let mut resp = Fetch::Request(req)
            .send()
            .await
            .map_err(|e| format!("request to FormBricks failed: {e}"))?;

        let status = resp.status_code();
        if status != 200 {
            let body = resp.text().await.unwrap_or_default();
            return Err(format!(
                "FormBricks API returned status {status}: {}",
                truncate(&body, 512)
            ));
        }

        resp.json::<FormbricksResponseList>()
            .await
            .map_err(|e| format!("failed to parse FormBricks response: {e}"))
    }

    /// Fetch all responses for a survey, paginating automatically up to a safety limit.
    pub async fn get_all_responses(
        &self,
        survey_id: &str,
        max_pages: u32,
    ) -> Result<Vec<FormbricksResponse>, String> {
        let mut all_responses = Vec::new();
        let limit = 100u32;
        let mut offset = 0u32;
        let mut pages = 0u32;

        loop {
            let result = self.list_responses(survey_id, limit, offset).await?;
            all_responses.extend(result.data);

            let total = result.meta.total.unwrap_or(0) as u32;
            offset += limit;
            pages += 1;

            if offset >= total || pages >= max_pages {
                break;
            }
        }

        Ok(all_responses)
    }
}

/// Truncate a string to `max` characters for safe error reporting.
fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max {
        s
    } else {
        // Find a valid char boundary near `max`.
        let mut end = max;
        while end > 0 && !s.is_char_boundary(end) {
            end -= 1;
        }
        &s[..end]
    }
}
