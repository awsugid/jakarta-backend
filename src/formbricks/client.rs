use super::types::{FormbricksResponse, FormbricksResponseList, FormbricksSurvey};
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
    #[allow(dead_code)]
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

        let body = resp
            .text()
            .await
            .map_err(|e| format!("failed to read FormBricks response text: {e}"))?;
        worker::console_log!("FormBricks raw response: {}", body);

        serde_json::from_str::<FormbricksResponseList>(&body)
            .map_err(|e| format!("failed to parse FormBricks response: {e}"))
    }

    /// Fetch all responses for a survey, paginating automatically up to a safety limit.
    #[allow(dead_code)]
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

            let total = result.meta.as_ref().and_then(|m| m.total).unwrap_or(0) as u32;
            offset += limit;
            pages += 1;

            if offset >= total || pages >= max_pages {
                break;
            }
        }

        Ok(all_responses)
    }

    /// Delete a response by ID.
    ///
    /// Calls `DELETE /api/v2/management/responses/{id}`.
    pub async fn delete_response(&self, response_id: &str) -> Result<(), String> {
        let url = format!(
            "{}/api/v2/management/responses/{}",
            self.base_url, response_id
        );

        let headers = Headers::new();
        headers
            .set("x-api-key", &self.api_key)
            .map_err(|e| format!("failed to set header: {e}"))?;

        let req = Request::new_with_init(
            &url,
            &RequestInit {
                headers,
                method: Method::Delete,
                ..Default::default()
            },
        )
        .map_err(|e| format!("failed to build request: {e}"))?;

        let mut resp = Fetch::Request(req)
            .send()
            .await
            .map_err(|e| format!("request to FormBricks failed: {e}"))?;

        let status = resp.status_code();
        if status != 200 && status != 204 {
            let body = resp.text().await.unwrap_or_default();
            return Err(format!(
                "FormBricks API returned status {status}: {}",
                truncate(&body, 512)
            ));
        }

        Ok(())
    }

    /// Fetch a single response by ID.
    ///
    /// Calls `GET /api/v2/management/responses/{id}`.
    pub async fn get_response(&self, response_id: &str) -> Result<FormbricksResponse, String> {
        let url = format!(
            "{}/api/v2/management/responses/{}",
            self.base_url, response_id
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

        let body = resp
            .text()
            .await
            .map_err(|e| format!("failed to read FormBricks response text: {e}"))?;

        // Fallback-friendly deserialization: try wrapped {"data": FormbricksResponse} first, then direct FormbricksResponse
        #[derive(serde::Deserialize)]
        struct Wrapper {
            data: FormbricksResponse,
        }

        if let Ok(wrapper) = serde_json::from_str::<Wrapper>(&body) {
            Ok(wrapper.data)
        } else {
            serde_json::from_str::<FormbricksResponse>(&body)
                .map_err(|e| format!("failed to parse FormBricks response: {e}"))
        }
    }

    /// Fetch a survey definition including questions.
    ///
    /// Calls `GET /api/v1/management/surveys/{surveyId}`.
    pub async fn get_survey(&self, survey_id: &str) -> Result<FormbricksSurvey, String> {
        let url = format!("{}/api/v1/management/surveys/{}", self.base_url, survey_id);

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

        let body = resp
            .text()
            .await
            .map_err(|e| format!("failed to read FormBricks response text: {e}"))?;

        // Try wrapped response first, then direct
        #[derive(serde::Deserialize)]
        struct Wrapper {
            data: FormbricksSurvey,
        }

        if let Ok(wrapper) = serde_json::from_str::<Wrapper>(&body) {
            Ok(wrapper.data)
        } else {
            serde_json::from_str::<FormbricksSurvey>(&body)
                .map_err(|e| format!("failed to parse FormBricks survey: {e}"))
        }
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
