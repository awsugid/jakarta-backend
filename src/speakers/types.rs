use serde::{Deserialize, Serialize};

use crate::links::types::deserialize_d1_bool;

/// Row from the `event_speakers` table.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventSpeaker {
    pub id: String,
    #[serde(alias = "event_slug")]
    pub event_slug: String,
    pub name: String,
    pub role: String,
    pub company: Option<String>,
    #[serde(alias = "photo_url")]
    pub photo_url: String,
    #[serde(alias = "talk_title")]
    pub talk_title: String,
    #[serde(alias = "talk_format")]
    pub talk_format: Option<String>,
    #[serde(alias = "topic_image_url")]
    pub topic_image_url: Option<String>,
    pub bio: Option<String>,
    #[serde(alias = "social_url")]
    pub social_url: Option<String>,
    #[serde(alias = "is_active", deserialize_with = "deserialize_d1_bool")]
    pub is_active: bool,
    #[serde(alias = "display_order")]
    pub display_order: i32,
    #[serde(alias = "created_at")]
    pub created_at: String,
    #[serde(alias = "updated_at")]
    pub updated_at: String,
}

/// POST /api/admin/events/:eventSlug/speakers body.
///
/// The directory is profile-only: talk metadata is owned by sessions/schedules.
/// `talk_title`/`talk_format` stay accepted for legacy clients but are optional
/// and no longer authoritative.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventSpeakerCreate {
    pub name: String,
    pub role: String,
    pub company: Option<String>,
    pub photo_url: String,
    pub talk_title: Option<String>,
    pub talk_format: Option<String>,
    pub topic_image_url: Option<String>,
    pub bio: Option<String>,
    pub social_url: Option<String>,
    pub is_active: Option<bool>,
}

/// PUT /api/admin/events/:eventSlug/speakers/:speakerId body.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventSpeakerUpdate {
    pub name: String,
    pub role: String,
    pub company: Option<String>,
    pub photo_url: String,
    // Accepted for legacy-client body compatibility; updates never write talk
    // metadata (owned by the schedule), so the fields are never read.
    #[allow(dead_code)]
    pub talk_title: Option<String>,
    #[allow(dead_code)]
    pub talk_format: Option<String>,
    pub topic_image_url: Option<String>,
    pub bio: Option<String>,
    pub social_url: Option<String>,
    pub is_active: bool,
}

/// Item in PUT /api/admin/events/:eventSlug/speakers/order body.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventSpeakerOrderItem {
    pub id: String,
    pub display_order: i32,
}

/// PUT /api/admin/events/:eventSlug/speakers/order body.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventSpeakerReorderBody {
    pub items: Vec<EventSpeakerOrderItem>,
}

/// Public / Admin speakers response.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EventSpeakersResponse<'a> {
    pub event_slug: &'a str,
    pub speakers: &'a [EventSpeaker],
}

/// POST /api/admin/events/:eventSlug/speakers/sync-sessionize body.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncSessionizeSpeakersRequest {
    pub speaker_ids: Option<Vec<String>>,
    pub overwrite: Option<bool>,
    pub fresh: Option<bool>,
}

/// Response from POST /api/admin/events/:eventSlug/speakers/sync-sessionize.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncSpeakersResponse {
    pub total_processed: usize,
    pub imported: usize,
    pub updated: usize,
    pub skipped: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_body_without_talk_fields_deserializes() {
        let body: EventSpeakerCreate =
            serde_json::from_str(r#"{"name":"Ada","role":"Speaker","photoUrl":"p.png"}"#)
                .expect("talk fields must not be required");
        assert_eq!(body.talk_title, None);
        assert_eq!(body.talk_format, None);
    }

    #[test]
    fn create_body_with_legacy_talk_fields_still_deserializes() {
        let body: EventSpeakerCreate = serde_json::from_str(
            r#"{"name":"Ada","role":"Speaker","photoUrl":"p.png","talkTitle":"T","talkFormat":"Keynote"}"#,
        )
        .expect("legacy talk fields stay accepted");
        assert_eq!(body.talk_title.as_deref(), Some("T"));
        assert_eq!(body.talk_format.as_deref(), Some("Keynote"));
    }

    #[test]
    fn update_body_without_talk_fields_deserializes() {
        let body: EventSpeakerUpdate = serde_json::from_str(
            r#"{"name":"Ada","role":"Speaker","photoUrl":"p.png","isActive":true}"#,
        )
        .expect("talk fields must not be required");
        assert_eq!(body.talk_title, None);
        assert_eq!(body.talk_format, None);
    }
}
