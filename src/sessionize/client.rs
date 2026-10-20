use std::collections::{HashMap, HashSet};
use worker::{Cache, Fetch, Method, Request, RequestInit, Response};

use crate::sessionize::types::{
    EnrichedSession, EnrichedSessionSpeaker, EnrichedSpeaker, EnrichedSpeakerSession,
    SessionizeDataPayload, SessionizeMetadata, SessionizeQueryFilter, SessionizeRawSession,
    SessionizeRawSessionGroup, SessionizeRawSpeaker,
};

pub const DEFAULT_SPEAKERS_URL: &str = "https://sessionize.com/api/v2/egep9rxt/view/Speakers";
pub const DEFAULT_SESSIONS_URL: &str = "https://sessionize.com/api/v2/egep9rxt/view/Sessions";
const CACHE_KEY: &str = "https://jakarta-backend.local/cache/sessionize-data-v2";
const CACHE_TTL_SECONDS: u32 = 900; // 15 minutes edge cache

pub struct SessionizeClient {
    speakers_url: String,
    sessions_url: String,
}

impl Default for SessionizeClient {
    fn default() -> Self {
        Self::new(DEFAULT_SPEAKERS_URL, DEFAULT_SESSIONS_URL)
    }
}

impl SessionizeClient {
    pub fn new(speakers_url: &str, sessions_url: &str) -> Self {
        Self {
            speakers_url: speakers_url.to_string(),
            sessions_url: sessions_url.to_string(),
        }
    }

    /// Fetch data from Sessionize or return from Cloudflare edge cache.
    pub async fn get_data(&self, fresh: bool) -> Result<SessionizeDataPayload, String> {
        let cache = Cache::default();

        if !fresh {
            if let Ok(Some(mut cached_resp)) = cache.get(CACHE_KEY, true).await {
                if let Ok(body) = cached_resp.text().await {
                    if let Ok(payload) = serde_json::from_str::<SessionizeDataPayload>(&body) {
                        return Ok(payload);
                    }
                }
            }
        }

        // Fetch both endpoints
        let (raw_speakers, raw_session_groups) = self.fetch_upstream().await?;

        // Process and enrich data
        let payload = self.build_enriched_payload(raw_speakers, raw_session_groups);

        // Put in edge cache
        if let Ok(json_str) = serde_json::to_string(&payload) {
            if let Ok(mut cache_resp) = Response::ok(&json_str) {
                let headers = cache_resp.headers_mut();
                let _ = headers.set("Content-Type", "application/json");
                let _ = headers.set(
                    "Cache-Control",
                    &format!("public, max-age={CACHE_TTL_SECONDS}"),
                );
                let _ = cache.put(CACHE_KEY, cache_resp).await;
            }
        }

        Ok(payload)
    }

    async fn fetch_upstream(
        &self,
    ) -> Result<(Vec<SessionizeRawSpeaker>, Vec<SessionizeRawSessionGroup>), String> {
        let (speakers_res, sessions_res) = (
            Self::http_get_json::<Vec<SessionizeRawSpeaker>>(&self.speakers_url).await,
            Self::http_get_json::<Vec<SessionizeRawSessionGroup>>(&self.sessions_url).await,
        );

        let speakers = speakers_res.map_err(|e| format!("Failed fetching speakers: {e}"))?;
        let session_groups = sessions_res.map_err(|e| format!("Failed fetching sessions: {e}"))?;

        Ok((speakers, session_groups))
    }

    async fn http_get_json<T: serde::de::DeserializeOwned>(url: &str) -> Result<T, String> {
        let req = Request::new_with_init(
            url,
            &RequestInit {
                method: Method::Get,
                ..Default::default()
            },
        )
        .map_err(|e| format!("failed to build request: {e}"))?;

        let mut resp = Fetch::Request(req)
            .send()
            .await
            .map_err(|e| format!("request to Sessionize failed: {e}"))?;

        let status = resp.status_code();
        if status != 200 {
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("Sessionize API returned HTTP {status}: {body}"));
        }

        let body = resp
            .text()
            .await
            .map_err(|e| format!("failed to read response text: {e}"))?;

