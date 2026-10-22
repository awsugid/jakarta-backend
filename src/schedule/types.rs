use serde::{Deserialize, Serialize};

use crate::links::types::deserialize_d1_bool;

/// D1 stores JSON string arrays (`room_ids_json`, `speaker_ids_json`); accept
/// the D1 string form, native arrays, null, and empty strings.
pub fn deserialize_id_list<'de, D>(d: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;
    match serde_json::Value::deserialize(d)? {
        serde_json::Value::Null => Ok(Vec::new()),
        serde_json::Value::String(s) if s.trim().is_empty() => Ok(Vec::new()),
        serde_json::Value::String(s) => serde_json::from_str(&s).map_err(D::Error::custom),
        other => serde_json::from_value(other).map_err(D::Error::custom),
    }
}

/// One entry of a schedule item's speakers array.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleSpeaker {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub company: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub social_url: Option<String>,
}

/// D1 stores speakers as the `speakers_json` TEXT column; the API exposes the
/// parsed array under `speakers` and never leaks the raw JSON string. Accepts
/// the D1 string form (via alias) and any array/null value.
fn deserialize_speakers<'de, D>(d: D) -> Result<Vec<ScheduleSpeaker>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;
    match serde_json::Value::deserialize(d)? {
        serde_json::Value::Null => Ok(Vec::new()),
        serde_json::Value::String(s) if s.trim().is_empty() => Ok(Vec::new()),
        serde_json::Value::String(s) => serde_json::from_str(&s).map_err(D::Error::custom),
        other => serde_json::from_value(other).map_err(D::Error::custom),
    }
}

/// Singular legacy columns for the first speaker (compat with old clients).
#[allow(clippy::type_complexity)]
pub fn first_speaker_legacy(
    speakers: &[ScheduleSpeaker],
) -> (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
) {
    match speakers.first() {
        Some(s) => (
            s.id.clone(),
            Some(s.name.clone()),
            s.role.clone(),
            s.company.clone(),
            s.avatar.clone(),
            s.social_url.clone(),
        ),
        None => (None, None, None, None, None, None),
    }
}

