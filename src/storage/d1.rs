use serde::{de, Deserialize, Deserializer, Serialize};
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
    #[serde(deserialize_with = "deserialize_d1_bool")]
    pub is_active: bool,
    pub opens_at: Option<String>,
    pub closes_at: Option<String>,
    pub editable_until: Option<String>,
    pub archive_after: Option<String>,
    pub display_order: i32,
    pub created_at: String,
    pub updated_at: String,
}

fn deserialize_d1_bool<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    struct D1BoolVisitor;

    impl<'de> de::Visitor<'de> for D1BoolVisitor {
        type Value = bool;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a boolean or SQLite numeric boolean")
        }

        fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
            Ok(value)
        }

        fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(value != 0)
        }

        fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(value != 0)
        }

        fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(value != 0.0)
        }
    }

    deserializer.deserialize_any(D1BoolVisitor)
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

    /// Get a form by its FormBricks survey ID.
    #[allow(dead_code)]
    pub async fn get_form_by_survey_id(
        &self,
        survey_id: &str,
    ) -> WorkerResult<Option<ApplicationForm>> {
        let sql = "SELECT * FROM application_forms WHERE formbricks_survey_id = ?";
        self.db
            .prepare(sql)
            .bind(&[JsValue::from_str(survey_id)])?
            .first::<ApplicationForm>(None)
            .await
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

/// Represents a row from the application_response_index table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplicationResponseIndex {
    pub id: String,
    pub form_id: String,
    pub formbricks_survey_id: String,
    pub formbricks_response_id: String,
    pub normalized_email: String,
    pub normalized_linkedin_url: Option<String>,
    #[serde(deserialize_with = "deserialize_d1_bool")]
    pub finished: bool,
    pub status: String,
    pub submitted_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl FormRepository {
    /// Get active response index by form ID and normalized email
    pub async fn get_index_by_form_email(
        &self,
        form_id: &str,
        normalized_email: &str,
    ) -> WorkerResult<Option<ApplicationResponseIndex>> {
        let sql = "SELECT * FROM application_response_index WHERE form_id = ? AND normalized_email = ? AND status = 'active' ORDER BY created_at DESC LIMIT 1";
        self.db
            .prepare(sql)
            .bind(&[
                JsValue::from_str(form_id),
                JsValue::from_str(normalized_email),
            ])?
            .first::<ApplicationResponseIndex>(None)
            .await
    }

    /// List active response indexes by normalized email
    pub async fn list_indexes_by_email(
        &self,
        normalized_email: &str,
    ) -> WorkerResult<Vec<ApplicationResponseIndex>> {
        let sql = "SELECT * FROM application_response_index WHERE normalized_email = ? AND status = 'active'";
        let result = self
            .db
            .prepare(sql)
            .bind(&[JsValue::from_str(normalized_email)])?
            .all()
            .await?;
        result.results::<ApplicationResponseIndex>()
    }

    /// Get active response index by form ID and normalized linkedin url
    pub async fn get_index_by_form_linkedin(
        &self,
        form_id: &str,
        normalized_linkedin_url: &str,
    ) -> WorkerResult<Option<ApplicationResponseIndex>> {
        let sql = "SELECT * FROM application_response_index WHERE form_id = ? AND normalized_linkedin_url = ? AND status = 'active' ORDER BY created_at DESC LIMIT 1";
        self.db
            .prepare(sql)
            .bind(&[
                JsValue::from_str(form_id),
                JsValue::from_str(normalized_linkedin_url),
            ])?
            .first::<ApplicationResponseIndex>(None)
            .await
    }

    /// Upsert an active response into the index
    pub async fn upsert_active_response_index(
        &self,
        index: &ApplicationResponseIndex,
    ) -> WorkerResult<()> {
        let sql = r#"
            INSERT INTO application_response_index (
                id, form_id, formbricks_survey_id, formbricks_response_id,
                normalized_email, normalized_linkedin_url, finished, status, submitted_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(form_id, formbricks_response_id) DO UPDATE SET
                normalized_email = excluded.normalized_email,
                normalized_linkedin_url = excluded.normalized_linkedin_url,
                finished = excluded.finished,
                status = excluded.status,
                submitted_at = excluded.submitted_at,
                updated_at = datetime('now')
        "#;

        let id = JsValue::from_str(&index.id);
        let form_id = JsValue::from_str(&index.form_id);
        let survey_id = JsValue::from_str(&index.formbricks_survey_id);
        let response_id = JsValue::from_str(&index.formbricks_response_id);
        let email = JsValue::from_str(&index.normalized_email);
        let linkedin = match &index.normalized_linkedin_url {
            Some(l) => JsValue::from_str(l),
            None => JsValue::null(),
        };
        let finished = JsValue::from_bool(index.finished);
        let status = JsValue::from_str(&index.status);
        let submitted_at = match &index.submitted_at {
            Some(s) => JsValue::from_str(s),
            None => JsValue::null(),
        };

        self.db
            .prepare(sql)
            .bind(&[
                id,
                form_id,
                survey_id,
                response_id,
                email,
                linkedin,
                finished,
                status,
                submitted_at,
            ])?
            .run()
            .await?;

        Ok(())
    }

    /// Update status of a response index (e.g. to 'duplicate_deleted')
    pub async fn mark_response_index_status(
        &self,
        formbricks_response_id: &str,
        status: &str,
    ) -> WorkerResult<()> {
        let sql = "UPDATE application_response_index SET status = ?, updated_at = datetime('now') WHERE formbricks_response_id = ?";
        self.db
            .prepare(sql)
            .bind(&[
                JsValue::from_str(status),
                JsValue::from_str(formbricks_response_id),
            ])?
            .run()
            .await?;
        Ok(())
    }

    /// Delete old response for same form and email if it exists (for edit flow)
    pub async fn delete_old_response_index(
        &self,
        form_id: &str,
        normalized_email: &str,
        current_response_id: &str,
    ) -> WorkerResult<()> {
        let sql = "UPDATE application_response_index SET status = 'deleted', updated_at = datetime('now') WHERE form_id = ? AND normalized_email = ? AND formbricks_response_id != ?";
        self.db
            .prepare(sql)
            .bind(&[
                JsValue::from_str(form_id),
                JsValue::from_str(normalized_email),
                JsValue::from_str(current_response_id),
            ])?
            .run()
            .await?;
        Ok(())
    }
}
