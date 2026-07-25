use std::collections::HashSet;

use serde::Deserialize;
use wasm_bindgen::{JsCast, JsValue};
use worker::d1::D1Result;
use worker::{D1Database, Result as WorkerResult};

use crate::links::types::{LinkCreate, LinkItem, LinkPage, LinkUpdate, PageUpdate};

pub struct LinkRepository {
    db: D1Database,
}

impl LinkRepository {
    pub fn new(db: D1Database) -> Self {
        Self { db }
    }

    /// Read the singleton `link_page` row. Internal error if the seed row is missing.
    pub async fn get_page(&self) -> WorkerResult<LinkPage> {
        let sql = "SELECT title, bio, avatar_url, background, button_style, updated_at FROM link_page WHERE id = 1";
        self.db
            .prepare(sql)
            .first::<LinkPage>(None)
            .await?
            .ok_or_else(|| worker::Error::RustError("link_page singleton row missing".into()))
    }

    /// List link items ordered by display_order then created_at.
    pub async fn get_items(&self, enabled_only: bool) -> WorkerResult<Vec<LinkItem>> {
        let sql = if enabled_only {
            "SELECT id, label, url, icon, is_enabled, display_order, created_at, updated_at FROM link_items WHERE is_enabled = 1 ORDER BY display_order, created_at"
        } else {
            "SELECT id, label, url, icon, is_enabled, display_order, created_at, updated_at FROM link_items ORDER BY display_order, created_at"
        };
        let result = self.db.prepare(sql).all().await?;
        result.results::<LinkItem>()
    }

    /// Update the singleton page profile, then return the refreshed row.
    pub async fn update_page(&self, input: &PageUpdate) -> WorkerResult<LinkPage> {
        let sql = r#"
            UPDATE link_page
            SET title = ?,
                bio = ?,
                avatar_url = ?,
                background = ?,
                button_style = ?,
                updated_at = datetime('now')
            WHERE id = 1
        "#;
        let title = JsValue::from_str(&input.title);
        let bio = input
            .bio
            .as_deref()
            .map_or_else(JsValue::null, JsValue::from_str);
        let avatar_url = input
            .avatar_url
            .as_deref()
            .map_or_else(JsValue::null, JsValue::from_str);
        let background = JsValue::from_str(&input.background);
        let button_style = JsValue::from_str(&input.button_style);

        self.db
            .prepare(sql)
            .bind(&[title, bio, avatar_url, background, button_style])?
            .run()
            .await?;

        self.get_page().await
    }

    /// Create a link. display_order is computed as MAX(display_order) + 1.
    pub async fn create_link(&self, input: &LinkCreate) -> WorkerResult<LinkItem> {
        let id = gen_id();
        let next_order = self.next_display_order().await?;

        let sql = r#"
            INSERT INTO link_items (id, label, url, icon, is_enabled, display_order)
            VALUES (?, ?, ?, ?, ?, ?)
        "#;
        let id_val = JsValue::from_str(&id);
        let label = JsValue::from_str(&input.label);
        let url = JsValue::from_str(&input.url);
        let icon = input
            .icon
            .as_deref()
            .map_or_else(JsValue::null, JsValue::from_str);
        let is_enabled = JsValue::from_bool(input.is_enabled.unwrap_or(true));
        let display_order = JsValue::from_f64(next_order as f64);

        self.db
            .prepare(sql)
            .bind(&[id_val, label, url, icon, is_enabled, display_order])?
            .run()
            .await?;

        self.get_link_by_id(&id)
            .await?
            .ok_or_else(|| worker::Error::RustError("inserted link not found after insert".into()))
    }

    /// Update a link by id. Returns None if no row was affected.
    pub async fn update_link(
        &self,
        id: &str,
        input: &LinkUpdate,
    ) -> WorkerResult<Option<LinkItem>> {
        let sql = r#"
            UPDATE link_items
            SET label = ?,
                url = ?,
                icon = ?,
                is_enabled = ?,
                updated_at = datetime('now')
            WHERE id = ?
        "#;
        let label = JsValue::from_str(&input.label);
        let url = JsValue::from_str(&input.url);
        let icon = input
            .icon
            .as_deref()
            .map_or_else(JsValue::null, JsValue::from_str);
        let is_enabled = JsValue::from_bool(input.is_enabled.unwrap_or(true));
        let id_val = JsValue::from_str(id);

        let result = self
            .db
            .prepare(sql)
            .bind(&[label, url, icon, is_enabled, id_val])?
            .run()
            .await?;

        if changed_rows(&result)? == 0 {
            return Ok(None);
        }
        self.get_link_by_id(id).await
    }

