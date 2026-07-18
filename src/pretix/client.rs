use crate::config::AppConfig;
use crate::pretix::orders::{PretixEventSummary, UserPretixOrderSummary};
use worker::{Fetch, Headers, Method, Request, RequestInit};

pub struct PretixClient {
    base_url: String,
    api_token: String,
}

impl PretixClient {
    pub fn new(config: &AppConfig) -> Self {
        Self {
            base_url: config.pretix_base_url.trim_end_matches('/').to_string(),
            api_token: config.pretix_api_token.clone(),
        }
    }

    /// GET .../checkinlists/{list}/positions/?page_size=1[&has_checkin=true][&subevent={}][&item__in=1,2,3]
    /// Returns the top-level `count` only.
    pub async fn get_position_count(
        &self,
        organizer: &str,
        event: &str,
        list: &str,
        has_checkin: bool,
        subevent: Option<&str>,
        item_ids: Option<&[u64]>,
    ) -> Result<u64, String> {
        let mut url = format!(
            "{}/api/v1/organizers/{}/events/{}/checkinlists/{}/positions/?page_size=1",
            self.base_url, organizer, event, list
        );
        if has_checkin {
            url.push_str("&has_checkin=true");
        }
        if let Some(sub) = subevent {
            url.push_str(&format!("&subevent={}", sub));
        }
        if let Some(ids) = item_ids {
            let joined = ids
                .iter()
                .map(|i| i.to_string())
                .collect::<Vec<_>>()
                .join(",");
            url.push_str(&format!("&item__in={}", joined));
        }

        let headers = Headers::new();
        headers
            .set("Authorization", &format!("Token {}", self.api_token))
            .map_err(|e| format!("header error: {e}"))?;
        headers
            .set("Accept", "application/json")
            .map_err(|e| format!("header error: {e}"))?;

        let req = Request::new_with_init(
            &url,
            &RequestInit {
                headers,
                method: Method::Get,
                ..Default::default()
            },
        )
        .map_err(|e| format!("request build error: {e}"))?;

        let mut resp = Fetch::Request(req)
            .send()
            .await
            .map_err(|e| format!("Pretix request failed: {e}"))?;

        let status = resp.status_code();
        if status != 200 {
            let body = resp.text().await.unwrap_or_default();
            return Err(format!(
                "Pretix API returned {status}: {}",
                truncate(&body, 256)
            ));
        }

        let body = resp.text().await.map_err(|e| format!("read error: {e}"))?;

        #[derive(serde::Deserialize)]
        struct CountOnly {
            count: u64,
        }
        let parsed: CountOnly =
            serde_json::from_str(&body).map_err(|e| format!("parse error: {e}"))?;
        Ok(parsed.count)
    }

    /// Auto-discover the check-in list ID with the most registered positions.
    ///
    /// Pretix's `position_count` field on a checkinlist object can be misleading
    /// (includes/excludes based on `include_pending`, product filters, etc.). To get
    /// an apples-to-apples comparison we call the positions endpoint for each list
    /// with `page_size=1` and read the top-level `count`, then pick the max.
    pub async fn get_first_checkin_list_id(
        &self,
        organizer: &str,
        event: &str,
    ) -> Result<String, String> {
        let ids = self.list_checkinlist_ids(organizer, event).await?;
        if ids.is_empty() {
            return Err("No check-in lists configured for this event".to_string());
        }

        // For each list, fetch the actual position count and pick the highest.
        let mut best: Option<(u64, u64)> = None; // (count, id)
        for id in &ids {
            let count = self
                .get_position_count(organizer, event, &id.to_string(), false, None, None)
                .await
                .unwrap_or(0);
            match best {
                Some((bc, _)) if bc >= count => {}
                _ => best = Some((count, *id)),
            }
        }
        let (_count, id) =
            best.ok_or_else(|| "No check-in lists configured for this event".to_string())?;
        Ok(id.to_string())
    }

    /// Fetch all check-in list IDs for an event.
    async fn list_checkinlist_ids(&self, organizer: &str, event: &str) -> Result<Vec<u64>, String> {
        let url = format!(
            "{}/api/v1/organizers/{}/events/{}/checkinlists/?page_size=100",
            self.base_url, organizer, event
        );

        let headers = Headers::new();
        headers
            .set("Authorization", &format!("Token {}", self.api_token))
            .map_err(|e| format!("header error: {e}"))?;
        headers
            .set("Accept", "application/json")
            .map_err(|e| format!("header error: {e}"))?;

        let req = Request::new_with_init(
            &url,
            &RequestInit {
                headers,
                method: Method::Get,
                ..Default::default()
            },
        )
        .map_err(|e| format!("request build error: {e}"))?;

        let mut resp = Fetch::Request(req)
            .send()
            .await
            .map_err(|e| format!("Pretix request failed: {e}"))?;

        let status = resp.status_code();
        if status != 200 {
            let body = resp.text().await.unwrap_or_default();
            return Err(format!(
                "Pretix checkinlists lookup returned {status}: {}",
                truncate(&body, 256)
            ));
        }

        let body = resp.text().await.map_err(|e| format!("read error: {e}"))?;

        #[derive(serde::Deserialize)]
        struct ListResponse {
            results: Vec<ListEntry>,
        }
        #[derive(serde::Deserialize)]
        struct ListEntry {
            id: u64,
        }
        let parsed: ListResponse =
            serde_json::from_str(&body).map_err(|e| format!("parse error: {e}"))?;
        Ok(parsed.results.into_iter().map(|e| e.id).collect())
    }