/// Array fallback when only legacy singular fields are provided (create path).
pub fn speakers_from_legacy(
    speaker_id: &Option<String>,
    speaker_name: &Option<String>,
    speaker_role: &Option<String>,
    speaker_company: &Option<String>,
    speaker_avatar: &Option<String>,
    speaker_social_url: &Option<String>,
) -> Vec<ScheduleSpeaker> {
    match speaker_name {
        Some(n) if !n.trim().is_empty() => vec![ScheduleSpeaker {
            id: speaker_id.clone(),
            name: n.clone(),
            role: speaker_role.clone(),
            company: speaker_company.clone(),
            avatar: speaker_avatar.clone(),
            social_url: speaker_social_url.clone(),
        }],
        _ => Vec::new(),
    }
}

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
    #[serde(
        default,
        alias = "speakers_json",
        deserialize_with = "deserialize_speakers"
    )]
    pub speakers: Vec<ScheduleSpeaker>,
    pub room: Option<String>,
    pub track: Option<String>,
    #[serde(
        default,
        alias = "room_ids_json",
        deserialize_with = "deserialize_id_list"
    )]
    pub room_ids: Vec<String>,
    #[serde(
        default,
        alias = "speaker_ids_json",
        deserialize_with = "deserialize_id_list"
    )]
    pub speaker_ids: Vec<String>,
    #[serde(default, alias = "all_rooms", deserialize_with = "deserialize_d1_bool")]
    pub all_rooms: bool,
    #[serde(
        default,
        alias = "languages_json",
        deserialize_with = "deserialize_id_list"
    )]
    pub languages: Vec<String>,
    #[serde(
        default,
        alias = "levels_json",
        deserialize_with = "deserialize_id_list"
    )]
    pub levels: Vec<String>,
    /// Resolved room names (admin listing and published snapshot only; not a column).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rooms: Option<Vec<String>>,
    #[serde(default, alias = "starts_at")]
    pub starts_at: Option<String>,
    #[serde(alias = "ends_at")]
    pub ends_at: Option<String>,
    #[serde(alias = "session_type")]
    pub session_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", alias = "format_id")]
    pub format_id: Option<String>,
    /// Resolved format name (admin listing and published snapshot only; not a column).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    #[serde(alias = "is_service_session", deserialize_with = "deserialize_d1_bool")]
    pub is_service_session: bool,
    #[serde(alias = "is_active", deserialize_with = "deserialize_d1_bool")]
    pub is_active: bool,
    #[serde(alias = "display_order")]
    pub display_order: i32,
    #[serde(default, alias = "revision")]
    pub revision: i32,
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
    /// Full speaker list. When present, singular columns mirror the first entry.
    pub speakers: Option<Vec<ScheduleSpeaker>>,
    /// Directory speaker ids; server resolves display details from `event_speakers`.
    /// Wins over `speakers` when both are present.
    pub speaker_ids: Option<Vec<String>>,
    pub room: Option<String>,
    pub track: Option<String>,
    pub room_ids: Option<Vec<String>>,
    pub all_rooms: Option<bool>,
    pub format_id: Option<String>,
    /// Spoken languages (Sessionize "Language" category values).
    pub languages: Option<Vec<String>>,
    /// Audience level (Sessionize level-category values).
    pub levels: Option<Vec<String>>,
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
    pub revision: i32,
    pub session_id: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub speaker_id: Option<String>,
    pub speaker_name: Option<String>,
    pub speaker_role: Option<String>,
    pub speaker_company: Option<String>,
    pub speaker_avatar: Option<String>,
    pub speaker_social_url: Option<String>,
    /// Full speaker list. `None` (omitted) keeps existing speakers, `Some([])` clears.
    pub speakers: Option<Vec<ScheduleSpeaker>>,
    /// Directory speaker ids; when present, wins over `speakers` and singular fields.
    pub speaker_ids: Option<Vec<String>>,
    pub room: Option<String>,
    pub track: Option<String>,
    pub room_ids: Option<Vec<String>>,
    pub all_rooms: Option<bool>,
    /// Format id; empty string clears the link (label falls back to sessionType).
    pub format_id: Option<String>,
    /// Spoken languages; `None` (omitted) keeps existing, `Some([])` clears.
    pub languages: Option<Vec<String>>,
    /// Audience level; `None` (omitted) keeps existing, `Some([])` clears.
    pub levels: Option<Vec<String>>,
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

/// Admin listing: schedule plus the workspace revision (review/publish token)
/// and the last publication timestamp, if any.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminScheduleResponse<'a> {
    pub event_slug: &'a str,
    pub schedule: &'a [EventScheduleItem],
    pub revision: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub published_at: Option<String>,
}

/// Row from the `event_rooms` table.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventRoom {
    pub id: String,
    #[serde(alias = "event_slug")]
    pub event_slug: String,
    pub name: String,
    #[serde(alias = "display_order")]
    pub display_order: i32,
    #[serde(alias = "archived", deserialize_with = "deserialize_d1_bool")]
    pub archived: bool,
    #[serde(default, alias = "revision")]
    pub revision: i32,
    #[serde(alias = "created_at")]
    pub created_at: String,
    #[serde(alias = "updated_at")]
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EventRoomsResponse<'a> {
    pub event_slug: &'a str,
    pub rooms: &'a [EventRoom],
}

/// POST /api/admin/events/:eventSlug/rooms body.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateRoomBody {
    pub name: String,
    pub display_order: Option<i32>,
}

/// PUT /api/admin/events/:eventSlug/rooms/:roomId body.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateRoomBody {
    pub revision: i32,
    pub name: Option<String>,
    pub display_order: Option<i32>,
    pub archived: Option<bool>,
}

/// Row from the `event_formats` table.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventFormat {
    pub id: String,
    #[serde(alias = "event_slug")]
    pub event_slug: String,
    pub name: String,
    #[serde(alias = "display_order")]
    pub display_order: i32,
    #[serde(alias = "archived", deserialize_with = "deserialize_d1_bool")]
    pub archived: bool,
    #[serde(default, alias = "revision")]
    pub revision: i32,
    #[serde(alias = "created_at")]
    pub created_at: String,
    #[serde(alias = "updated_at")]
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EventFormatsResponse<'a> {
    pub event_slug: &'a str,
    pub formats: &'a [EventFormat],
}

