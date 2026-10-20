use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionizeRawSpeaker {
    pub id: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub full_name: String,
    pub bio: Option<String>,
    pub tag_line: Option<String>,
    pub profile_picture: Option<String>,
    #[serde(default)]
    pub sessions: Vec<SessionizeRawSpeakerSessionRef>,
    #[serde(default)]
    pub is_top_speaker: bool,
    #[serde(default)]
    pub links: Vec<SessionizeRawLink>,
    #[serde(default)]
    pub question_answers: Vec<SessionizeRawQuestionAnswer>,
    #[serde(default)]
    pub categories: Vec<SessionizeRawCategory>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionizeRawSpeakerSessionRef {
    pub id: serde_json::Value,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionizeRawLink {
    pub title: Option<String>,
    pub url: String,
    pub link_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionizeRawQuestionAnswer {
    pub id: i64,
    pub question: String,
    pub question_type: Option<String>,
    pub answer: Option<String>,
    pub sort: Option<i64>,
    pub answer_extra: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionizeRawCategory {
    pub id: i64,
    pub name: String,
    #[serde(default)]
    pub category_items: Vec<SessionizeRawCategoryItem>,
    pub sort: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionizeRawCategoryItem {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionizeRawSessionGroup {
    pub group_id: Option<serde_json::Value>,
    pub group_name: Option<String>,
    #[serde(default)]
    pub sessions: Vec<SessionizeRawSession>,
    #[serde(default)]
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionizeRawSession {
    pub id: serde_json::Value,
    pub title: String,
    pub description: Option<String>,
    pub starts_at: Option<String>,
    pub ends_at: Option<String>,
    #[serde(default)]
    pub is_service_session: bool,
    #[serde(default)]
    pub is_plenum_session: bool,
    #[serde(default)]
    pub speakers: Vec<SessionizeRawSessionSpeakerRef>,
    #[serde(default)]
    pub categories: Vec<SessionizeRawCategory>,
    pub room_id: Option<i64>,
    pub room: Option<String>,
    pub live_url: Option<String>,
    pub recording_url: Option<String>,
    pub status: Option<String>,
    pub is_informed: Option<bool>,
    pub is_confirmed: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionizeRawSessionSpeakerRef {
    pub id: String,
    pub name: String,
}

// ----------------------------------------------------------------------------
// Enriched domain representations (joined, cleaned, and normalized)
// ----------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnrichedSpeakerSession {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub starts_at: Option<String>,
    pub ends_at: Option<String>,
    pub room: Option<String>,
    pub room_id: Option<i64>,
    pub status: Option<String>,
    pub is_service_session: bool,
    pub is_confirmed: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnrichedSpeaker {
    pub id: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub full_name: String,
    pub bio: Option<String>,
    pub tag_line: Option<String>,
    pub profile_picture: Option<String>,
    pub company: Option<String>,
    pub job_title: Option<String>,
    pub position_name: Option<String>,
    pub linkedin_url: Option<String>,
    pub is_top_speaker: bool,
    pub links: Vec<SessionizeRawLink>,
    pub sessions: Vec<EnrichedSpeakerSession>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnrichedSessionSpeaker {
    pub id: String,
    pub name: String,
    pub tag_line: Option<String>,
    pub bio: Option<String>,
    pub profile_picture: Option<String>,
    pub company: Option<String>,
    pub job_title: Option<String>,
    pub linkedin_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnrichedSession {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub starts_at: Option<String>,
    pub ends_at: Option<String>,
    pub room: Option<String>,
    pub room_id: Option<i64>,
    pub status: Option<String>,
    pub is_service_session: bool,
    pub is_plenum_session: bool,
    pub is_confirmed: Option<bool>,
    pub speakers: Vec<EnrichedSessionSpeaker>,
    pub categories: Vec<SessionizeRawCategory>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionizeMetadata {
    pub rooms: Vec<String>,
    pub job_titles: Vec<String>,
    pub companies: Vec<String>,
    pub total_speakers: usize,
    pub total_sessions: usize,
    pub cached_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionizeDataPayload {
    pub speakers: Vec<EnrichedSpeaker>,
    pub sessions: Vec<EnrichedSession>,
    pub metadata: SessionizeMetadata,
}

#[derive(Debug, Clone, Default)]
pub struct SessionizeQueryFilter {
    pub q: Option<String>,
    pub room: Option<String>,
    pub job_title: Option<String>,
    pub company: Option<String>,
    pub fresh: bool,
}
