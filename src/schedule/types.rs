use serde::{Deserialize, Serialize};

use crate::links::types::deserialize_d1_bool;

/// Row from the `event_schedules` table.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventScheduleItem {
    pub id: String,
    #[serde(alias = "event_slug")]
    pub event_slug: String,
    #[serde(alias = "session_id")]
    pub session_id: Option<String>,
    pub title: String,
    pub description: Option<String>,
    #[serde(alias = "speaker_id")]
    pub speaker_id: Option<String>,
    #[serde(alias = "speaker_name")]
    pub speaker_name: Option<String>,
    #[serde(alias = "speaker_role")]
    pub speaker_role: Option<String>,
    #[serde(alias = "speaker_company")]
    pub speaker_company: Option<String>,
    #[serde(alias = "speaker_avatar")]
    pub speaker_avatar: Option<String>,
    #[serde(alias = "speaker_social_url")]
    pub speaker_social_url: Option<String>,
    pub room: Option<String>,
    pub track: Option<String>,
    #[serde(alias = "starts_at")]
    pub starts_at: Option<String>,
    #[serde(alias = "ends_at")]
    pub ends_at: Option<String>,
    #[serde(alias = "session_type")]
    pub session_type: Option<String>,
    #[serde(alias = "is_service_session", deserialize_with = "deserialize_d1_bool")]
    pub is_service_session: bool,
    #[serde(alias = "is_active", deserialize_with = "deserialize_d1_bool")]
    pub is_active: bool,
    #[serde(alias = "display_order")]
    pub display_order: i32,
    #[serde(alias = "created_at")]
    pub created_at: String,
    #[serde(alias = "updated_at")]
    pub updated_at: String,
}

/// Request body for creating a schedule entry manually.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateScheduleItem {
    pub session_id: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub speaker_id: Option<String>,
    pub speaker_name: Option<String>,
    pub speaker_role: Option<String>,
    pub speaker_company: Option<String>,
    pub speaker_avatar: Option<String>,
    pub speaker_social_url: Option<String>,
    pub room: Option<String>,
    pub track: Option<String>,
    pub starts_at: Option<String>,
    pub ends_at: Option<String>,
    pub session_type: Option<String>,
    pub is_service_session: Option<bool>,
    pub is_active: Option<bool>,
    pub display_order: Option<i32>,
}

/// Request body for updating a schedule entry.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateScheduleItem {
    pub session_id: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub speaker_id: Option<String>,
    pub speaker_name: Option<String>,
    pub speaker_role: Option<String>,
    pub speaker_company: Option<String>,
    pub speaker_avatar: Option<String>,
    pub speaker_social_url: Option<String>,
    pub room: Option<String>,
    pub track: Option<String>,
    pub starts_at: Option<String>,
    pub ends_at: Option<String>,
    pub session_type: Option<String>,
    pub is_service_session: Option<bool>,
    pub is_active: Option<bool>,
    pub display_order: Option<i32>,
}

/// Batch reorder request body item.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleOrderItem {
    pub id: String,
    pub display_order: i32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReorderScheduleBody {
    pub items: Vec<ScheduleOrderItem>,
}

/// Request body for Sessionize sync into schedule.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncSessionizeRequest {
    /// Optional specific session IDs to import. If omitted/empty, syncs all sessions.
    pub session_ids: Option<Vec<String>>,
    /// If true, overrides existing matching sessions; otherwise skips existing.
    pub overwrite: Option<bool>,
    /// Force fetch fresh data from Sessionize.
    pub fresh: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncSessionizeResponse {
    pub imported: usize,
    pub updated: usize,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EventScheduleResponse<'a> {
    pub event_slug: &'a str,
    pub schedule: &'a [EventScheduleItem],
}
