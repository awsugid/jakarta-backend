use std::collections::{HashMap, HashSet};
use worker::{d1::D1Database, wasm_bindgen::JsValue, Fetch, Method, Request, RequestInit};

use crate::sessionize::types::{
    EnrichedSession, EnrichedSessionSpeaker, EnrichedSpeaker, EnrichedSpeakerSession,
    SessionizeDataPayload, SessionizeMetadata, SessionizeQueryFilter, SessionizeRawSession,
    SessionizeRawSessionGroup, SessionizeRawSpeaker,
};

pub const DEFAULT_SPEAKERS_URL: &str = "https://sessionize.com/api/v2/egep9rxt/view/Speakers";
pub const DEFAULT_SESSIONS_URL: &str = "https://sessionize.com/api/v2/egep9rxt/view/Sessions";

/// Error message prefix returned by `get_data` when no snapshot exists yet.
pub const NO_SNAPSHOT_ERROR: &str = "No Sessionize snapshot in D1 yet; an admin must run a fresh sync (fresh=true) or wait for the scheduled refresh";

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

    /// Read the D1 snapshot (non-fresh) or fetch upstream, filter to accepted
    /// sessions, and persist a new snapshot (fresh). Public reads never touch
    /// upstream; a missing snapshot is an error until an admin/scheduled job
    /// initializes it. Upstream failure leaves the stored snapshot untouched.
    pub async fn get_data(
        &self,
        db: &D1Database,
        fresh: bool,
    ) -> Result<SessionizeDataPayload, String> {
        if !fresh {
            let payload = Self::read_snapshot(db, &self.speakers_url, &self.sessions_url).await?;
            // Defensive: snapshots are filtered at write time, but re-apply so a
            // stale/hand-edited row can never leak non-accepted sessions.
            return Ok(Self::retain_accepted_sessions(payload));
        }

        let (raw_speakers, raw_session_groups) = self.fetch_upstream().await?;
        let payload = Self::retain_accepted_sessions(
            self.build_enriched_payload(raw_speakers, raw_session_groups),
        );
        Self::write_snapshot(db, &self.speakers_url, &self.sessions_url, &payload).await?;
        Ok(payload)
    }

    async fn read_snapshot(
        db: &D1Database,
        speakers_url: &str,
        sessions_url: &str,
    ) -> Result<SessionizeDataPayload, String> {
        let sql = "SELECT payload FROM sessionize_source_cache \
                   WHERE id = 1 AND speakers_url = ?1 AND sessions_url = ?2";
        let bind_values = vec![
            JsValue::from_str(speakers_url),
            JsValue::from_str(sessions_url),
        ];
        let row = db
            .prepare(sql)
            .bind(&bind_values)
            .map_err(|e| format!("failed to prepare snapshot read: {e}"))?
            .first::<String>(Some("payload"))
            .await
            .map_err(|e| format!("failed to read Sessionize snapshot from D1: {e}"))?;
        let Some(row) = row else {
            return Err(NO_SNAPSHOT_ERROR.to_string());
        };
        serde_json::from_str(&row)
            .map_err(|e| format!("corrupt Sessionize snapshot: failed to parse payload: {e}"))
    }

    async fn write_snapshot(
        db: &D1Database,
        speakers_url: &str,
        sessions_url: &str,
        payload: &SessionizeDataPayload,
    ) -> Result<(), String> {
        let json = serde_json::to_string(payload)
            .map_err(|e| format!("failed to serialize Sessionize payload: {e}"))?;
        let sql = "INSERT INTO sessionize_source_cache \
                   (id, speakers_url, sessions_url, payload, refreshed_at) \
                   VALUES (1, ?1, ?2, ?3, datetime('now')) \
                   ON CONFLICT(id) DO UPDATE SET \
                   speakers_url = excluded.speakers_url, \
                   sessions_url = excluded.sessions_url, \
                   payload = excluded.payload, \
                   refreshed_at = excluded.refreshed_at";
        let bind_values = vec![
            JsValue::from_str(speakers_url),
            JsValue::from_str(sessions_url),
            JsValue::from_str(&json),
        ];
        db.prepare(sql)
            .bind(&bind_values)
            .map_err(|e| format!("failed to prepare snapshot write: {e}"))?
            .run()
            .await
            .map_err(|e| format!("failed to persist Sessionize snapshot to D1: {e}"))?;
        Ok(())
    }

    /// Keep only sessions whose status is "Accepted" (case-insensitive).
    /// Missing/unknown status fails closed (excluded). Speakers are kept only
    /// when referenced by an accepted session, and each speaker's session list
    /// is trimmed to accepted IDs. Metadata facets/counts are recomputed.
    pub fn retain_accepted_sessions(payload: SessionizeDataPayload) -> SessionizeDataPayload {
        let is_accepted = |s: &EnrichedSession| {
            s.status
                .as_deref()
                .is_some_and(|st| st.trim().eq_ignore_ascii_case("accepted"))
        };

        let sessions: Vec<EnrichedSession> =
            payload.sessions.into_iter().filter(is_accepted).collect();

        let accepted_ids: HashSet<&str> = sessions.iter().map(|s| s.id.as_str()).collect();
        let referenced_speakers: HashSet<&str> = sessions
            .iter()
            .flat_map(|s| s.speakers.iter().map(|sp| sp.id.as_str()))
            .collect();

        let speakers: Vec<EnrichedSpeaker> = payload
            .speakers
            .into_iter()
            .filter(|sp| referenced_speakers.contains(sp.id.as_str()))
            .map(|mut sp| {
                sp.sessions.retain(|s| accepted_ids.contains(s.id.as_str()));
                sp
            })
            .collect();

        let mut rooms: HashSet<String> = HashSet::new();
        for sess in &sessions {
            if let Some(rm) = &sess.room {
                if !rm.trim().is_empty() {
                    rooms.insert(rm.trim().to_string());
                }
            }
        }
        let mut job_titles: HashSet<String> = HashSet::new();
        let mut companies: HashSet<String> = HashSet::new();
        for sp in &speakers {
            if let Some(j) = sp
                .job_title
                .as_deref()
                .map(str::trim)
                .filter(|j| !j.is_empty())
            {
                job_titles.insert(j.to_string());
            }
            if let Some(c) = sp
                .company
                .as_deref()
                .map(str::trim)
                .filter(|c| !c.is_empty())
            {
                companies.insert(c.to_string());
            }
        }

        let metadata = SessionizeMetadata {
            rooms: sorted_set(rooms),
            job_titles: sorted_set(job_titles),
            companies: sorted_set(companies),
            total_speakers: speakers.len(),
            total_sessions: sessions.len(),
            cached_at: payload.metadata.cached_at,
        };

        SessionizeDataPayload {
            speakers,
            sessions,
            metadata,
        }
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

fn sorted_set(set: HashSet<String>) -> Vec<String> {
    let mut v: Vec<String> = set.into_iter().collect();
    v.sort();
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn payload_json() -> serde_json::Value {
        json!({
            "speakers": [
                {
                    "id": "sp1",
                    "fullName": "Speaker One",
                    "isTopSpeaker": false,
                    "links": [],
                    "sessions": [
                        { "id": "s1", "title": "Talk One", "isServiceSession": false },
                        { "id": "s3", "title": "Ghost Talk", "isServiceSession": false }
                    ],
                    "jobTitle": "Dev Rel",
                    "company": "ACME"
                },
                {
                    "id": "sp2",
                    "fullName": "Speaker Two",
                    "isTopSpeaker": false,
                    "links": [],
                    "sessions": [
                        { "id": "s1", "title": "Talk One", "isServiceSession": false }
                    ]
                },
                {
                    "id": "sp3",
                    "fullName": "Speaker Three",
                    "isTopSpeaker": false,
                    "links": [],
                    "sessions": [
                        { "id": "s2", "title": "Talk Two", "isServiceSession": false }
                    ]
                },
                {
                    "id": "sp4",
                    "fullName": "Orphan Speaker",
                    "isTopSpeaker": false,
                    "links": [],
                    "sessions": [
                        { "id": "s3", "title": "Ghost Talk", "isServiceSession": false }
                    ]
                },
                {
                    "id": "sp5",
                    "fullName": "Declined Speaker",
                    "isTopSpeaker": false,
                    "links": [],
                    "sessions": []
                }
            ],
            "sessions": [
                {
                    "id": "s1",
                    "title": "Talk One",
                    "status": "Accepted",
                    "isServiceSession": false,
                    "isPlenumSession": false,
                    "room": "Room A",
                    "speakers": [
                        { "id": "sp1", "name": "Speaker One" },
                        { "id": "sp2", "name": "Speaker Two" }
                    ],
                    "categories": []
                },
                {
                    "id": "s2",
                    "title": "Talk Two",
                    "status": "accepted",
                    "isServiceSession": false,
                    "isPlenumSession": false,
                    "room": "Room B",
                    "speakers": [
                        { "id": "sp3", "name": "Speaker Three" }
                    ],
                    "categories": []
                },
                {
                    "id": "s3",
                    "title": "Missing Status",
                    "isServiceSession": false,
                    "isPlenumSession": false,
                    "room": "Room C",
                    "speakers": [
                        { "id": "sp4", "name": "Orphan Speaker" }
                    ],
                    "categories": []
                },
                {
                    "id": "s4",
                    "title": "Declined Talk",
                    "status": "Declined",
                    "isServiceSession": false,
                    "isPlenumSession": false,
                    "speakers": [
                        { "id": "sp5", "name": "Declined Speaker" }
                    ],
                    "categories": []
                }
            ],
            "metadata": {
                "rooms": ["Room A", "Room B", "Room C"],
                "jobTitles": ["Dev Rel", "Old Title"],
                "companies": ["ACME", "Gone Corp"],
                "totalSpeakers": 5,
                "totalSessions": 4,
                "cachedAt": "2026-10-21T00:00:00.000Z"
            }
        })
    }

    #[test]
    fn retain_accepted_filters_sessions_and_speakers() {
        let payload: SessionizeDataPayload =
            serde_json::from_value(payload_json()).expect("payload must deserialize");
        let filtered = SessionizeClient::retain_accepted_sessions(payload);

        let session_ids: Vec<&str> = filtered.sessions.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(
            session_ids,
            vec!["s1", "s2"],
            "only Accepted sessions (case-insensitive) survive; missing/Declined excluded"
        );

        let speaker_ids: Vec<&str> = filtered.speakers.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(
            speaker_ids,
            vec!["sp1", "sp2", "sp3"],
            "speakers referenced only by non-accepted sessions are dropped"
        );

        let sp1 = filtered.speakers.iter().find(|s| s.id == "sp1").unwrap();
        let sp1_sessions: Vec<&str> = sp1.sessions.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(
            sp1_sessions,
            vec!["s1"],
            "speaker.sessions keeps only accepted session IDs"
        );

        assert_eq!(filtered.metadata.total_speakers, 3);
        assert_eq!(filtered.metadata.total_sessions, 2);
        assert_eq!(
            filtered.metadata.rooms,
            vec!["Room A", "Room B"],
            "rooms recomputed from accepted sessions"
        );
        assert_eq!(filtered.metadata.job_titles, vec!["Dev Rel"]);
        assert_eq!(filtered.metadata.companies, vec!["ACME"]);
        assert_eq!(filtered.metadata.cached_at, "2026-10-21T00:00:00.000Z");
    }

    #[test]
    fn retain_accepted_fail_closed_on_all_rejected() {
        let mut payload: SessionizeDataPayload = serde_json::from_value(payload_json()).unwrap();
        for s in payload.sessions.iter_mut() {
            s.status = Some("Maybe".to_string());
        }
        let filtered = SessionizeClient::retain_accepted_sessions(payload);
        assert!(filtered.sessions.is_empty());
        assert!(filtered.speakers.is_empty());
    }
}
