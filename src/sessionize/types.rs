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

/// Category names (case-insensitive) that carry session languages.
pub const LANGUAGE_CATEGORY_NAMES: [&str; 2] = ["language", "languages"];
/// Category names (case-insensitive) that carry the audience level.
pub const LEVEL_CATEGORY_NAMES: [&str; 4] = [
    "level",
    "session level",
    "experience level",
    "audience level",
];

/// Distinct, trimmed, order-preserved item names of categories whose name
/// matches `names` case-insensitively. Session categories only — speaker
/// categories (e.g. Job Title) are a different field and never mix in.
fn category_item_values(categories: &[SessionizeRawCategory], names: &[&str]) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for cat in categories {
        if !names.contains(&cat.name.trim().to_lowercase().as_str()) {
            continue;
        }
        for item in &cat.category_items {
            let value = item.name.trim();
            if !value.is_empty() && seen.insert(value.to_string()) {
                out.push(value.to_string());
            }
        }
    }
    out
}

/// Languages spoken in a session (Sessionize "Language" category).
pub fn extract_languages(categories: &[SessionizeRawCategory]) -> Vec<String> {
    category_item_values(categories, &LANGUAGE_CATEGORY_NAMES)
}

/// Audience level of a session (Sessionize level categories).
pub fn extract_levels(categories: &[SessionizeRawCategory]) -> Vec<String> {
    category_item_values(categories, &LEVEL_CATEGORY_NAMES)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cat(name: &str, items: &[&str]) -> SessionizeRawCategory {
        SessionizeRawCategory {
            id: 1,
            name: name.into(),
            category_items: items
                .iter()
                .map(|n| SessionizeRawCategoryItem {
                    id: 1,
                    name: (*n).into(),
                })
                .collect(),
            sort: None,
        }
    }

    #[test]
    fn extracts_bilingual_language_and_ignores_other_categories() {
        let cats = vec![
            cat("Language", &["English", "Bahasa Indonesia"]),
            cat("Job Title", &["CEO"]),
            cat("Session Format", &["Workshop"]),
        ];
        assert_eq!(
            extract_languages(&cats),
            vec!["English".to_string(), "Bahasa Indonesia".to_string()]
        );
        assert!(extract_levels(&cats).is_empty(), "no level category");
    }

    #[test]
    fn level_name_aliases_match_case_insensitively_without_job_title_mixup() {
        for name in [
            "Level",
            "SESSION Level",
            "Experience Level",
            "Audience Level ",
        ] {
            let cats = vec![cat(name, &["Advanced"]), cat("Job Title", &["Developer"])];
            assert_eq!(
                extract_levels(&cats),
                vec!["Advanced".to_string()],
                "{name}"
            );
            assert!(extract_languages(&cats).is_empty(), "{name}");
        }
    }

    #[test]
    fn dedupes_and_drops_blank_values() {
        let cats = vec![
            cat("languages", &["English", " English "]),
            cat("Language", &["Bahasa Indonesia", "  "]),
        ];
        assert_eq!(
            extract_languages(&cats),
            vec!["English".to_string(), "Bahasa Indonesia".to_string()]
        );
        assert!(extract_levels(&cats).is_empty());
    }
}

#[derive(Debug, Clone, Default)]
pub struct SessionizeQueryFilter {
    pub q: Option<String>,
    pub room: Option<String>,
    pub job_title: Option<String>,
    pub company: Option<String>,
    pub fresh: bool,
}
