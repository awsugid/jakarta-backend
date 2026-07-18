use serde::Serialize;

/// Minimal event summary returned by `PretixClient::list_events_for_year`.
/// `name` pulled from `name.default` (fallback `name.en`, then empty).
#[derive(Debug, Clone, Serialize)]
pub struct PretixEventSummary {
    pub slug: String,
    pub name: String,
}

/// Sanitized summary of a user's Pretix order. No secrets, web secrets, or admin URLs.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserPretixOrderSummary {
    pub order_code: String,
    pub event_slug: String,
    pub event_name: String,
    pub event_date: Option<String>,
    pub order_datetime: Option<String>,
    pub status: String,
    pub attendee_count: u64,
    pub checked_in_count: Option<u64>,
    pub total: Option<String>,
    pub currency: Option<String>,
    pub pretix_customer_portal_url: Option<String>,
}

/// Response envelope for `GET /api/pretix/me/orders`.
#[derive(Debug, Clone, Serialize)]
pub struct UserPretixOrdersResponse {
    pub orders: Vec<UserPretixOrderSummary>,
    pub total: Option<u64>,
    pub limit: u32,
    pub offset: u32,
}
