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
    /// Calls `GET /api/v2/management/responses?surveyId=xxx&limit=xx&skip=xx`.
    /// Upstream v2 has no `offset` param (unknown params are dropped), so the
    /// public `offset` is sent as `skip`; the response meta echoes it as `offset`.
    #[allow(dead_code)]
    pub async fn list_responses(
        &self,
        survey_id: &str,
        limit: u32,
        offset: u32,
    ) -> Result<FormbricksResponseList, String> {
        let url = build_responses_url(&self.base_url, survey_id, limit, offset);

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

        let parsed = serde_json::from_str::<FormbricksResponseList>(&body)
            .map_err(|e| format!("failed to parse FormBricks response: {e}"));
        if parsed.is_ok() {
            worker::console_log!("FormBricks survey {} responses page fetched", survey_id);
        }
        parsed
    }

    /// Fetch all responses for a survey, paginating automatically up to a safety limit.
    /// Fails loudly if `meta.total` is missing or the safety cap is hit, rather than
    /// silently returning a truncated set.
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
            let total = require_total(&result)?;
            all_responses.extend(result.data);
            offset += limit;
            pages += 1;

            if !should_continue_paging(offset, total, pages, max_pages)? {
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

/// Build the upstream v2 responses list URL. The paging param is `skip`:
/// upstream zod validation drops unknown params, so an `offset=` query would be
/// silently ignored and every page would return the first page.
fn build_responses_url(base_url: &str, survey_id: &str, limit: u32, skip: u32) -> String {
    format!(
        "{}/api/v2/management/responses?surveyId={}&limit={}&skip={}",
        base_url, survey_id, limit, skip
    )
}

/// Extract `meta.total`, failing loudly when upstream omits it.
/// A missing total previously defaulted to 0, silently truncating to the first page.
fn require_total(list: &FormbricksResponseList) -> Result<u64, String> {
    list.meta.as_ref().and_then(|m| m.total).ok_or_else(|| {
        "FormBricks response page omitted meta.total; cannot paginate safely".to_string()
    })
}

/// Decide whether another page is needed. Errors instead of silently truncating
/// when the safety cap would stop the walk before all pages are fetched.
/// Exhaustion is checked first so finishing exactly at the cap is not an error.
fn should_continue_paging(
    skip: u32,
    total: u64,
    pages: u32,
    max_pages: u32,
) -> Result<bool, String> {
    if skip as u64 >= total {
        Ok(false)
    } else if pages >= max_pages {
        Err(format!(
            "safety cap of {max_pages} pages reached with {total} total responses; refusing to silently truncate"
        ))
    } else {
        Ok(true)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formbricks::types::FormbricksMeta;

    #[test]
    fn responses_url_uses_skip_not_offset() {
        // Regression: upstream v2 reads `skip`; an `offset=` param is dropped by
        // zod validation, making every page request return the first page.
        let url = build_responses_url("https://fb.example.com", "cm1survey", 50, 100);
        assert!(url.contains("surveyId=cm1survey"));
        assert!(url.contains("limit=50"));
        assert!(url.contains("skip=100"));
        assert!(!url.contains("offset"));
    }

    fn list_with_total(total: Option<u64>) -> FormbricksResponseList {
        FormbricksResponseList {
            data: vec![],
            meta: total.map(|t| FormbricksMeta {
                total: Some(t),
                limit: None,
                offset: None,
            }),
        }
    }

    #[test]
    fn require_total_fails_when_missing() {
        // Regression: missing meta/total used to default to 0 and silently
        // truncate get_all_responses to the first page.
        assert!(require_total(&list_with_total(None)).is_err());
        let no_total = FormbricksResponseList {
            data: vec![],
            meta: Some(FormbricksMeta {
                total: None,
                limit: None,
                offset: None,
            }),
        };
        assert!(require_total(&no_total).is_err());
        assert_eq!(require_total(&list_with_total(Some(250))).unwrap(), 250);
    }

    #[test]
    fn paging_stops_only_when_exhausted() {
        assert!(should_continue_paging(200, 250, 2, 10).unwrap());
        assert!(!should_continue_paging(250, 250, 3, 10).unwrap());
        assert!(!should_continue_paging(300, 250, 3, 10).unwrap());
    }

    #[test]
    fn paging_cap_fails_instead_of_truncating() {
        // Regression: hitting max_pages used to break the loop silently,
        // returning a truncated set as if it were complete.
        // 1000 responses at page size 100 needs 10 pages; cap of 5 must error.
        assert!(should_continue_paging(500, 1000, 5, 5).is_err());
        // Finishing exactly at the cap is success, not an error.
        assert!(!should_continue_paging(1000, 1000, 10, 10).unwrap());
    }
}
