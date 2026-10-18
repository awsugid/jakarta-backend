use worker::{d1::D1Database, wasm_bindgen::JsValue, Result as WorkerResult};

use crate::speakers::types::{
    EventSpeaker, EventSpeakerCreate, EventSpeakerOrderItem, EventSpeakerUpdate,
};

#[derive(Debug)]
pub enum CreateSpeakerError {
    DuplicateName,
    SpeakerLimit,
    Db(worker::Error),
}

impl From<worker::Error> for CreateSpeakerError {
    fn from(e: worker::Error) -> Self {
        Self::Db(e)
    }
}

#[derive(Debug)]
pub enum UpdateSpeakerError {
    NotFound,
    DuplicateName,
    Db(worker::Error),
}

impl From<worker::Error> for UpdateSpeakerError {
    fn from(e: worker::Error) -> Self {
        Self::Db(e)
    }
}

#[derive(Debug)]
pub enum DeleteSpeakerError {
    NotFound,
    Db(worker::Error),
}

impl From<worker::Error> for DeleteSpeakerError {
    fn from(e: worker::Error) -> Self {
        Self::Db(e)
    }
}

#[derive(Debug)]
pub enum ReorderSpeakersError {
    InvalidList,
    Db(worker::Error),
}

impl From<worker::Error> for ReorderSpeakersError {
    fn from(e: worker::Error) -> Self {
        Self::Db(e)
    }
}

pub const MAX_SPEAKERS_PER_EVENT: usize = 100;

pub struct SpeakerRepository {
    db: D1Database,
}

impl SpeakerRepository {
    pub fn new(db: D1Database) -> Self {
        Self { db }
    }

    /// List active speakers for public carousel/showcase ordered by display_order.
    pub async fn list_public_speakers(&self, event_slug: &str) -> WorkerResult<Vec<EventSpeaker>> {
        let sql = r#"
            SELECT event_slug, id, name, role, company, photo_url, talk_title,
                   talk_format, topic_image_url, bio, social_url, is_active, display_order, created_at, updated_at
            FROM event_speakers
            WHERE event_slug = ? AND is_active = 1
            ORDER BY display_order ASC, id ASC
        "#;
        let result = self
            .db
            .prepare(sql)
            .bind(&[JsValue::from_str(event_slug)])?
            .all()
            .await?;
        result.results::<EventSpeaker>()
    }

    /// List all speakers (including inactive) for admin console.
    pub async fn list_speakers(&self, event_slug: &str) -> WorkerResult<Vec<EventSpeaker>> {
        let sql = r#"
            SELECT event_slug, id, name, role, company, photo_url, talk_title,
                   talk_format, topic_image_url, bio, social_url, is_active, display_order, created_at, updated_at
            FROM event_speakers
            WHERE event_slug = ?
            ORDER BY display_order ASC, id ASC
        "#;
        let result = self
            .db
            .prepare(sql)
            .bind(&[JsValue::from_str(event_slug)])?
            .all()
            .await?;
        result.results::<EventSpeaker>()
    }

    /// Get a single speaker by ID.
    pub async fn get_speaker(
        &self,
        event_slug: &str,
        speaker_id: &str,
    ) -> WorkerResult<Option<EventSpeaker>> {
        let sql = r#"
            SELECT event_slug, id, name, role, company, photo_url, talk_title,
                   talk_format, topic_image_url, bio, social_url, is_active, display_order, created_at, updated_at
            FROM event_speakers
            WHERE event_slug = ? AND id = ?
        "#;
        self.db
            .prepare(sql)
            .bind(&[JsValue::from_str(event_slug), JsValue::from_str(speaker_id)])?
            .first::<EventSpeaker>(None)
            .await
    }