        serde_json::from_str::<T>(&body)
            .map_err(|e| format!("failed to parse JSON from {url}: {e}"))
    }

    fn build_enriched_payload(
        &self,
        raw_speakers: Vec<SessionizeRawSpeaker>,
        raw_session_groups: Vec<SessionizeRawSessionGroup>,
    ) -> SessionizeDataPayload {
        // Flatten sessions from all groups
        let mut raw_sessions: Vec<SessionizeRawSession> = Vec::new();
        for group in raw_session_groups {
            for session in group.sessions {
                raw_sessions.push(session);
            }
        }

        // Map session_id (string representation) -> raw session
        let mut session_map: HashMap<String, SessionizeRawSession> = HashMap::new();
        for session in &raw_sessions {
            let sid = match &session.id {
                serde_json::Value::Number(n) => n.to_string(),
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string().trim_matches('"').to_string(),
            };
            session_map.insert(sid, session.clone());
        }

        // Parse speaker details & map speaker_id -> speaker
        let mut speaker_map: HashMap<String, SessionizeRawSpeaker> = HashMap::new();
        for speaker in &raw_speakers {
            speaker_map.insert(speaker.id.clone(), speaker.clone());
        }

        let mut rooms_set: HashSet<String> = HashSet::new();
        let mut job_titles_set: HashSet<String> = HashSet::new();
        let mut companies_set: HashSet<String> = HashSet::new();

        // Build enriched speakers
        let mut enriched_speakers: Vec<EnrichedSpeaker> = Vec::with_capacity(raw_speakers.len());
        for sp in raw_speakers {
            let mut company: Option<String> = None;
            let mut linkedin_url: Option<String> = None;
            let mut position_name: Option<String> = None;
            let mut job_title: Option<String> = None;

            for qa in &sp.question_answers {
                let q_lower = qa.question.to_lowercase();
                if q_lower.contains("company") || q_lower.contains("organization") {
                    if let Some(ans) = &qa.answer {
                        let trimmed = ans.trim();
                        if !trimmed.is_empty() {
                            company = Some(trimmed.to_string());
                            companies_set.insert(trimmed.to_string());
                        }
                    }
                } else if q_lower.contains("linkedin") {
                    if let Some(ans) = &qa.answer {
                        let trimmed = ans.trim();
                        if !trimmed.is_empty() {
                            linkedin_url = Some(trimmed.to_string());
                        }
                    }
                } else if q_lower.contains("position") {
                    if let Some(ans) = &qa.answer {
                        let trimmed = ans.trim();
                        if !trimmed.is_empty() {
                            position_name = Some(trimmed.to_string());
                        }
                    }
                }
            }

            for cat in &sp.categories {
                let cat_lower = cat.name.to_lowercase();
                if cat_lower.contains("job") || cat_lower.contains("title") {
                    for item in &cat.category_items {
                        let trimmed = item.name.trim();
                        if !trimmed.is_empty() {
                            job_title = Some(trimmed.to_string());
                            job_titles_set.insert(trimmed.to_string());
                        }
                    }
                }
            }

            // Fallback job title from position_name or tag_line if empty
            if job_title.is_none() {
                if let Some(pos) = &position_name {
                    job_title = Some(pos.clone());
                    job_titles_set.insert(pos.clone());
                } else if let Some(tag) = &sp.tag_line {
                    // Extract title before '@' or '|'
                    let clean = tag.split('@').next().unwrap_or(tag).trim();
                    if !clean.is_empty() {
                        job_title = Some(clean.to_string());
                        job_titles_set.insert(clean.to_string());
                    }
                }
            }

            // Fallback company from tag_line if empty
            if company.is_none() {
                if let Some(tag) = &sp.tag_line {
                    if let Some((_, comp)) = tag.split_once('@') {
                        let trimmed = comp.trim();
                        if !trimmed.is_empty() {
                            company = Some(trimmed.to_string());
                            companies_set.insert(trimmed.to_string());
                        }
                    }
                }
            }

            // Link enriched sessions
            let mut enriched_sessions: Vec<EnrichedSpeakerSession> = Vec::new();
            for s_ref in &sp.sessions {
                let sid = match &s_ref.id {
                    serde_json::Value::Number(n) => n.to_string(),
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string().trim_matches('"').to_string(),
                };

                if let Some(full_sess) = session_map.get(&sid) {
                    if let Some(rm) = &full_sess.room {
                        if !rm.trim().is_empty() {
                            rooms_set.insert(rm.trim().to_string());
                        }
                    }

                    enriched_sessions.push(EnrichedSpeakerSession {
                        id: sid,
                        title: full_sess.title.clone(),
                        description: full_sess.description.clone(),
                        starts_at: full_sess.starts_at.clone(),
                        ends_at: full_sess.ends_at.clone(),
                        room: full_sess.room.clone(),
                        room_id: full_sess.room_id,
                        status: full_sess.status.clone(),
                        is_service_session: full_sess.is_service_session,
                        is_confirmed: full_sess.is_confirmed,
                    });
                } else {
                    enriched_sessions.push(EnrichedSpeakerSession {
                        id: sid,
                        title: s_ref.name.clone(),
                        description: None,
                        starts_at: None,
                        ends_at: None,
                        room: None,
                        room_id: None,
                        status: None,
                        is_service_session: false,
                        is_confirmed: None,
                    });
                }
            }

            enriched_speakers.push(EnrichedSpeaker {
                id: sp.id,
                first_name: sp.first_name,
                last_name: sp.last_name,
                full_name: sp.full_name,
                bio: sp.bio,
                tag_line: sp.tag_line,
                profile_picture: sp.profile_picture,
                company,
                job_title,
                position_name,
                linkedin_url,
                is_top_speaker: sp.is_top_speaker,
                links: sp.links,
                sessions: enriched_sessions,
            });
        }

        // Build enriched sessions
        let mut enriched_sessions: Vec<EnrichedSession> = Vec::with_capacity(raw_sessions.len());
        for sess in raw_sessions {
            let sid = match &sess.id {
                serde_json::Value::Number(n) => n.to_string(),
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string().trim_matches('"').to_string(),
            };

            if let Some(rm) = &sess.room {
                if !rm.trim().is_empty() {
                    rooms_set.insert(rm.trim().to_string());
                }
            }

            let mut sess_speakers: Vec<EnrichedSessionSpeaker> = Vec::new();
            for sp_ref in &sess.speakers {
                if let Some(sp_full) = enriched_speakers.iter().find(|s| s.id == sp_ref.id) {
                    sess_speakers.push(EnrichedSessionSpeaker {
                        id: sp_full.id.clone(),
                        name: sp_full.full_name.clone(),
                        tag_line: sp_full.tag_line.clone(),
                        bio: sp_full.bio.clone(),
                        profile_picture: sp_full.profile_picture.clone(),
                        company: sp_full.company.clone(),
                        job_title: sp_full.job_title.clone(),
                        linkedin_url: sp_full.linkedin_url.clone(),
                    });
                } else {
                    sess_speakers.push(EnrichedSessionSpeaker {
                        id: sp_ref.id.clone(),
                        name: sp_ref.name.clone(),
                        tag_line: None,
                        bio: None,
                        profile_picture: None,
                        company: None,
                        job_title: None,
                        linkedin_url: None,
                    });
                }
            }

            enriched_sessions.push(EnrichedSession {
                id: sid,
                title: sess.title,
                description: sess.description,
                starts_at: sess.starts_at,
                ends_at: sess.ends_at,
                room: sess.room,
                room_id: sess.room_id,
                status: sess.status,
                is_service_session: sess.is_service_session,
                is_plenum_session: sess.is_plenum_session,
                is_confirmed: sess.is_confirmed,
                speakers: sess_speakers,
                categories: sess.categories,
            });
        }

        let mut rooms: Vec<String> = rooms_set.into_iter().collect();
        rooms.sort();
        let mut job_titles: Vec<String> = job_titles_set.into_iter().collect();
        job_titles.sort();
        let mut companies: Vec<String> = companies_set.into_iter().collect();
        companies.sort();

        let metadata = SessionizeMetadata {
            rooms,
            job_titles,
            companies,
            total_speakers: enriched_speakers.len(),
            total_sessions: enriched_sessions.len(),
            cached_at: js_sys::Date::new_0()
                .to_iso_string()
                .as_string()
                .unwrap_or_default(),
        };

        SessionizeDataPayload {
            speakers: enriched_speakers,
            sessions: enriched_sessions,
            metadata,
        }
    }

    /// Filter enriched payload according to query parameters.
    pub fn apply_filters(
        payload: SessionizeDataPayload,
        filter: &SessionizeQueryFilter,
    ) -> SessionizeDataPayload {
        let q_clean = filter
            .q
            .as_deref()
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty());
        let room_clean = filter
            .room
            .as_deref()
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty());
        let job_clean = filter
            .job_title
            .as_deref()
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty());
        let comp_clean = filter
            .company
            .as_deref()
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty());

        if q_clean.is_none() && room_clean.is_none() && job_clean.is_none() && comp_clean.is_none()
        {
            return payload;
        }

        let speakers: Vec<EnrichedSpeaker> = payload
            .speakers
            .into_iter()
            .filter(|sp| {
                // Room filter: speaker must have at least one session in that room
                if let Some(ref target_room) = room_clean {
                    let has_room = sp.sessions.iter().any(|s| {
                        s.room
                            .as_ref()
                            .map(|r| r.to_lowercase().contains(target_room))
                            .unwrap_or(false)
                    });
                    if !has_room {
                        return false;
                    }
                }

                // Job title filter
                if let Some(ref target_job) = job_clean {
                    let match_job = sp
                        .job_title
                        .as_ref()
                        .map(|j| j.to_lowercase().contains(target_job))
                        .unwrap_or(false)
                        || sp
                            .position_name
                            .as_ref()
                            .map(|j| j.to_lowercase().contains(target_job))
                            .unwrap_or(false);
                    if !match_job {
                        return false;
                    }
                }

                // Company filter
                if let Some(ref target_comp) = comp_clean {
                    let match_comp = sp
                        .company
                        .as_ref()
                        .map(|c| c.to_lowercase().contains(target_comp))
                        .unwrap_or(false);
                    if !match_comp {
                        return false;
                    }
                }

                // Keyword query filter: matches speaker or session details
                if let Some(ref q) = q_clean {
                    let in_speaker = sp.full_name.to_lowercase().contains(q)
                        || sp
                            .bio
                            .as_ref()
                            .map(|b| b.to_lowercase().contains(q))
                            .unwrap_or(false)
                        || sp
                            .tag_line
                            .as_ref()
                            .map(|t| t.to_lowercase().contains(q))
                            .unwrap_or(false)
                        || sp
                            .company
                            .as_ref()
                            .map(|c| c.to_lowercase().contains(q))
                            .unwrap_or(false)
                        || sp
                            .job_title
                            .as_ref()
                            .map(|j| j.to_lowercase().contains(q))
                            .unwrap_or(false);

                    let in_sessions = sp.sessions.iter().any(|s| {
                        s.title.to_lowercase().contains(q)
                            || s.description
                                .as_ref()
                                .map(|d| d.to_lowercase().contains(q))
                                .unwrap_or(false)
                            || s.room
                                .as_ref()
                                .map(|r| r.to_lowercase().contains(q))
                                .unwrap_or(false)
                    });

                    if !in_speaker && !in_sessions {
                        return false;
                    }
                }

                true
            })
            .collect();

        let sessions: Vec<EnrichedSession> = payload
            .sessions
            .into_iter()
            .filter(|sess| {
                // Room filter
                if let Some(ref target_room) = room_clean {
                    let match_room = sess
                        .room
                        .as_ref()
                        .map(|r| r.to_lowercase().contains(target_room))
                        .unwrap_or(false);
                    if !match_room {
                        return false;
                    }
                }

                // Job title filter: check speaker job title
                if let Some(ref target_job) = job_clean {
                    let match_job = sess.speakers.iter().any(|sp| {
                        sp.job_title
                            .as_ref()
                            .map(|j| j.to_lowercase().contains(target_job))
                            .unwrap_or(false)
                    });
                    if !match_job {
                        return false;
                    }
                }

                // Company filter: check speaker company
                if let Some(ref target_comp) = comp_clean {
                    let match_comp = sess.speakers.iter().any(|sp| {
                        sp.company
                            .as_ref()
                            .map(|c| c.to_lowercase().contains(target_comp))
                            .unwrap_or(false)
                    });
                    if !match_comp {
                        return false;
                    }
                }

                // Keyword query filter
                if let Some(ref q) = q_clean {
                    let in_session = sess.title.to_lowercase().contains(q)
                        || sess
                            .description
                            .as_ref()
                            .map(|d| d.to_lowercase().contains(q))
                            .unwrap_or(false)
                        || sess
                            .room
                            .as_ref()
                            .map(|r| r.to_lowercase().contains(q))
                            .unwrap_or(false);

                    let in_speakers = sess.speakers.iter().any(|sp| {
                        sp.name.to_lowercase().contains(q)
                            || sp
                                .company
                                .as_ref()
                                .map(|c| c.to_lowercase().contains(q))
                                .unwrap_or(false)
                            || sp
                                .tag_line
                                .as_ref()
                                .map(|t| t.to_lowercase().contains(q))
                                .unwrap_or(false)
                    });

                    if !in_session && !in_speakers {
                        return false;
                    }
                }

                true
            })
            .collect();

        SessionizeDataPayload {
            speakers,
            sessions,
            metadata: payload.metadata,
        }
    }
}
