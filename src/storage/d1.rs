use serde::{Deserialize, Serialize};
use wasm_bindgen::JsValue;
use worker::{D1Database, Result as WorkerResult};

/// Represents a row from the application_forms table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplicationForm {
    pub id: String,
    pub kind: String,
    pub slug: String,
    pub title: String,
    pub description: Option<String>,
    pub formbricks_survey_id: String,
    pub formbricks_public_url: Option<String>,
    pub email_question_id: String,
    pub linkedin_question_id: String,
    pub is_active: bool,
    pub opens_at: Option<String>,
    pub closes_at: Option<String>,
    pub editable_until: Option<String>,
    pub archive_after: Option<String>,
    pub display_order: i32,
    pub created_at: String,
    pub updated_at: String,
}

pub struct FormRepository {
    db: D1Database,
}

impl FormRepository {
    pub fn new(db: D1Database) -> Self {
        Self { db }
    }

    /// List all active forms, optionally filtered by kind.
    pub async fn list_forms(&self, kind: Option<&str>) -> WorkerResult<Vec<ApplicationForm>> {
        let result = match kind {
            Some(k) => {
                let sql = "SELECT * FROM application_forms WHERE kind = ? AND is_active = 1 ORDER BY display_order, title";
                self.db
                    .prepare(sql)
                    .bind(&[JsValue::from_str(k)])?
                    .all()
                    .await?
            }
            None => {
                let sql = "SELECT * FROM application_forms WHERE is_active = 1 ORDER BY kind, display_order, title";
                self.db.prepare(sql).all().await?
            }
        };

        result.results::<ApplicationForm>()
    }

    /// Get a specific form by kind and slug.
    pub async fn get_form(&self, kind: &str, slug: &str) -> WorkerResult<Option<ApplicationForm>> {
        let sql = "SELECT * FROM application_forms WHERE kind = ? AND slug = ?";
        self.db
            .prepare(sql)
            .bind(&[JsValue::from_str(kind), JsValue::from_str(slug)])?
            .first::<ApplicationForm>(None)
            .await
    }
}