/// POST /api/admin/events/:eventSlug/formats body.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateFormatBody {
    pub name: String,
    pub display_order: Option<i32>,
}

/// PUT /api/admin/events/:eventSlug/formats/:formatId body.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateFormatBody {
    pub revision: i32,
    pub name: Option<String>,
    pub display_order: Option<i32>,
    pub archived: Option<bool>,
}

/// Stored published snapshot document.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicationSnapshotDoc {
    pub schedule: Vec<EventScheduleItem>,
}

/// Row from `event_schedule_publications` (fields the API needs).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicationRow {
    #[serde(alias = "snapshot_json")]
    pub snapshot_json: String,
    #[serde(alias = "published_at")]
    pub published_at: String,
}

/// POST /api/admin/events/:eventSlug/schedule/publish body.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishBody {
    pub revision: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishResponse {
    pub published_at: String,
    pub item_count: usize,
    pub revision: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal D1 row shape (snake_case columns) for deserialization tests.
    #[derive(Deserialize)]
    struct Row {
        #[serde(
            default,
            alias = "speakers_json",
            deserialize_with = "deserialize_speakers"
        )]
        speakers: Vec<ScheduleSpeaker>,
    }

    #[test]
    fn deserializes_d1_json_string_and_drops_nulls_on_output() {
        let row: Row = serde_json::from_str(
            r#"{"id":"s1","title":"T","is_service_session":false,"is_active":true,
                "display_order":0,"created_at":"","updated_at":"",
                "speakers_json":"[{\"id\":\"spk-1\",\"name\":\"Ada\",\"role\":null,\"company\":\"AWS\"}]"}"#,
        )
        .unwrap();
        assert_eq!(row.speakers.len(), 1);
        assert_eq!(row.speakers[0].id.as_deref(), Some("spk-1"));
        assert_eq!(row.speakers[0].role, None);
        // Serialized form exposes `speakers`, omits nulls, never leaks JSON string.
        let out = serde_json::to_value(&row.speakers).unwrap();
        assert_eq!(
            out,
            serde_json::json!([{ "id": "spk-1", "name": "Ada", "company": "AWS" }])
        );
    }

    #[test]
    fn deserializes_null_empty_and_missing_as_empty_array() {
        for raw in [
            r#"{"id":"s","title":"T","is_service_session":false,"is_active":true,"display_order":0,"created_at":"","updated_at":"","speakers_json":null}"#,
            r#"{"id":"s","title":"T","is_service_session":false,"is_active":true,"display_order":0,"created_at":"","updated_at":"","speakers_json":""}"#,
            r#"{"id":"s","title":"T","is_service_session":false,"is_active":true,"display_order":0,"created_at":"","updated_at":""}"#,
        ] {
            let row: Row = serde_json::from_str(raw).unwrap();
            assert!(row.speakers.is_empty(), "expected empty for {raw}");
        }
    }

    #[test]
    fn deserializes_native_array_form() {
        let row: Row = serde_json::from_str(
            r#"{"id":"s","title":"T","is_service_session":false,"is_active":true,
                "display_order":0,"created_at":"","updated_at":"",
                "speakers":[{"name":"Ada"},{"name":"Grace"}]}"#,
        )
        .unwrap();
        assert_eq!(row.speakers.len(), 2);
    }

    #[test]
    fn legacy_derivation_roundtrip() {
        let speakers = speakers_from_legacy(
            &Some("spk-1".into()),
            &Some("  Ada  ".into()),
            &Some("DevRel".into()),
            &None,
            &None,
            &None,
        );
        assert_eq!(speakers.len(), 1);
        let (id, name, role, ..) = first_speaker_legacy(&speakers);
        assert_eq!(id.as_deref(), Some("spk-1"));
        assert_eq!(name.as_deref(), Some("  Ada  "));
        assert_eq!(role.as_deref(), Some("DevRel"));
        // No legacy name -> no speakers -> singular fields all cleared.
        let empty = speakers_from_legacy(&Some("spk-1".into()), &None, &None, &None, &None, &None);
        assert!(empty.is_empty());
        assert_eq!(
            first_speaker_legacy(&empty),
            (None, None, None, None, None, None)
        );
    }
}
