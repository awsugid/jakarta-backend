use std::collections::HashMap;

use worker::{d1::D1Database, wasm_bindgen::JsValue, Result as WorkerResult};

use crate::schedule::types::{
    first_speaker_legacy, speakers_from_legacy, CreateFormatBody, CreateRoomBody,
    CreateScheduleItem, EventFormat, EventRoom, EventScheduleItem, PublicationRow,
    ScheduleOrderItem, ScheduleSpeaker, SyncSessionizeResponse, UpdateFormatBody, UpdateRoomBody,
    UpdateScheduleItem,
};
use crate::sessionize::types::EnrichedSession;
use crate::speakers::repository::changed_rows;

/// Normalize a clearable optional text field: `Some("")` (or whitespace)
/// means "clear" and becomes `None`.
fn normalize_clear(v: Option<String>) -> Option<String> {
    match v.as_deref() {
        Some(s) if s.trim().is_empty() => None,
        _ => v,
    }
}

/// Update tri-state for clearable optional text fields (sessionId,
/// description, track, startsAt, endsAt): `Some("")` clears (NULL), `None`
/// keeps the existing value.
fn clearable(input: Option<String>, existing: Option<String>) -> Option<String> {
    match input.as_deref() {
        Some(s) if s.trim().is_empty() => None, // explicit clear wins over existing
        Some(_) => input,
        None => existing,
    }
}

/// Item-level optimistic-concurrency failures.
#[derive(Debug)]
pub enum ItemMutationError {
    NotFound,
    StaleRevision,
}

/// Room/format-level optimistic-concurrency and guard failures.
#[derive(Debug)]
pub enum RoomMutationError {
    NotFound,
    StaleRevision,
    DuplicateName,
    /// Archive rejected: items still reference this room/format.
    Occupied(Vec<(String, String)>),
}

/// Publish-guard failures.
#[derive(Debug)]
pub enum PublishError {
    StaleRevision,
    Other(String),
}

// Workspace revision advances atomically with every relevant mutation via
// native AFTER INSERT/UPDATE/DELETE triggers (migration 0029) on
// event_schedules, event_rooms, event_formats and event_speakers — no
// separate bump round-trip that could race a publish guard.
pub struct ScheduleRepository {
    db: D1Database,
}

impl ScheduleRepository {
    pub fn new(db: D1Database) -> Self {
        Self { db }
    }

    /// List rooms for an event, ordered for display.
    pub async fn list_rooms(&self, event_slug: &str) -> WorkerResult<Vec<EventRoom>> {
        let sql =
            "SELECT * FROM event_rooms WHERE event_slug = ? ORDER BY display_order ASC, name ASC";
        let result = self
            .db
            .prepare(sql)
            .bind(&[JsValue::from_str(event_slug)])?
            .all()
            .await?;
        result.results::<EventRoom>()
    }

    /// Create a room; duplicate names are rejected.
    pub async fn create_room(
        &self,
        event_slug: &str,
        input: &CreateRoomBody,
    ) -> WorkerResult<Result<EventRoom, RoomMutationError>> {
        let name = input.name.trim();
        if name.is_empty() || name.len() > 100 {
            return Ok(Err(RoomMutationError::DuplicateName));
        }
        let existing = self.list_rooms(event_slug).await?;
        if existing.iter().any(|r| r.name.eq_ignore_ascii_case(name)) {
            return Ok(Err(RoomMutationError::DuplicateName));
        }
        let display_order = input
            .display_order
            .unwrap_or_else(|| existing.iter().map(|r| r.display_order).max().unwrap_or(0) + 1);
        let id = format!(
            "room-{}",
            js_sys::Math::random().to_string().replace("0.", "")
        );
        let room_create_sql = "INSERT INTO event_rooms (id, event_slug, name, display_order, archived, revision, created_at, updated_at) \
                               VALUES (?, ?, ?, ?, 0, 1, datetime('now'), datetime('now'))";
        self.db
            .prepare(room_create_sql)
            .bind(&[
                JsValue::from_str(&id),
                JsValue::from_str(event_slug),
                JsValue::from_str(name),
                JsValue::from_f64(display_order as f64),
            ])?
            .run()
            .await?;
        Ok(Ok(self
            .list_rooms(event_slug)
            .await?
            .into_iter()
            .find(|r| r.id == id)
            .ok_or_else(|| {
                worker::Error::RustError("Failed to load created room".into())
            })?))
    }

    /// Titles of schedule items still assigned to a room (archive guard).
    pub async fn room_occupants(
        &self,
        event_slug: &str,
        room_id: &str,
    ) -> WorkerResult<Vec<(String, String)>> {
        let room_occupancy_sql = "SELECT id, title FROM event_schedules \
                                  WHERE event_slug = ? AND EXISTS (SELECT 1 FROM json_each(room_ids_json) WHERE value = ?)";
        #[derive(serde::Deserialize)]
        struct Row {
            id: String,
            title: String,
        }
        let rows = self
            .db
            .prepare(room_occupancy_sql)
            .bind(&[JsValue::from_str(event_slug), JsValue::from_str(room_id)])?
            .all()
            .await?
            .results::<Row>()?;
        Ok(rows.into_iter().map(|r| (r.id, r.title)).collect())
    }

