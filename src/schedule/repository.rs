use worker::{d1::D1Database, wasm_bindgen::JsValue, Result as WorkerResult};

use crate::schedule::types::{
    CreateScheduleItem, EventScheduleItem, ScheduleOrderItem, SyncSessionizeResponse,
    UpdateScheduleItem,
};
use crate::sessionize::types::EnrichedSession;

pub struct ScheduleRepository {
    db: D1Database,
}

impl ScheduleRepository {
    pub fn new(db: D1Database) -> Self {
        Self { db }
    }

    /// List active schedule items for public display.
    pub async fn list_public_schedule(
        &self,
        event_slug: &str,
    ) -> WorkerResult<Vec<EventScheduleItem>> {
        let sql = "SELECT * FROM event_schedules \
                   WHERE event_slug = ? AND is_active = 1 \
                   ORDER BY starts_at ASC, display_order ASC";
        let bind_values = vec![JsValue::from_str(event_slug)];
        let result = self.db.prepare(sql).bind(&bind_values)?.all().await?;
        result.results::<EventScheduleItem>()
    }

    /// List all schedule items (active & inactive) for admin management.
    pub async fn list_admin_schedule(
        &self,
        event_slug: &str,
    ) -> WorkerResult<Vec<EventScheduleItem>> {
        let sql = "SELECT * FROM event_schedules \
                   WHERE event_slug = ? \
                   ORDER BY starts_at ASC, display_order ASC";
        let bind_values = vec![JsValue::from_str(event_slug)];
        let result = self.db.prepare(sql).bind(&bind_values)?.all().await?;
        result.results::<EventScheduleItem>()
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

    /// Create new schedule item.
    pub async fn create_schedule_item(
        &self,
        event_slug: &str,
        input: CreateScheduleItem,
    ) -> WorkerResult<EventScheduleItem> {
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

        let sql = "INSERT INTO event_schedules ( \
                   id, event_slug, session_id, title, description, \
                   speaker_id, speaker_name, speaker_role, speaker_company, speaker_avatar, speaker_social_url, \
                   room, track, starts_at, ends_at, session_type, is_service_session, is_active, display_order \
                   ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)";

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
            input
                .speaker_id
                .as_deref()
                .map(JsValue::from_str)
                .unwrap_or(JsValue::NULL),
            input
                .speaker_name
                .as_deref()
                .map(JsValue::from_str)
                .unwrap_or(JsValue::NULL),
            input
                .speaker_role
                .as_deref()
                .map(JsValue::from_str)
                .unwrap_or(JsValue::NULL),
            input
                .speaker_company
                .as_deref()
                .map(JsValue::from_str)
                .unwrap_or(JsValue::NULL),
            input
                .speaker_avatar
                .as_deref()
                .map(JsValue::from_str)
                .unwrap_or(JsValue::NULL),
            input
                .speaker_social_url
                .as_deref()
                .map(JsValue::from_str)
                .unwrap_or(JsValue::NULL),
            input
                .room
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
            input
                .session_type
                .as_deref()
                .map(JsValue::from_str)
                .unwrap_or(JsValue::NULL),
            JsValue::from_f64(is_service as f64),
            JsValue::from_f64(is_active as f64),
            JsValue::from_f64(display_order as f64),
        ];

        self.db.prepare(sql).bind(&bind_values)?.run().await?;

        self.get_schedule_item(event_slug, &id)
            .await?
            .ok_or_else(|| {
                worker::Error::RustError("Failed to load newly created schedule item".to_string())
            })
    }

    /// Update existing schedule item.
    pub async fn update_schedule_item(
        &self,
        event_slug: &str,
        item_id: &str,
        input: UpdateScheduleItem,
    ) -> WorkerResult<Option<EventScheduleItem>> {
        let existing = match self.get_schedule_item(event_slug, item_id).await? {
            Some(e) => e,
            None => return Ok(None),
        };

        let session_id = input.session_id.or(existing.session_id);
        let title = input.title.unwrap_or(existing.title);
        let description = input.description.or(existing.description);
        let speaker_id = input.speaker_id.or(existing.speaker_id);
        let speaker_name = input.speaker_name.or(existing.speaker_name);
        let speaker_role = input.speaker_role.or(existing.speaker_role);
        let speaker_company = input.speaker_company.or(existing.speaker_company);
        let speaker_avatar = input.speaker_avatar.or(existing.speaker_avatar);
        let speaker_social_url = input.speaker_social_url.or(existing.speaker_social_url);
        let room = input.room.or(existing.room);
        let track = input.track.or(existing.track);
        let starts_at = input.starts_at.or(existing.starts_at);
        let ends_at = input.ends_at.or(existing.ends_at);
        let session_type = input.session_type.or(existing.session_type);
        let is_service = input
            .is_service_session
            .unwrap_or(existing.is_service_session);
        let is_active = input.is_active.unwrap_or(existing.is_active);
        let display_order = input.display_order.unwrap_or(existing.display_order);

        let sql = "UPDATE event_schedules SET \
                   session_id = ?, title = ?, description = ?, \
                   speaker_id = ?, speaker_name = ?, speaker_role = ?, speaker_company = ?, speaker_avatar = ?, speaker_social_url = ?, \
                   room = ?, track = ?, starts_at = ?, ends_at = ?, session_type = ?, is_service_session = ?, is_active = ?, display_order = ?, \
                   updated_at = datetime('now') \
                   WHERE event_slug = ? AND id = ?";

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
            room.as_deref()
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
            JsValue::from_f64((if is_service { 1 } else { 0 }) as f64),
            JsValue::from_f64((if is_active { 1 } else { 0 }) as f64),
            JsValue::from_f64(display_order as f64),
            JsValue::from_str(event_slug),
            JsValue::from_str(item_id),
        ];