    /// List orders for a purchaser email.
    ///
    /// Calls `GET /api/v1/organizers/{organizer}/orders/?email=...`.
    /// Returns the top-level `count` plus sanitized summaries.
    pub async fn list_orders_for_email(
        &self,
        organizer: &str,
        event: Option<&str>,
        email: &str,
        limit: u32,
        offset: u32,
        status: Option<&str>,
    ) -> Result<PretixOrderListPage, String> {
        let mut url = format!(
            "{}/api/v1/organizers/{}/orders/?email={}&limit={}&offset={}",
            self.base_url, organizer, email, limit, offset
        );
        if let Some(ev) = event {
            url.push_str(&format!("&event={}", ev));
        }
        if let Some(s) = status {
            if !s.eq_ignore_ascii_case("all") {
                url.push_str(&format!("&status={}", s));
            }
        }

        let headers = Headers::new();
        headers
            .set("Authorization", &format!("Token {}", self.api_token))
            .map_err(|e| format!("header error: {e}"))?;
        headers
            .set("Accept", "application/json")
            .map_err(|e| format!("header error: {e}"))?;

        let req = Request::new_with_init(
            &url,
            &RequestInit {
                headers,
                method: Method::Get,
                ..Default::default()
            },
        )
        .map_err(|e| format!("request build error: {e}"))?;

        let mut resp = Fetch::Request(req)
            .send()
            .await
            .map_err(|e| format!("Pretix request failed: {e}"))?;

        let status_code = resp.status_code();
        if status_code != 200 {
            let body = resp.text().await.unwrap_or_default();
            return Err(format!(
                "Pretix API returned {status_code}: {}",
                truncate(&body, 256)
            ));
        }

        let body = resp.text().await.map_err(|e| format!("read error: {e}"))?;
        let parsed: RawOrderList =
            serde_json::from_str(&body).map_err(|e| format!("parse error: {e}"))?;

        let orders = parsed.results.into_iter().map(map_order_summary).collect();

        Ok(PretixOrderListPage {
            count: parsed.count,
            orders,
        })
    }

    /// List event slugs for the organizer whose `date_from` falls within the given year (UTC).
    ///
    /// Calls `GET /api/v1/organizers/{organizer}/events/?date_from_after={YYYY}-01-01T00:00:00Z&date_from_before={YYYY}-12-31T23:59:59Z&page_size=100`.
    /// Only fetches the first page; logs a warning if `count > 100` (rare for a user group).
    pub async fn list_events_for_year(
        &self,
        organizer: &str,
        year: u32,
    ) -> Result<Vec<PretixEventSummary>, String> {
        let url = format!(
            "{}/api/v1/organizers/{}/events/?date_from_after={}-01-01T00:00:00Z&date_from_before={}-12-31T23:59:59Z&page_size=100",
            self.base_url, organizer, year, year
        );

        let headers = Headers::new();
        headers
            .set("Authorization", &format!("Token {}", self.api_token))
            .map_err(|e| format!("header error: {e}"))?;
        headers
            .set("Accept", "application/json")
            .map_err(|e| format!("header error: {e}"))?;

        let req = Request::new_with_init(
            &url,
            &RequestInit {
                headers,
                method: Method::Get,
                ..Default::default()
            },
        )
        .map_err(|e| format!("request build error: {e}"))?;

        let mut resp = Fetch::Request(req)
            .send()
            .await
            .map_err(|e| format!("Pretix request failed: {e}"))?;

        let status = resp.status_code();
        if status != 200 {
            let body = resp.text().await.unwrap_or_default();
            return Err(format!(
                "Pretix events lookup returned {status}: {}",
                truncate(&body, 256)
            ));
        }

        let body = resp.text().await.map_err(|e| format!("read error: {e}"))?;
        let parsed: EventsListResponse =
            serde_json::from_str(&body).map_err(|e| format!("parse error: {e}"))?;

        if parsed.count > parsed.results.len() as u64 {
            worker::console_log!(
                "list_events_for_year: count {} exceeds fetched page ({}); truncating",
                parsed.count,
                parsed.results.len()
            );
        }

        Ok(parsed
            .results
            .into_iter()
            .map(|e| PretixEventSummary {
                slug: e.slug,
                name: extract_event_name(&e.name),
            })
            .collect())
    }