    /// Update a room (rename / reorder / archive) with revision CAS.
    pub async fn update_room(
        &self,
        event_slug: &str,
        room_id: &str,
        input: &UpdateRoomBody,
    ) -> WorkerResult<Result<EventRoom, RoomMutationError>> {
        let existing = match self
            .list_rooms(event_slug)
            .await?
            .into_iter()
            .find(|r| r.id == room_id)
        {
            Some(r) => r,
            None => return Ok(Err(RoomMutationError::NotFound)),
        };
        let name = input
            .name
            .as_deref()
            .map(str::trim)
            .unwrap_or(&existing.name);
        if name.is_empty() || name.len() > 100 {
            return Ok(Err(RoomMutationError::DuplicateName));
        }
        if name != existing.name {
            let all = self.list_rooms(event_slug).await?;
            if all
                .iter()
                .any(|r| r.id != room_id && r.name.eq_ignore_ascii_case(name))
            {
                return Ok(Err(RoomMutationError::DuplicateName));
            }
        }
        let display_order = input.display_order.unwrap_or(existing.display_order);
        let archived = input.archived.unwrap_or(existing.archived);
        if archived && !existing.archived {
            let occupants = self.room_occupants(event_slug, room_id).await?;
            if !occupants.is_empty() {
                return Ok(Err(RoomMutationError::Occupied(occupants)));
            }
        }
        let room_update_sql = "UPDATE event_rooms SET name = ?, display_order = ?, archived = ?, \
                               revision = revision + 1, updated_at = datetime('now') \
                               WHERE event_slug = ? AND id = ? AND revision = ?";
        let res = self
            .db
            .prepare(room_update_sql)
            .bind(&[
                JsValue::from_str(name),
                JsValue::from_f64(display_order as f64),
                JsValue::from_f64(if archived { 1.0 } else { 0.0 }),
                JsValue::from_str(event_slug),
                JsValue::from_str(room_id),
                JsValue::from_f64(input.revision as f64),
            ])?
            .run()
            .await?;
        if !changed_rows(&res) {
            return Ok(Err(RoomMutationError::StaleRevision));
        }
        Ok(Ok(self
            .list_rooms(event_slug)
            .await?
            .into_iter()
            .find(|r| r.id == room_id)
            .ok_or_else(|| {
                worker::Error::RustError("Failed to load updated room".into())
            })?))
    }

    /// List talk formats for an event, ordered for display.
    pub async fn list_formats(&self, event_slug: &str) -> WorkerResult<Vec<EventFormat>> {
        let sql =
            "SELECT * FROM event_formats WHERE event_slug = ? ORDER BY display_order ASC, name ASC";
        let result = self
            .db
            .prepare(sql)
            .bind(&[JsValue::from_str(event_slug)])?
            .all()
            .await?;
        result.results::<EventFormat>()
    }

    /// Create a talk format; duplicate names are rejected.
    pub async fn create_format(
        &self,
        event_slug: &str,
        input: &CreateFormatBody,
    ) -> WorkerResult<Result<EventFormat, RoomMutationError>> {
        let name = input.name.trim();
        if name.is_empty() || name.len() > 100 {
            return Ok(Err(RoomMutationError::DuplicateName));
        }
        let existing = self.list_formats(event_slug).await?;
        if existing.iter().any(|f| f.name.eq_ignore_ascii_case(name)) {
            return Ok(Err(RoomMutationError::DuplicateName));
        }
        let display_order = input
            .display_order
            .unwrap_or_else(|| existing.iter().map(|f| f.display_order).max().unwrap_or(0) + 1);
        let id = format!(
            "fmt-{}",
            js_sys::Math::random().to_string().replace("0.", "")
        );
        let format_create_sql = "INSERT INTO event_formats (id, event_slug, name, display_order, archived, revision, created_at, updated_at) \
                                 VALUES (?, ?, ?, ?, 0, 1, datetime('now'), datetime('now'))";
        self.db
            .prepare(format_create_sql)
            .bind(&[
                JsValue::from_str(&id),
                JsValue::from_str(event_slug),
                JsValue::from_str(name),
                JsValue::from_f64(display_order as f64),
            ])?
            .run()
            .await?;
        Ok(Ok(self
            .list_formats(event_slug)
            .await?
            .into_iter()
            .find(|f| f.id == id)
            .ok_or_else(|| {
                worker::Error::RustError("Failed to load created format".into())
            })?))
    }

    /// Titles of schedule items still linked to a format (archive guard).
    pub async fn format_occupants(
        &self,
        event_slug: &str,
        format_id: &str,
    ) -> WorkerResult<Vec<(String, String)>> {
        let format_occupancy_sql = "SELECT id, title FROM event_schedules \
                                    WHERE event_slug = ? AND format_id = ?";
        #[derive(serde::Deserialize)]
        struct Row {
            id: String,
            title: String,
        }
        let rows = self
            .db
            .prepare(format_occupancy_sql)
            .bind(&[JsValue::from_str(event_slug), JsValue::from_str(format_id)])?
            .all()
            .await?
            .results::<Row>()?;
        Ok(rows.into_iter().map(|r| (r.id, r.title)).collect())
    }