    /// Create a new speaker for an event.
    pub async fn create_speaker(
        &self,
        event_slug: &str,
        input: EventSpeakerCreate,
    ) -> Result<EventSpeaker, CreateSpeakerError> {
        let existing = self.list_speakers(event_slug).await?;
        let name_trim = input.name.trim();

        if existing.len() >= MAX_SPEAKERS_PER_EVENT {
            return Err(CreateSpeakerError::SpeakerLimit);
        }

        if existing
            .iter()
            .any(|s| s.name.trim().eq_ignore_ascii_case(name_trim))
        {
            return Err(CreateSpeakerError::DuplicateName);
        }

        let base_slug = slugify(name_trim);
        let id = generate_unique_id(&base_slug, |cand| existing.iter().any(|s| s.id == cand));
        let display_order = existing.iter().map(|s| s.display_order).max().unwrap_or(0) + 1;
        let is_active = input.is_active.unwrap_or(true);

        let company = match input.company.as_deref() {
            Some(c) if !c.trim().is_empty() => JsValue::from_str(c.trim()),
            _ => JsValue::NULL,
        };
        let talk_format = match input.talk_format.as_deref() {
            Some(f) if !f.trim().is_empty() => JsValue::from_str(f.trim()),
            _ => JsValue::from_str("Technical Session"),
        };
        let topic_image_url = match input.topic_image_url.as_deref() {
            Some(t) if !t.trim().is_empty() => JsValue::from_str(t.trim()),
            _ => JsValue::NULL,
        };
        let bio = match input.bio.as_deref() {
            Some(b) if !b.trim().is_empty() => JsValue::from_str(b.trim()),
            _ => JsValue::NULL,
        };
        let social_url = match input.social_url.as_deref() {
            Some(u) if !u.trim().is_empty() => JsValue::from_str(u.trim()),
            _ => JsValue::NULL,
        };

        let sql = r#"
            INSERT INTO event_speakers
              (event_slug, id, name, role, company, photo_url, talk_title, talk_format,
               topic_image_url, bio, social_url, is_active, display_order)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#;

        self.db
            .prepare(sql)
            .bind(&[
                JsValue::from_str(event_slug),
                JsValue::from_str(&id),
                JsValue::from_str(name_trim),
                JsValue::from_str(input.role.trim()),
                company,
                JsValue::from_str(input.photo_url.trim()),
                JsValue::from_str(input.talk_title.trim()),
                talk_format,
                topic_image_url,
                bio,
                social_url,
                JsValue::from_f64(if is_active { 1.0 } else { 0.0 }),
                JsValue::from_f64(display_order as f64),
            ])?
            .run()
            .await?;

        let created = self.get_speaker(event_slug, &id).await?.ok_or_else(|| {
            worker::Error::RustError("Failed to load created speaker".to_string())
        })?;

        Ok(created)
    }

    /// Update an existing speaker.
    pub async fn update_speaker(
        &self,
        event_slug: &str,
        speaker_id: &str,
        input: EventSpeakerUpdate,
    ) -> Result<EventSpeaker, UpdateSpeakerError> {
        let existing = self.list_speakers(event_slug).await?;
        let name_trim = input.name.trim();

        if !existing.iter().any(|s| s.id == speaker_id) {
            return Err(UpdateSpeakerError::NotFound);
        }

        if existing
            .iter()
            .any(|s| s.id != speaker_id && s.name.trim().eq_ignore_ascii_case(name_trim))
        {
            return Err(UpdateSpeakerError::DuplicateName);
        }

        let company = match input.company.as_deref() {
            Some(c) if !c.trim().is_empty() => JsValue::from_str(c.trim()),
            _ => JsValue::NULL,
        };
        let talk_format = match input.talk_format.as_deref() {
            Some(f) if !f.trim().is_empty() => JsValue::from_str(f.trim()),
            _ => JsValue::from_str("Technical Session"),
        };
        let topic_image_url = match input.topic_image_url.as_deref() {
            Some(t) if !t.trim().is_empty() => JsValue::from_str(t.trim()),
            _ => JsValue::NULL,
        };
        let bio = match input.bio.as_deref() {
            Some(b) if !b.trim().is_empty() => JsValue::from_str(b.trim()),
            _ => JsValue::NULL,
        };
        let social_url = match input.social_url.as_deref() {
            Some(u) if !u.trim().is_empty() => JsValue::from_str(u.trim()),
            _ => JsValue::NULL,
        };

        let sql = r#"
            UPDATE event_speakers
            SET name = ?,
                role = ?,
                company = ?,
                photo_url = ?,
                talk_title = ?,
                talk_format = ?,
                topic_image_url = ?,
                bio = ?,
                social_url = ?,
                is_active = ?,
                updated_at = datetime('now')
            WHERE event_slug = ? AND id = ?
        "#;

        self.db
            .prepare(sql)
            .bind(&[
                JsValue::from_str(name_trim),
                JsValue::from_str(input.role.trim()),
                company,
                JsValue::from_str(input.photo_url.trim()),
                JsValue::from_str(input.talk_title.trim()),
                talk_format,
                topic_image_url,
                bio,
                social_url,
                JsValue::from_f64(if input.is_active { 1.0 } else { 0.0 }),
                JsValue::from_str(event_slug),
                JsValue::from_str(speaker_id),
            ])?
            .run()
            .await?;

        let updated = self
            .get_speaker(event_slug, speaker_id)
            .await?
            .ok_or_else(|| {
                worker::Error::RustError("Failed to load updated speaker".to_string())
            })?;

        Ok(updated)
    }