    /// Delete a link by id. Returns false if no row was deleted.
    pub async fn delete_link(&self, id: &str) -> WorkerResult<bool> {
        let sql = "DELETE FROM link_items WHERE id = ?";
        let result = self
            .db
            .prepare(sql)
            .bind(&[JsValue::from_str(id)])?
            .run()
            .await?;
        Ok(changed_rows(&result)? > 0)
    }

    /// Reorder link items to match the provided id sequence.
    ///
    /// Count is capped at 200. After the batched UPDATE, confirms every
    /// supplied id is present in the refreshed listing.
    pub async fn reorder(&self, ids: &[String]) -> WorkerResult<Vec<LinkItem>> {
        if ids.len() > 200 {
            return Err(worker::Error::RustError(format!(
                "reorder count {} exceeds limit of 200",
                ids.len()
            )));
        }

        if !ids.is_empty() {
            let mut statements = Vec::with_capacity(ids.len());
            for (index, id) in ids.iter().enumerate() {
                let stmt = self
                    .db
                    .prepare("UPDATE link_items SET display_order = ? WHERE id = ?")
                    .bind(&[JsValue::from_f64((index + 1) as f64), JsValue::from_str(id)])?;
                statements.push(stmt);
            }
            self.db.batch(statements).await?;
        }

        let updated = self.get_items(false).await?;
        let present: HashSet<&str> = updated.iter().map(|item| item.id.as_str()).collect();
        for id in ids {
            if !present.contains(id.as_str()) {
                return Err(worker::Error::RustError(format!(
                    "reorder target id {id} not found after update"
                )));
            }
        }
        Ok(updated)
    }

    async fn get_link_by_id(&self, id: &str) -> WorkerResult<Option<LinkItem>> {
        let sql = "SELECT id, label, url, icon, is_enabled, display_order, created_at, updated_at FROM link_items WHERE id = ?";
        self.db
            .prepare(sql)
            .bind(&[JsValue::from_str(id)])?
            .first::<LinkItem>(None)
            .await
    }

    async fn next_display_order(&self) -> WorkerResult<i32> {
        let sql = "SELECT COALESCE(MAX(display_order), 0) AS max_order FROM link_items";
        let row: Option<MaxOrder> = self.db.prepare(sql).first::<MaxOrder>(None).await?;
        Ok(row.map(|r| r.max_order + 1).unwrap_or(1))
    }
}

#[derive(Deserialize)]
struct MaxOrder {
    max_order: i32,
}

/// Number of rows affected by the most recent D1 mutation.
fn changed_rows(result: &D1Result) -> WorkerResult<usize> {
    Ok(result.meta()?.and_then(|m| m.changes).unwrap_or(0))
}

/// Generate a UUID via `crypto.randomUUID()`, with a timestamp + random fallback.
fn gen_id() -> String {
    let global = js_sys::global();
    let crypto_key = JsValue::from("crypto");
    let crypto = match js_sys::Reflect::get(&global, &crypto_key) {
        Ok(v) => v,
        Err(_) => return fallback_id(),
    };
    let uuid_key = JsValue::from("randomUUID");
    let random_uuid = match js_sys::Reflect::get(&crypto, &uuid_key) {
        Ok(v) => v,
        Err(_) => return fallback_id(),
    };
    let f: js_sys::Function = match random_uuid.dyn_into() {
        Ok(f) => f,
        Err(_) => return fallback_id(),
    };
    match f.call0(&crypto) {
        Ok(v) => v.as_string().unwrap_or_else(fallback_id),
        Err(_) => fallback_id(),
    }
}

fn fallback_id() -> String {
    let now = js_sys::Date::now() as u64;
    format!("id-{now:x}-{}", (js_sys::Math::random() * 1e9_f64) as u64)
}