    /// Update a talk format (rename / reorder / archive) with revision CAS.
    pub async fn update_format(
        &self,
        event_slug: &str,
        format_id: &str,
        input: &UpdateFormatBody,
    ) -> WorkerResult<Result<EventFormat, RoomMutationError>> {
        let existing = match self
            .list_formats(event_slug)
            .await?
            .into_iter()
            .find(|f| f.id == format_id)
        {
            Some(f) => f,
            None => return Ok(Err(RoomMutationError::NotFound)),
        };
        let name = input
            .name
            .as_deref()
            .map(str::trim)
            .unwrap_or(&existing.name);
        if name.is_empty() || name.len() > 100 {
            return Ok(Err(RoomMutationError::DuplicateName));
        }
        if name != existing.name {
            let all = self.list_formats(event_slug).await?;
            if all
                .iter()
                .any(|f| f.id != format_id && f.name.eq_ignore_ascii_case(name))
            {
                return Ok(Err(RoomMutationError::DuplicateName));
            }
        }
        let display_order = input.display_order.unwrap_or(existing.display_order);
        let archived = input.archived.unwrap_or(existing.archived);
        if archived && !existing.archived {
            let occupants = self.format_occupants(event_slug, format_id).await?;
            if !occupants.is_empty() {
                return Ok(Err(RoomMutationError::Occupied(occupants)));
            }
        }
        let format_update_sql =
            "UPDATE event_formats SET name = ?, display_order = ?, archived = ?, \
                                 revision = revision + 1, updated_at = datetime('now') \
                                 WHERE event_slug = ? AND id = ? AND revision = ?";
        let res = self
            .db
            .prepare(format_update_sql)
            .bind(&[
                JsValue::from_str(name),
                JsValue::from_f64(display_order as f64),
                JsValue::from_f64(if archived { 1.0 } else { 0.0 }),
                JsValue::from_str(event_slug),
                JsValue::from_str(format_id),
                JsValue::from_f64(input.revision as f64),
            ])?
            .run()
            .await?;
        if !changed_rows(&res) {
            return Ok(Err(RoomMutationError::StaleRevision));
        }
        Ok(Ok(self
            .list_formats(event_slug)
            .await?
            .into_iter()
            .find(|f| f.id == format_id)
            .ok_or_else(|| {
                worker::Error::RustError("Failed to load updated format".into())
            })?))
    }

    /// Ensure a workspace row exists and return the current revision.
    pub async fn workspace_revision(&self, event_slug: &str) -> WorkerResult<i64> {
        let ensure_sql =
            "INSERT OR IGNORE INTO event_schedule_workspace (event_slug, revision) VALUES (?, 1)";
        self.db
            .prepare(ensure_sql)
            .bind(&[JsValue::from_str(event_slug)])?
            .run()
            .await?;
        let sql = "SELECT revision FROM event_schedule_workspace WHERE event_slug = ?";
        let rev = self
            .db
            .prepare(sql)
            .bind(&[JsValue::from_str(event_slug)])?
            .first::<f64>(None)
            .await?
            .unwrap_or(1.0);
        Ok(rev as i64)
    }

    /// Latest publication row, if this event was ever published (or backfilled).
    pub async fn get_publication(&self, event_slug: &str) -> WorkerResult<Option<PublicationRow>> {
        let sql = "SELECT * FROM event_schedule_publications WHERE event_slug = ?";
        self.db
            .prepare(sql)
            .bind(&[JsValue::from_str(event_slug)])?
            .first(None)
            .await
    }

    /// Speaker directory entries usable for schedule speaker resolution.
    pub async fn list_directory_speakers(
        &self,
        event_slug: &str,
    ) -> WorkerResult<HashMap<String, ScheduleSpeaker>> {
        let sql = "SELECT id, name, role, company, photo_url, social_url FROM event_speakers \
                   WHERE event_slug = ?";
        #[derive(serde::Deserialize)]
        struct Row {
            id: String,
            name: String,
            role: Option<String>,
            company: Option<String>,
            photo_url: Option<String>,
            social_url: Option<String>,
        }
        let rows = self
            .db
            .prepare(sql)
            .bind(&[JsValue::from_str(event_slug)])?
            .all()
            .await?
            .results::<Row>()?;
        Ok(rows
            .into_iter()
            .map(|r| {
                (
                    r.id,
                    ScheduleSpeaker {
                        id: None,
                        name: r.name,
                        role: r.role,
                        company: r.company,
                        avatar: r.photo_url,
                        social_url: r.social_url,
                    },
                )
            })
            .collect())
    }

    /// Atomically publish a snapshot: batched [guarded upsert, workspace bump].
    /// The guard SELECT embeds the expected workspace revision; a mismatch
    /// yields NULL for the NOT NULL `revision` column, failing the whole D1
    /// batch (transactional) — so no residual read-validate-write race.
    pub async fn publish_snapshot(
        &self,
        event_slug: &str,
        snapshot_json: &str,
        expected_revision: i64,
    ) -> WorkerResult<Result<String, PublishError>> {
        let current = self.workspace_revision(event_slug).await?;
        if current != expected_revision {
            return Ok(Err(PublishError::StaleRevision));
        }
        let publish_guard_sql = "INSERT INTO event_schedule_publications \
                                 (event_slug, snapshot_json, published_at, published_by, revision) \
                                 VALUES (?1, ?2, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), 'admin', \
                                 (SELECT revision FROM event_schedule_workspace WHERE event_slug = ?1 AND revision = ?3)) \
                                 ON CONFLICT(event_slug) DO UPDATE SET snapshot_json = excluded.snapshot_json, \
                                 published_at = excluded.published_at, published_by = excluded.published_by, revision = excluded.revision";
        let workspace_bump_sql =
            "UPDATE event_schedule_workspace SET revision = revision + 1 WHERE event_slug = ?";
        let batch = self.db.batch(vec![
            self.db.prepare(publish_guard_sql).bind(&[
                JsValue::from_str(event_slug),
                JsValue::from_str(snapshot_json),
                JsValue::from_f64(expected_revision as f64),
            ])?,
            self.db
                .prepare(workspace_bump_sql)
                .bind(&[JsValue::from_str(event_slug)])?,
        ]);
        if let Err(e) = batch.await {
            let msg = e.to_string();
            if msg.contains("NOT NULL") {
                return Ok(Err(PublishError::StaleRevision));
            }
            return Ok(Err(PublishError::Other(msg)));
        }
        let published_at = self
            .get_publication(event_slug)
            .await?
            .map(|p| p.published_at)
            .ok_or_else(|| worker::Error::RustError("Publication missing after publish".into()))?;
        Ok(Ok(published_at))
    }