    /// Delete a speaker.
    pub async fn delete_speaker(
        &self,
        event_slug: &str,
        speaker_id: &str,
    ) -> Result<(), DeleteSpeakerError> {
        let existing = self.list_speakers(event_slug).await?;
        let target = existing
            .iter()
            .find(|s| s.id == speaker_id)
            .ok_or(DeleteSpeakerError::NotFound)?;
        let removed_order = target.display_order;

        self.db
            .prepare("DELETE FROM event_speakers WHERE event_slug = ? AND id = ?")
            .bind(&[JsValue::from_str(event_slug), JsValue::from_str(speaker_id)])?
            .run()
            .await?;

        // Re-compact subsequent display orders
        let shift_sql = r#"
            UPDATE event_speakers
            SET display_order = display_order - 1,
                updated_at = datetime('now')
            WHERE event_slug = ? AND display_order > ?
        "#;
        self.db
            .prepare(shift_sql)
            .bind(&[
                JsValue::from_str(event_slug),
                JsValue::from_f64(removed_order as f64),
            ])?
            .run()
            .await?;

        Ok(())
    }

    /// Reorder speakers for an event.
    pub async fn reorder_speakers(
        &self,
        event_slug: &str,
        items: &[EventSpeakerOrderItem],
    ) -> Result<Vec<EventSpeaker>, ReorderSpeakersError> {
        let existing = self.list_speakers(event_slug).await?;
        if items.len() != existing.len() {
            return Err(ReorderSpeakersError::InvalidList);
        }

        let mut existing_ids: Vec<String> = existing.iter().map(|s| s.id.clone()).collect();
        existing_ids.sort();
        let mut item_ids: Vec<String> = items.iter().map(|i| i.id.clone()).collect();
        item_ids.sort();
        if existing_ids != item_ids {
            return Err(ReorderSpeakersError::InvalidList);
        }

        // 2-phase update to avoid unique constraint collisions
        for item in items {
            let temp_sql =
                "UPDATE event_speakers SET display_order = -? WHERE event_slug = ? AND id = ?";
            self.db
                .prepare(temp_sql)
                .bind(&[
                    JsValue::from_f64(item.display_order as f64),
                    JsValue::from_str(event_slug),
                    JsValue::from_str(&item.id),
                ])?
                .run()
                .await?;
        }

        for item in items {
            let final_sql = "UPDATE event_speakers SET display_order = ?, updated_at = datetime('now') WHERE event_slug = ? AND id = ?";
            self.db
                .prepare(final_sql)
                .bind(&[
                    JsValue::from_f64(item.display_order as f64),
                    JsValue::from_str(event_slug),
                    JsValue::from_str(&item.id),
                ])?
                .run()
                .await?;
        }

        self.list_speakers(event_slug).await.map_err(Into::into)
    }
}

fn slugify(input: &str) -> String {
    let mut out = String::new();
    let mut last_dash = true;
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    if out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        "speaker".to_string()
    } else {
        out
    }
}

fn generate_unique_id<F>(base: &str, exists: F) -> String
where
    F: Fn(&str) -> bool,
{
    if !exists(base) {
        return base.to_string();
    }
    for i in 2..100 {
        let candidate = format!("{base}-{i}");
        if !exists(&candidate) {
            return candidate;
        }
    }
    format!("{}-{}", base, (js_sys::Date::now() / 1000.0) as u64)
}