        self.db.prepare(sql).bind(&bind_values)?.run().await?;
        self.get_schedule_item(event_slug, item_id).await
    }

    /// Delete schedule item.
    pub async fn delete_schedule_item(
        &self,
        event_slug: &str,
        item_id: &str,
    ) -> WorkerResult<bool> {
        let sql = "DELETE FROM event_schedules WHERE event_slug = ? AND id = ?";
        let bind_values = vec![JsValue::from_str(event_slug), JsValue::from_str(item_id)];
        let res = self.db.prepare(sql).bind(&bind_values)?.run().await?;
        Ok(res.success())
    }

    /// Reorder schedule items in batch.
    pub async fn reorder_schedule_items(
        &self,
        event_slug: &str,
        items: Vec<ScheduleOrderItem>,
    ) -> WorkerResult<()> {
        let sql = "UPDATE event_schedules SET display_order = ?, updated_at = datetime('now') WHERE event_slug = ? AND id = ?";
        for item in items {
            let bind_values = vec![
                JsValue::from_f64(item.display_order as f64),
                JsValue::from_str(event_slug),
                JsValue::from_str(&item.id),
            ];
            self.db.prepare(sql).bind(&bind_values)?.run().await?;
        }
        Ok(())
    }

    /// Sync or import sessions from Sessionize into event_schedules.
    pub async fn sync_from_sessionize(
        &self,
        event_slug: &str,
        sessions: Vec<EnrichedSession>,
        selected_ids: Option<Vec<String>>,
        overwrite: bool,
    ) -> WorkerResult<SyncSessionizeResponse> {
        let current_items = self.list_admin_schedule(event_slug).await?;
        let mut imported = 0;
        let mut updated = 0;

        for (idx, sess) in sessions.into_iter().enumerate() {
            if let Some(ref sel) = selected_ids {
                if !sel.contains(&sess.id) {
                    continue;
                }
            }

            let first_sp = sess.speakers.first();
            let speaker_id = first_sp.map(|s| s.id.clone());
            let speaker_name = first_sp.map(|s| s.name.clone());
            let speaker_role =
                first_sp.and_then(|s| s.job_title.clone().or_else(|| s.tag_line.clone()));
            let speaker_company = first_sp.and_then(|s| s.company.clone());
            let speaker_avatar = first_sp.and_then(|s| s.profile_picture.clone());
            let speaker_social_url = first_sp.and_then(|s| s.linkedin_url.clone());

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
                    self.update_schedule_item(
                        event_slug,
                        &ex.id,
                        UpdateScheduleItem {
                            session_id: Some(sess.id.clone()),
                            title: Some(sess.title),
                            description: sess.description,
                            speaker_id,
                            speaker_name,
                            speaker_role,
                            speaker_company,
                            speaker_avatar,
                            speaker_social_url,
                            room: sess.room,
                            track: None,
                            starts_at: sess.starts_at,
                            ends_at: sess.ends_at,
                            session_type,
                            is_service_session: Some(sess.is_service_session),
                            is_active: None,
                            display_order: None,
                        },
                    )
                    .await?;
                    updated += 1;
                }
            } else {
                self.create_schedule_item(
                    event_slug,
                    CreateScheduleItem {
                        session_id: Some(sess.id.clone()),
                        title: sess.title,
                        description: sess.description,
                        speaker_id,
                        speaker_name,
                        speaker_role,
                        speaker_company,
                        speaker_avatar,
                        speaker_social_url,
                        room: sess.room,
                        track: None,
                        starts_at: sess.starts_at,
                        ends_at: sess.ends_at,
                        session_type,
                        is_service_session: Some(sess.is_service_session),
                        is_active: Some(true),
                        display_order: Some(idx as i32),
                    },
                )
                .await?;
                imported += 1;
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