    /// Fill resolved room names (`rooms`) and format names (`format`) for
    /// admin listing / review display.
    pub async fn resolve_item_display(
        &self,
        event_slug: &str,
        items: &mut [EventScheduleItem],
    ) -> WorkerResult<()> {
        if items.is_empty() {
            return Ok(());
        }
        let rooms = self.list_rooms(event_slug).await?;
        let formats = self.list_formats(event_slug).await?;
        let room_by_id: HashMap<&str, &EventRoom> =
            rooms.iter().map(|r| (r.id.as_str(), r)).collect();
        let active_names: Vec<String> = rooms
            .iter()
            .filter(|r| !r.archived)
            .map(|r| r.name.clone())
            .collect();
        for it in items.iter_mut() {
            it.rooms = Some(if it.all_rooms {
                active_names.clone()
            } else {
                it.room_ids
                    .iter()
                    .filter_map(|id| room_by_id.get(id.as_str()).map(|r| r.name.clone()))
                    .collect()
            });
            if let Some(fid) = it.format_id.as_deref() {
                it.format = formats.iter().find(|f| f.id == fid).map(|f| f.name.clone());
            }
        }
        Ok(())
    }

    /// List all schedule items (active & inactive) for admin management,
    /// with resolved room and format names.
    pub async fn list_admin_schedule(
        &self,
        event_slug: &str,
    ) -> WorkerResult<Vec<EventScheduleItem>> {
        let sql = "SELECT * FROM event_schedules \
                   WHERE event_slug = ? \
                   ORDER BY starts_at ASC, display_order ASC";
        let bind_values = vec![JsValue::from_str(event_slug)];
        let result = self.db.prepare(sql).bind(&bind_values)?.all().await?;
        let mut items = result.results::<EventScheduleItem>()?;
        self.resolve_item_display(event_slug, &mut items).await?;
        Ok(items)
    }

    /// Get single schedule item by id.
    pub async fn get_schedule_item(
        &self,
        event_slug: &str,
        item_id: &str,
    ) -> WorkerResult<Option<EventScheduleItem>> {
        let sql = "SELECT * FROM event_schedules WHERE event_slug = ? AND id = ? LIMIT 1";
        let bind_values = vec![JsValue::from_str(event_slug), JsValue::from_str(item_id)];
        let result = self.db.prepare(sql).bind(&bind_values)?.first(None).await?;
        Ok(result)
    }

