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

    /// Fetch all responses for a survey, paginating automatically up to a safety cap.
    ///
    /// Pages advance by the number of items actually received (the server may cap
    /// pages below the requested limit of 100) and the walk stops on the first
    /// empty page; a known `meta.total` short-circuits the walk early. The safety
    /// cap and repeated-page guard fail loudly instead of silently truncating.
    #[allow(dead_code)]
    pub async fn get_all_responses(
        &self,
        survey_id: &str,
        max_pages: u32,
    ) -> Result<Vec<FormbricksResponse>, String> {
        paginate_all_responses(
            |limit, skip| self.list_responses(survey_id, limit, skip),
            max_pages,
        )
        .await
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

/// Page walk shared by `get_all_responses` and the tests: `fetch_page` returns the
/// page for a (limit, skip) request. Kept as a free function so tests can inject
/// fixture pages without network access.
///
/// `meta.total` is optional: when present it short-circuits the walk once the
/// collected count reaches it; when missing the walk advances by the actual page
/// size (the server may cap pages below the requested limit) and stops on the
/// first empty page. The safety cap and a repeated-first-id guard prevent silent
/// truncation and non-advancing pagination.
async fn paginate_all_responses<F, Fut>(
    mut fetch_page: F,
    max_pages: u32,
) -> Result<Vec<FormbricksResponse>, String>
where
    F: FnMut(u32, u32) -> Fut,
    Fut: std::future::Future<Output = Result<FormbricksResponseList, String>>,
{
    let limit = 100u32;
    let mut all_responses = Vec::new();
    let mut skip = 0u32;
    let mut pages = 0u32;
    let mut last_first_id: Option<String> = None;

    loop {
        let page = fetch_page(limit, skip).await?;
        pages += 1;

        if let Some(first_id) = page.data.first().map(|r| r.id.clone()) {
            if last_first_id.as_deref() == Some(first_id.as_str()) {
                return Err(format!(
                    "FormBricks page at skip={skip} repeated first response id {first_id}; pagination is not advancing"
                ));
            }
            last_first_id = Some(first_id);
        }

        let page_len = page.data.len();
        if page_len == 0 {
            return Ok(all_responses);
        }

        all_responses.extend(page.data);
        skip += page_len as u32;

        if let Some(total) = page.meta.as_ref().and_then(|m| m.total) {
            if skip as u64 >= total {
                return Ok(all_responses);
            }
        }

        if pages >= max_pages {
            return Err(format!(
                "safety cap of {max_pages} pages reached after {} responses; refusing to silently truncate",
                all_responses.len()
            ));
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

    // --- async pagination harness (std-only; no async runtime dependency) ---

    /// Minimal executor: fixture futures are immediately ready, so poll to
    /// completion with a no-op waker instead of pulling in tokio.
    fn block_on<F: std::future::Future>(fut: F) -> F::Output {
        use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
        fn noop(_: *const ()) {}
        fn clone(p: *const ()) -> RawWaker {
            RawWaker::new(p, &VTABLE)
        }
        static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, noop, noop, noop);
        // SAFETY: vtable functions are no-ops and never dereference the data pointer.
        let waker = unsafe { Waker::from_raw(RawWaker::new(std::ptr::null(), &VTABLE)) };
        let mut cx = Context::from_waker(&waker);
        let mut fut = std::pin::pin!(fut);
        loop {
            if let Poll::Ready(out) = fut.as_mut().poll(&mut cx) {
                return out;
            }
        }
    }

    fn response_ids(prefix: &str, start: usize, n: usize) -> Vec<String> {
        (start..start + n)
            .map(|i| format!("{prefix}{i:03}"))
            .collect()
    }

    /// Serialize a real v2 list envelope `{"data":[...],"meta":...}` so tests
    /// exercise the production deserialization path. `meta` is a raw JSON
    /// fragment: `""` omits the key entirely, `"\"meta\":null"` sends null.
    fn envelope_json(ids: &[String], meta: &str) -> String {
        let items: Vec<String> = ids
            .iter()
            .map(|id| {
                format!(
                    "{{\"id\":\"{id}\",\"surveyId\":\"svy\",\"createdAt\":\"2026-01-01T00:00:00.000Z\",\"updatedAt\":\"2026-01-01T00:00:00.000Z\",\"finished\":true,\"data\":{{}}}}"
                )
            })
            .collect();
        if meta.is_empty() {
            format!("{{\"data\":[{}]}}", items.join(","))
        } else {
            format!("{{\"data\":[{}],{}}}", items.join(","), meta)
        }
    }

    fn scripted_page(ids: &[String], meta: &str) -> Result<FormbricksResponseList, String> {
        serde_json::from_str(&envelope_json(ids, meta))
            .map_err(|e| format!("fixture parse failed: {e}"))
    }

    fn scripted_fetch(
        pages: Vec<Result<FormbricksResponseList, String>>,
        calls: &mut Vec<(u32, u32)>,
    ) -> impl FnMut(u32, u32) -> std::future::Ready<Result<FormbricksResponseList, String>> + '_
    {
        let mut next = 0usize;
        move |limit: u32, skip: u32| {
            calls.push((limit, skip));
            let page = pages[next].clone();
            next += 1;
            std::future::ready(page)
        }
    }

    #[test]
    fn missing_meta_paginates_by_actual_count_until_empty_page() {
        // Production regression: FormBricks omits meta.total, and the walk failed
        // with "omitted meta.total". Instead it must advance by the actual page
        // size (server caps pages at 50 despite limit=100) and stop on the first
        // empty page.
        let ids = |start: usize, n: usize| response_ids("r", start, n);
        let pages: Vec<Result<FormbricksResponseList, String>> = vec![
            scripted_page(&ids(0, 50), ""),               // no meta key at all
            scripted_page(&ids(50, 50), "\"meta\":null"), // meta present, total null
            scripted_page(&[], "\"meta\":null"),          // empty page = done
        ];
        let mut calls: Vec<(u32, u32)> = Vec::new();

        let all = block_on(paginate_all_responses(
            scripted_fetch(pages, &mut calls),
            10,
        ))
        .expect("missing meta.total must not abort the walk");

        assert_eq!(all.len(), 100);
        assert_eq!(calls, vec![(100, 0), (100, 50), (100, 100)]);
    }

    #[test]
    fn exact_multiple_with_known_total_skips_terminal_fetch() {
        // Total 200 is exactly two full pages: stop at skip=200 without a third
        // request that would only confirm exhaustion.
        let ids = |start: usize, n: usize| response_ids("r", start, n);
        let total = "\"meta\":{\"total\":200}";
        let pages: Vec<Result<FormbricksResponseList, String>> = vec![
            scripted_page(&ids(0, 100), total),
            scripted_page(&ids(100, 100), total),
        ];
        let mut calls: Vec<(u32, u32)> = Vec::new();

        let all = block_on(paginate_all_responses(
            scripted_fetch(pages, &mut calls),
            10,
        ))
        .unwrap();

        assert_eq!(all.len(), 200);
        assert_eq!(calls, vec![(100, 0), (100, 100)]);
    }

    #[test]
    fn exact_multiple_without_total_fetches_empty_page() {
        // Without a total, an exact multiple needs one extra empty page to prove
        // exhaustion.
        let ids = |start: usize, n: usize| response_ids("r", start, n);
        let pages: Vec<Result<FormbricksResponseList, String>> = vec![
            scripted_page(&ids(0, 100), "\"meta\":null"),
            scripted_page(&ids(100, 100), "\"meta\":null"),
            scripted_page(&[], "\"meta\":null"),
        ];
        let mut calls: Vec<(u32, u32)> = Vec::new();

        let all = block_on(paginate_all_responses(
            scripted_fetch(pages, &mut calls),
            10,
        ))
        .unwrap();

        assert_eq!(all.len(), 200);
        assert_eq!(calls, vec![(100, 0), (100, 100), (100, 200)]);
    }

    #[test]
    fn empty_first_page_returns_no_responses() {
        let pages: Vec<Result<FormbricksResponseList, String>> = vec![scripted_page(&[], "")];
        let mut calls: Vec<(u32, u32)> = Vec::new();

        let all = block_on(paginate_all_responses(
            scripted_fetch(pages, &mut calls),
            10,
        ))
        .unwrap();

        assert!(all.is_empty());
        assert_eq!(calls, vec![(100, 0)]);
    }

    #[test]
    fn known_total_with_short_final_page_stops_at_total() {
        // 250 = 100 + 100 + 50: stop once the collected count reaches the total,
        // even though the final page is short.
        let ids = |start: usize, n: usize| response_ids("r", start, n);
        let total = "\"meta\":{\"total\":250}";
        let pages: Vec<Result<FormbricksResponseList, String>> = vec![
            scripted_page(&ids(0, 100), total),
            scripted_page(&ids(100, 100), total),
            scripted_page(&ids(200, 50), total),
        ];
        let mut calls: Vec<(u32, u32)> = Vec::new();

        let all = block_on(paginate_all_responses(
            scripted_fetch(pages, &mut calls),
            10,
        ))
        .unwrap();

        assert_eq!(all.len(), 250);
        assert_eq!(calls, vec![(100, 0), (100, 100), (100, 200)]);
    }

    #[test]
    fn fetch_error_propagates() {
        let ids = |start: usize, n: usize| response_ids("r", start, n);
        let pages: Vec<Result<FormbricksResponseList, String>> = vec![
            scripted_page(&ids(0, 50), ""),
            Err("upstream 503".to_string()),
        ];
        let mut calls: Vec<(u32, u32)> = Vec::new();

        let err = block_on(paginate_all_responses(
            scripted_fetch(pages, &mut calls),
            10,
        ))
        .unwrap_err();

        assert!(err.contains("upstream 503"), "unexpected error: {err}");
        assert_eq!(calls, vec![(100, 0), (100, 50)]);
    }

    #[test]
    fn cap_error_when_total_exceeds_max_pages() {
        // 1000 responses at 100/page needs 10 pages; a cap of 5 must error
        // instead of returning a truncated set.
        let ids = |start: usize, n: usize| response_ids("r", start, n);
        let total = "\"meta\":{\"total\":1000}";
        let pages: Vec<Result<FormbricksResponseList, String>> = (0..5)
            .map(|p| scripted_page(&ids(p * 100, 100), total))
            .collect();
        let mut calls: Vec<(u32, u32)> = Vec::new();

        let err =
            block_on(paginate_all_responses(scripted_fetch(pages, &mut calls), 5)).unwrap_err();

        assert!(err.contains("safety cap of 5"), "unexpected error: {err}");
        assert_eq!(calls.len(), 5);
    }

    #[test]
    fn cap_error_without_total_and_zero_max_pages() {
        let ids = |start: usize, n: usize| response_ids("r", start, n);

        // Unknown total: a never-ending stream of non-empty pages must hit the cap.
        let endless: Vec<Result<FormbricksResponseList, String>> = (0..4)
            .map(|p| scripted_page(&ids(p * 50, 50), ""))
            .collect();
        let mut calls: Vec<(u32, u32)> = Vec::new();
        let err = block_on(paginate_all_responses(
            scripted_fetch(endless, &mut calls),
            3,
        ))
        .unwrap_err();
        assert!(err.contains("safety cap of 3"), "unexpected error: {err}");
        assert_eq!(calls.len(), 3);

        // Zero cap with a non-exhausted first page errors...
        let pages: Vec<Result<FormbricksResponseList, String>> =
            vec![scripted_page(&ids(0, 50), "\"meta\":{\"total\":1000}")];
        let mut calls: Vec<(u32, u32)> = Vec::new();
        assert!(block_on(paginate_all_responses(scripted_fetch(pages, &mut calls), 0)).is_err());

        // ...but a first page that already exhausts a known total succeeds.
        let pages: Vec<Result<FormbricksResponseList, String>> =
            vec![scripted_page(&ids(0, 50), "\"meta\":{\"total\":50}")];
        let mut calls: Vec<(u32, u32)> = Vec::new();
        let all = block_on(paginate_all_responses(scripted_fetch(pages, &mut calls), 0)).unwrap();
        assert_eq!(all.len(), 50);
    }

    #[test]
    fn repeated_page_aborts_instead_of_looping_forever() {
        // A server that ignores `skip` returns the same page forever; the guard
        // must abort on a repeated first response id.
        let ids = |start: usize, n: usize| response_ids("r", start, n);
        let pages: Vec<Result<FormbricksResponseList, String>> = vec![
            scripted_page(&ids(0, 50), ""),
            scripted_page(&ids(0, 50), ""),
        ];
        let mut calls: Vec<(u32, u32)> = Vec::new();

        let err = block_on(paginate_all_responses(
            scripted_fetch(pages, &mut calls),
            10,
        ))
        .unwrap_err();

        assert!(err.contains("not advancing"), "unexpected error: {err}");
        assert_eq!(calls, vec![(100, 0), (100, 50)]);
    }
}