    /// Fetch ALL order positions for an event, paginating through all pages.
    /// Calls GET /api/v1/organizers/{organizer}/events/{event}/orderpositions/?page_size=100
    /// Returns raw JSON values (each position is a serde_json::Value) so callers
    /// can extract the fields they need without a rigid type.
    ///
    /// Safety cap at 20 pages (2000 positions) to bound runtime in Workers.
    pub async fn get_all_order_positions(
        &self,
        organizer: &str,
        event: &str,
    ) -> Result<Vec<serde_json::Value>, String> {
        let mut url: Option<String> = Some(format!(
            "{}/api/v1/organizers/{}/events/{}/orderpositions/?page_size=100",
            self.base_url, organizer, event
        ));
        let mut all: Vec<serde_json::Value> = Vec::new();
        let mut pages = 0u32;
        while let Some(page_url) = url.take() {
            pages += 1;
            if pages > 20 {
                worker::console_log!(
                    "get_all_order_positions: {} hit safety cap at 20 pages",
                    event
                );
                break;
            }

            let headers = Headers::new();
            headers
                .set("Authorization", &format!("Token {}", self.api_token))
                .map_err(|e| format!("header error: {e}"))?;
            headers
                .set("Accept", "application/json")
                .map_err(|e| format!("header error: {e}"))?;

            let req = Request::new_with_init(
                &page_url,
                &RequestInit {
                    headers,
                    method: Method::Get,
                    ..Default::default()
                },
            )
            .map_err(|e| format!("request build error: {e}"))?;

            let mut resp = Fetch::Request(req)
                .send()
                .await
                .map_err(|e| format!("Pretix request failed: {e}"))?;

            let status = resp.status_code();
            if status != 200 {
                let body = resp.text().await.unwrap_or_default();
                return Err(format!(
                    "Pretix orderpositions lookup returned {status}: {}",
                    truncate(&body, 256)
                ));
            }

            let body = resp.text().await.map_err(|e| format!("read error: {e}"))?;
            let parsed: OrderPositionsPage =
                serde_json::from_str(&body).map_err(|e| format!("parse error: {e}"))?;

            all.extend(parsed.results);
            url = parsed.next;
        }
        Ok(all)
    }
}

#[derive(serde::Deserialize)]
struct OrderPositionsPage {
    #[serde(default)]
    next: Option<String>,
    #[serde(default)]
    results: Vec<serde_json::Value>,
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max {
        s
    } else {
        &s[..max]
    }
}

/// Paginated Pretix order result, already sanitized for return to frontend.
pub struct PretixOrderListPage {
    pub count: u64,
    pub orders: Vec<UserPretixOrderSummary>,
}

#[derive(serde::Deserialize)]
struct RawOrderList {
    count: u64,
    results: Vec<RawOrder>,
}

#[derive(serde::Deserialize)]
struct RawOrder {
    code: String,
    event: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    datetime: Option<String>,
    #[serde(default)]
    order_datetime: Option<String>,
    #[serde(default)]
    total: serde_json::Value,
    #[serde(default)]
    currency: Option<String>,
    #[serde(default)]
    positions: Vec<serde_json::Value>,
}

fn map_order_summary(o: RawOrder) -> UserPretixOrderSummary {
    let order_datetime = o.datetime.or(o.order_datetime);
    let total_str = match &o.total {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Number(n) => Some(n.to_string()),
        _ => None,
    };
    UserPretixOrderSummary {
        order_code: o.code,
        event_slug: o.event,
        event_name: String::new(),
        event_date: None,
        order_datetime,
        status: o.status,
        attendee_count: o.positions.len() as u64,
        checked_in_count: None,
        total: total_str,
        currency: o.currency,
        pretix_customer_portal_url: None,
    }
}

#[derive(serde::Deserialize)]
struct EventsListResponse {
    #[serde(default)]
    count: u64,
    #[serde(default)]
    results: Vec<EventsListEntry>,
}

#[derive(serde::Deserialize)]
struct EventsListEntry {
    slug: String,
    #[serde(default)]
    name: serde_json::Value,
}

/// Pretix `name` may be a string (legacy) or `{"default": "...", "en": "..."}` object.
/// Prefer `default`, then `en`, then empty string.
fn extract_event_name(name: &serde_json::Value) -> String {
    if let Some(s) = name.as_str() {
        return s.to_string();
    }
    if let Some(obj) = name.as_object() {
        if let Some(v) = obj.get("default").and_then(|v| v.as_str()) {
            return v.to_string();
        }
        if let Some(v) = obj.get("en").and_then(|v| v.as_str()) {
            return v.to_string();
        }
    }
    String::new()
}