    /// Create new schedule item. Input must already be resolved by the
    /// handler (speakerIds -> speakers, room names -> ids, format validated);
    /// `session_type` carries the display label (mirrors linked format name).
    pub async fn create_schedule_item(
        &self,
        event_slug: &str,
        input: CreateScheduleItem,
    ) -> WorkerResult<EventScheduleItem> {
        // Empty strings for clearable optional fields mean "not set".
        let input = CreateScheduleItem {
            session_id: normalize_clear(input.session_id),
            description: normalize_clear(input.description),
            track: normalize_clear(input.track),
            starts_at: normalize_clear(input.starts_at),
            ends_at: normalize_clear(input.ends_at),
            ..input
        };
        let id = format!(
            "sched-{}",
            js_sys::Math::random().to_string().replace("0.", "")
        );
        let display_order = input.display_order.unwrap_or(0);
        let is_service = if input.is_service_session.unwrap_or(false) {
            1
        } else {
            0
        };
        let is_active = if input.is_active.unwrap_or(true) {
            1
        } else {
            0
        };
        // Array wins when present; otherwise derive it from legacy singular fields.
        let speakers = match input.speakers {
            Some(sp) => sp,
            None => speakers_from_legacy(
                &input.speaker_id,
                &input.speaker_name,
                &input.speaker_role,
                &input.speaker_company,
                &input.speaker_avatar,
                &input.speaker_social_url,
            ),
        };
        let (
            speaker_id,
            speaker_name,
            speaker_role,
            speaker_company,
            speaker_avatar,
            speaker_social_url,
        ) = first_speaker_legacy(&speakers);
        let speakers_json = serde_json::to_string(&speakers).unwrap_or_else(|_| "[]".to_string());
        let speaker_ids = input.speaker_ids.unwrap_or_default();
        let speaker_ids_json =
            serde_json::to_string(&speaker_ids).unwrap_or_else(|_| "[]".to_string());

        let all_rooms = input.all_rooms.unwrap_or(false);
        let room_ids = if all_rooms {
            Vec::new()
        } else {
            input.room_ids.unwrap_or_default()
        };
        let room_ids_json = serde_json::to_string(&room_ids).unwrap_or_else(|_| "[]".to_string());
        // Legacy `room` label mirrors the first resolved room name.
        let rooms = self.list_rooms(event_slug).await?;
        let legacy_room = if all_rooms {
            None
        } else {
            room_ids
                .first()
                .and_then(|rid| rooms.iter().find(|r| &r.id == rid).map(|r| r.name.clone()))
        };
        // Linked format drives the label when no explicit sessionType is sent.
        let format_id = input.format_id.filter(|f| !f.is_empty());
        let session_type = match (&format_id, &input.session_type) {
            (Some(fid), _) => self
                .list_formats(event_slug)
                .await?
                .into_iter()
                .find(|f| &f.id == fid)
                .map(|f| f.name)
                .or(input.session_type.clone()),
            (None, st) => st.clone(),
        };

        let create_sql = "INSERT INTO event_schedules ( \
                   id, event_slug, session_id, title, description, \
                   speaker_id, speaker_name, speaker_role, speaker_company, speaker_avatar, speaker_social_url, \
                   room, track, starts_at, ends_at, session_type, is_service_session, is_active, display_order, \
                   speakers_json, room_ids_json, all_rooms, speaker_ids_json, format_id \
                   ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)";

        let bind_values = vec![
            JsValue::from_str(&id),
            JsValue::from_str(event_slug),
            input
                .session_id
                .as_deref()
                .map(JsValue::from_str)
                .unwrap_or(JsValue::NULL),
            JsValue::from_str(&input.title),
            input
                .description
                .as_deref()
                .map(JsValue::from_str)
                .unwrap_or(JsValue::NULL),
            speaker_id
                .as_deref()
                .map(JsValue::from_str)
                .unwrap_or(JsValue::NULL),
            speaker_name
                .as_deref()
                .map(JsValue::from_str)
                .unwrap_or(JsValue::NULL),
            speaker_role
                .as_deref()
                .map(JsValue::from_str)
                .unwrap_or(JsValue::NULL),
            speaker_company
                .as_deref()
                .map(JsValue::from_str)
                .unwrap_or(JsValue::NULL),
            speaker_avatar
                .as_deref()
                .map(JsValue::from_str)
                .unwrap_or(JsValue::NULL),
            speaker_social_url
                .as_deref()
                .map(JsValue::from_str)
                .unwrap_or(JsValue::NULL),
            legacy_room
                .as_deref()
                .map(JsValue::from_str)
                .unwrap_or(JsValue::NULL),
            input
                .track
                .as_deref()
                .map(JsValue::from_str)
                .unwrap_or(JsValue::NULL),
            input
                .starts_at
                .as_deref()
                .map(JsValue::from_str)
                .unwrap_or(JsValue::NULL),
            input
                .ends_at
                .as_deref()
                .map(JsValue::from_str)
                .unwrap_or(JsValue::NULL),
            session_type
                .as_deref()
                .map(JsValue::from_str)
                .unwrap_or(JsValue::NULL),
            JsValue::from_f64(is_service as f64),
            JsValue::from_f64(is_active as f64),
            JsValue::from_f64(display_order as f64),
            JsValue::from_str(&speakers_json),
            JsValue::from_str(&room_ids_json),
            JsValue::from_f64(if all_rooms { 1.0 } else { 0.0 }),
            JsValue::from_str(&speaker_ids_json),
            format_id
                .as_deref()
                .map(JsValue::from_str)
                .unwrap_or(JsValue::NULL),
        ];

        self.db
            .prepare(create_sql)
            .bind(&bind_values)?
            .run()
            .await?;

        let mut item = self
            .get_schedule_item(event_slug, &id)
            .await?
            .ok_or_else(|| {
                worker::Error::RustError("Failed to load newly created schedule item".to_string())
            })?;
        self.resolve_item_display(event_slug, std::slice::from_mut(&mut item))
            .await?;
        Ok(item)
    }

    /// Update existing schedule item with revision CAS. Handler pre-resolves
    /// speakerIds/room names/format; tri-state rules:
    /// - `speaker_ids` Some -> replace links (display = resolved speakers);
    ///   speakers Some without ids -> free-form speakers, links cleared.
    /// - `room_ids` Some -> replace (empty unless allRooms); `all_rooms` Some -> flag.
    /// - `format_id` Some("") clears the link; Some(id) sets; None keeps. The
    ///   linked format's name always wins the session_type label.
    pub async fn update_schedule_item(
        &self,
        event_slug: &str,
        item_id: &str,
        input: UpdateScheduleItem,
    ) -> WorkerResult<Result<EventScheduleItem, ItemMutationError>> {
        let existing = match self.get_schedule_item(event_slug, item_id).await? {
            Some(e) => e,
            None => return Ok(Err(ItemMutationError::NotFound)),
        };

        let session_id = clearable(input.session_id, existing.session_id);
        let title = input.title.unwrap_or(existing.title);
        let description = clearable(input.description, existing.description);
        // Array wins when present (singular mirrors first entry); omitted keeps
        // both the array and any legacy singular edits. `Some([])` clears all
        // speaker columns.
        let speakers_json = input
            .speakers
            .as_ref()
            .map(|sp| serde_json::to_string(sp).unwrap_or_else(|_| "[]".to_string()));
        let (
            speaker_id,
            speaker_name,
            speaker_role,
            speaker_company,
            speaker_avatar,
            speaker_social_url,
        ) = match input.speakers {
            Some(ref sp) => first_speaker_legacy(sp),
            None => (
                input.speaker_id.or(existing.speaker_id),
                input.speaker_name.or(existing.speaker_name),
                input.speaker_role.or(existing.speaker_role),
                input.speaker_company.or(existing.speaker_company),
                input.speaker_avatar.or(existing.speaker_avatar),
                input.speaker_social_url.or(existing.speaker_social_url),
            ),
        };
        let speaker_ids_json = match (&input.speaker_ids, &input.speakers) {
            (Some(ids), _) => Some(serde_json::to_string(ids).unwrap_or_else(|_| "[]".to_string())),
            (None, Some(_)) => Some("[]".to_string()),
            (None, None) => None,
        };

        let track = clearable(input.track, existing.track);
        let starts_at = clearable(input.starts_at, existing.starts_at);
        let ends_at = clearable(input.ends_at, existing.ends_at);
        let is_service = input
            .is_service_session
            .unwrap_or(existing.is_service_session);
        let is_active = input.is_active.unwrap_or(existing.is_active);
        let display_order = input.display_order.unwrap_or(existing.display_order);

        let all_rooms = input.all_rooms.unwrap_or(existing.all_rooms);
        let room_ids = if all_rooms {
            Vec::new()
        } else {
            input.room_ids.unwrap_or(existing.room_ids)
        };
        let room_ids_json = serde_json::to_string(&room_ids).unwrap_or_else(|_| "[]".to_string());
        let rooms = self.list_rooms(event_slug).await?;
        let legacy_room = if all_rooms {
            None
        } else {
            room_ids
                .first()
                .and_then(|rid| rooms.iter().find(|r| &r.id == rid).map(|r| r.name.clone()))
        };

        let final_format_id = match input.format_id.as_deref() {
            Some("") => None,
            Some(id) => Some(id.to_string()),
            None => existing.format_id.clone(),
        };
        let session_type = if let Some(fid) = &final_format_id {
            self.list_formats(event_slug)
                .await?
                .into_iter()
                .find(|f| &f.id == fid)
                .map(|f| f.name)
                .or_else(|| input.session_type.clone().or(existing.session_type.clone()))
        } else {
            input.session_type.clone().or(existing.session_type.clone())
        };

        let update_sql = "UPDATE event_schedules SET \
                   session_id = ?, title = ?, description = ?, \
                   speaker_id = ?, speaker_name = ?, speaker_role = ?, speaker_company = ?, speaker_avatar = ?, speaker_social_url = ?, \
                   speakers_json = COALESCE(?, speakers_json), speaker_ids_json = COALESCE(?, speaker_ids_json), \
                   room = ?, track = ?, starts_at = ?, ends_at = ?, session_type = ?, format_id = ?, \
                   room_ids_json = ?, all_rooms = ?, \
                   is_service_session = ?, is_active = ?, display_order = ?, \
                   revision = revision + 1, updated_at = datetime('now') \
                   WHERE event_slug = ? AND id = ? AND revision = ?";

        let bind_values = vec![
            session_id
                .as_deref()
                .map(JsValue::from_str)
                .unwrap_or(JsValue::NULL),
            JsValue::from_str(&title),
            description
                .as_deref()
                .map(JsValue::from_str)
                .unwrap_or(JsValue::NULL),
            speaker_id
                .as_deref()
                .map(JsValue::from_str)
                .unwrap_or(JsValue::NULL),
            speaker_name
                .as_deref()
                .map(JsValue::from_str)
                .unwrap_or(JsValue::NULL),
            speaker_role
                .as_deref()
                .map(JsValue::from_str)
                .unwrap_or(JsValue::NULL),
            speaker_company
                .as_deref()
                .map(JsValue::from_str)
                .unwrap_or(JsValue::NULL),
            speaker_avatar
                .as_deref()
                .map(JsValue::from_str)
                .unwrap_or(JsValue::NULL),
            speaker_social_url
                .as_deref()
                .map(JsValue::from_str)
                .unwrap_or(JsValue::NULL),
            speakers_json
                .as_deref()
                .map(JsValue::from_str)
                .unwrap_or(JsValue::NULL),
            speaker_ids_json
                .as_deref()
                .map(JsValue::from_str)
                .unwrap_or(JsValue::NULL),
            legacy_room
                .as_deref()
                .map(JsValue::from_str)
                .unwrap_or(JsValue::NULL),
            track
                .as_deref()
                .map(JsValue::from_str)
                .unwrap_or(JsValue::NULL),
            starts_at
                .as_deref()
                .map(JsValue::from_str)
                .unwrap_or(JsValue::NULL),
            ends_at
                .as_deref()
                .map(JsValue::from_str)
                .unwrap_or(JsValue::NULL),
            session_type
                .as_deref()
                .map(JsValue::from_str)
                .unwrap_or(JsValue::NULL),
            final_format_id
                .as_deref()
                .map(JsValue::from_str)
                .unwrap_or(JsValue::NULL),
            JsValue::from_str(&room_ids_json),
            JsValue::from_f64(if all_rooms { 1.0 } else { 0.0 }),
            JsValue::from_f64((if is_service { 1 } else { 0 }) as f64),
            JsValue::from_f64((if is_active { 1 } else { 0 }) as f64),
            JsValue::from_f64(display_order as f64),
            JsValue::from_str(event_slug),
            JsValue::from_str(item_id),
            JsValue::from_f64(input.revision as f64),
        ];

        let res = self
            .db
            .prepare(update_sql)
            .bind(&bind_values)?
            .run()
            .await?;
        if !changed_rows(&res) {
            return Ok(Err(ItemMutationError::StaleRevision));
        }

        let mut updated = match self.get_schedule_item(event_slug, item_id).await? {
            Some(i) => i,
            None => return Ok(Err(ItemMutationError::NotFound)),
        };
        self.resolve_item_display(event_slug, std::slice::from_mut(&mut updated))
            .await?;
        Ok(Ok(updated))
    }

    /// Delete schedule item with revision CAS (`?revision=N`).
    pub async fn delete_schedule_item(
        &self,
        event_slug: &str,
        item_id: &str,
        revision: i32,
    ) -> WorkerResult<Result<(), ItemMutationError>> {
        let sql = "DELETE FROM event_schedules WHERE event_slug = ? AND id = ? AND revision = ?";
        let bind_values = vec![
            JsValue::from_str(event_slug),
            JsValue::from_str(item_id),
            JsValue::from_f64(revision as f64),
        ];
        let res = self.db.prepare(sql).bind(&bind_values)?.run().await?;
        if changed_rows(&res) {
            return Ok(Ok(()));
        }
        if self.get_schedule_item(event_slug, item_id).await?.is_some() {
            return Ok(Err(ItemMutationError::StaleRevision));
        }
        Ok(Err(ItemMutationError::NotFound))
    }

    /// Reorder schedule items atomically: one D1 batch (transactional), so a
    /// concurrent publish either sees all new orders or none. Revision bumps
    /// happen via the workspace triggers on each row update.
    pub async fn reorder_schedule_items(
        &self,
        event_slug: &str,
        items: Vec<ScheduleOrderItem>,
    ) -> WorkerResult<()> {
        let sql = "UPDATE event_schedules SET display_order = ?, revision = revision + 1, updated_at = datetime('now') WHERE event_slug = ? AND id = ?";
        let mut statements = Vec::with_capacity(items.len());
        for item in items {
            statements.push(self.db.prepare(sql).bind(&[
                JsValue::from_f64(item.display_order as f64),
                JsValue::from_str(event_slug),
                JsValue::from_str(&item.id),
            ])?);
        }
        if !statements.is_empty() {
            self.db.batch(statements).await?;
        }
        Ok(())
    }

    /// Sync or import sessions from Sessionize into event_schedules.
    ///
    /// Idempotent on (event_slug, session_id) via partial unique index;
    /// INSERT OR IGNORE keeps concurrent imports conflict-safe. Insert-only
    /// unless `overwrite` (admin edits stay untouched on the daily path).
    /// `speaker_id` stores the local speaker-directory id when the speaker was
    /// imported (matched by sessionize_id), otherwise the upstream Sessionize UUID.
    pub async fn sync_from_sessionize(
        &self,
        event_slug: &str,
        sessions: Vec<EnrichedSession>,
        selected_ids: Option<Vec<String>>,
        overwrite: bool,
    ) -> WorkerResult<SyncSessionizeResponse> {
        #[derive(serde::Deserialize)]
        struct DirectorySpeakerLink {
            id: String,
            sessionize_id: String,
        }

        let directory_sql = "SELECT id, sessionize_id FROM event_speakers \
                             WHERE event_slug = ? AND sessionize_id IS NOT NULL";
        let directory = self
            .db
            .prepare(directory_sql)
            .bind(&[JsValue::from_str(event_slug)])?
            .all()
            .await?
            .results::<DirectorySpeakerLink>()?;
        let resolve_speaker_id = |upstream: &str| -> String {
            directory
                .iter()
                .find(|d| d.sessionize_id == upstream)
                .map(|d| d.id.clone())
                .unwrap_or_else(|| upstream.to_string())
        };

        let current_items = self.list_admin_schedule(event_slug).await?;
        let mut imported = 0;
        let mut updated = 0;

        // Imports land as unscheduled, inactive drafts: upstream timing
        // and room assignments are ignored entirely.
        let insert_sql = "INSERT OR IGNORE INTO event_schedules ( \
                          id, event_slug, session_id, title, description, \
                          speaker_id, speaker_name, speaker_role, speaker_company, speaker_avatar, speaker_social_url, \
                          room, track, starts_at, ends_at, session_type, is_service_session, is_active, display_order, \
                          speakers_json, room_ids_json, all_rooms, speaker_ids_json \
                          ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, NULL, NULL, NULL, ?, ?, 0, ?, ?, '[]', 0, ?)";

        for (idx, sess) in sessions.into_iter().enumerate() {
            if let Some(ref sel) = selected_ids {
                if !sel.contains(&sess.id) {
                    continue;
                }
            }

            // Full speaker list; service sessions intentionally carry none.
            // Local speaker-directory ids win over upstream Sessionize UUIDs.
            let speakers: Vec<ScheduleSpeaker> = if sess.is_service_session {
                Vec::new()
            } else {
                sess.speakers
                    .iter()
                    .map(|s| ScheduleSpeaker {
                        id: Some(resolve_speaker_id(&s.id)),
                        name: s.name.clone(),
                        role: s.job_title.clone().or_else(|| s.tag_line.clone()),
                        company: s.company.clone(),
                        avatar: s.profile_picture.clone(),
                        social_url: s.linkedin_url.clone(),
                    })
                    .collect()
            };
            let (
                speaker_id,
                speaker_name,
                speaker_role,
                speaker_company,
                speaker_avatar,
                speaker_social_url,
            ) = first_speaker_legacy(&speakers);
            let speakers_json =
                serde_json::to_string(&speakers).unwrap_or_else(|_| "[]".to_string());
            // Directory-linked speaker ids only (freeze candidates at publish).
            let speaker_ids: Vec<String> = speakers
                .iter()
                .filter_map(|s| s.id.clone())
                .filter(|id| directory.iter().any(|d| &d.id == id))
                .collect();
            let speaker_ids_json =
                serde_json::to_string(&speaker_ids).unwrap_or_else(|_| "[]".to_string());

            let session_type = if sess.is_service_session {
                Some("Service Session".to_string())
            } else if sess.is_plenum_session {
                Some("Plenum / Keynote".to_string())
            } else {
                Some("Technical Session".to_string())
            };

            // Check if existing item has matching session_id
            let existing = current_items
                .iter()
                .find(|it| it.session_id.as_deref() == Some(&sess.id));

            if let Some(ex) = existing {
                if overwrite {
                    // Overwrite refreshes content only: timing, rooms, format
                    // link and active flag are admin-owned and never touched.
                    let result = self
                        .update_schedule_item(
                            event_slug,
                            &ex.id,
                            UpdateScheduleItem {
                                revision: ex.revision,
                                session_id: Some(sess.id.clone()),
                                title: Some(sess.title),
                                description: sess.description,
                                speaker_id,
                                speaker_name,
                                speaker_role,
                                speaker_company,
                                speaker_avatar,
                                speaker_social_url,
                                speakers: Some(speakers),
                                speaker_ids: Some(speaker_ids),
                                room: None,
                                track: None,
                                room_ids: None,
                                all_rooms: None,
                                format_id: None,
                                starts_at: None,
                                ends_at: None,
                                session_type,
                                is_service_session: Some(sess.is_service_session),
                                is_active: None,
                                display_order: None,
                            },
                        )
                        .await?;
                    // A stale CAS means an admin edited concurrently; admin edit wins.
                    if result.is_ok() {
                        updated += 1;
                    }
                }
            } else {
                let id = format!(
                    "sched-{}",
                    js_sys::Math::random().to_string().replace("0.", "")
                );

                let bind_values = vec![
                    JsValue::from_str(&id),
                    JsValue::from_str(event_slug),
                    JsValue::from_str(&sess.id),
                    JsValue::from_str(&sess.title),
                    sess.description
                        .as_deref()
                        .map(JsValue::from_str)
                        .unwrap_or(JsValue::NULL),
                    speaker_id
                        .as_deref()
                        .map(JsValue::from_str)
                        .unwrap_or(JsValue::NULL),
                    speaker_name
                        .as_deref()
                        .map(JsValue::from_str)
                        .unwrap_or(JsValue::NULL),
                    speaker_role
                        .as_deref()
                        .map(JsValue::from_str)
                        .unwrap_or(JsValue::NULL),
                    speaker_company
                        .as_deref()
                        .map(JsValue::from_str)
                        .unwrap_or(JsValue::NULL),
                    speaker_avatar
                        .as_deref()
                        .map(JsValue::from_str)
                        .unwrap_or(JsValue::NULL),
                    speaker_social_url
                        .as_deref()
                        .map(JsValue::from_str)
                        .unwrap_or(JsValue::NULL),
                    session_type
                        .as_deref()
                        .map(JsValue::from_str)
                        .unwrap_or(JsValue::NULL),
                    JsValue::from_f64(if sess.is_service_session { 1.0 } else { 0.0 }),
                    JsValue::from_f64(idx as f64),
                    JsValue::from_str(&speakers_json),
                    JsValue::from_str(&speaker_ids_json),
                ];

                let res = self
                    .db
                    .prepare(insert_sql)
                    .bind(&bind_values)?
                    .run()
                    .await?;
                if changed_rows(&res) {
                    imported += 1;
                }
            }
        }

        let total = self.list_admin_schedule(event_slug).await?.len();

        Ok(SyncSessionizeResponse {
            imported,
            updated,
            total,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{clearable, normalize_clear};

    #[test]
    fn empty_strings_clear_and_none_keeps() {
        // normalize_clear: Some("") (even whitespace) -> None; real values pass.
        assert_eq!(normalize_clear(Some("".into())), None);
        assert_eq!(normalize_clear(Some("   ".into())), None);
        assert_eq!(
            normalize_clear(Some("2026-10-21T09:00:00Z".into())).as_deref(),
            Some("2026-10-21T09:00:00Z")
        );
        let absent: Option<String> = None;
        assert_eq!(normalize_clear(absent), None);

        // clearable: None keeps existing, Some("") clears, Some(v) replaces.
        let existing = Some("old".to_string());
        assert_eq!(clearable(None, existing.clone()), existing);
        assert_eq!(clearable(Some("".into()), existing.clone()), None);
        assert_eq!(
            clearable(Some("new".into()), existing),
            Some("new".to_string())
        );
    }
}
