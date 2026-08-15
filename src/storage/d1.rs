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
    #[allow(dead_code)]
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

/// Represents a row from the profiles table.
/// Internal model: `links_json` stays a raw string; API layers parse it via
/// `Profile::links()` so it never leaks as an opaque string to clients.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub normalized_email: String,
    pub name: Option<String>,
    pub picture: Option<String>,
    pub updated_at: String,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default = "default_links_json")]
    pub links_json: String,
    #[serde(default, deserialize_with = "deserialize_d1_bool_opt")]
    pub is_public: bool,
    #[serde(default)]
    pub profile_updated_at: Option<String>,
}

fn default_links_json() -> String {
    "[]".to_string()
}

fn deserialize_d1_bool_opt<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    // D1 sends booleans as SQLite integers (0/1); accept bool too.
    let v = Option::<serde_json::Value>::deserialize(deserializer)?;
    match v {
        None | Some(serde_json::Value::Null) => Ok(false),
        Some(serde_json::Value::Bool(b)) => Ok(b),
        Some(serde_json::Value::Number(n)) => Ok(n.as_f64().unwrap_or(0.0) != 0.0),
        Some(other) => Err(de::Error::custom(format!("invalid boolean value: {other}"))),
    }
}

impl Profile {
    /// Parsed typed links; corrupt JSON degrades to an empty list.
    pub fn links(&self) -> Vec<crate::validation::profile::ProfileLink> {
        crate::validation::profile::parse_links_json(&self.links_json)
    }
}

pub struct ProfileRepository;

impl ProfileRepository {
    /// Upsert a user profile snapshot into the profiles table.
    pub async fn upsert_profile(
        db: &D1Database,
        normalized_email: &str,
        name: Option<&str>,
        picture: Option<&str>,
    ) -> WorkerResult<()> {
        let sql = r#"
            INSERT INTO profiles (normalized_email, name, picture, updated_at)
            VALUES (?, ?, ?, datetime('now'))
            ON CONFLICT(normalized_email) DO UPDATE SET
                name = COALESCE(excluded.name, profiles.name),
                picture = COALESCE(excluded.picture, profiles.picture),
                updated_at = datetime('now')
        "#;

        let email_val = JsValue::from_str(normalized_email);
        let name_val = match name {
            Some(n) if !n.trim().is_empty() => JsValue::from_str(n),
            _ => JsValue::null(),
        };
        let picture_val = match picture {
            Some(p) if !p.trim().is_empty() => JsValue::from_str(p),
            _ => JsValue::null(),
        };

        db.prepare(sql)
            .bind(&[email_val, name_val, picture_val])?
            .run()
            .await?;

        Ok(())
    }

    /// Get a single profile row by normalized email.
    pub async fn get_profile_by_email(
        db: &D1Database,
        normalized_email: &str,
    ) -> WorkerResult<Option<Profile>> {
        let sql = "SELECT normalized_email, name, picture, updated_at, display_name, title, links_json, is_public, profile_updated_at FROM profiles WHERE normalized_email = ?";
        db.prepare(sql)
            .bind(&[JsValue::from_str(normalized_email)])?
            .first::<Profile>(None)
            .await
    }

    /// Replace the user-owned editable fields atomically.
    ///
    /// Upsert keyed on the verified token email. Google-owned `name`/`picture`
    /// are only set on insert (first sight); the conflict branch never touches
    /// them, and it also never resets `profile_updated_at` semantics: only this
    /// method (an intentional edit) updates that timestamp.
    pub async fn update_profile_details(
        db: &D1Database,
        normalized_email: &str,
        display_name: Option<&str>,
        title: Option<&str>,
        links_json: &str,
        is_public: bool,
    ) -> WorkerResult<()> {
        let sql = r#"
            INSERT INTO profiles (
                normalized_email, name, picture, updated_at,
                display_name, title, links_json, is_public, profile_updated_at
            ) VALUES (?, NULL, NULL, datetime('now'), ?, ?, ?, ?, datetime('now'))
            ON CONFLICT(normalized_email) DO UPDATE SET
                display_name = excluded.display_name,
                title = excluded.title,
                links_json = excluded.links_json,
                is_public = excluded.is_public,
                profile_updated_at = datetime('now')
        "#;

        db.prepare(sql)
            .bind(&[
                JsValue::from_str(normalized_email),
                match display_name {
                    Some(v) => JsValue::from_str(v),
                    None => JsValue::null(),
                },
                match title {
                    Some(v) => JsValue::from_str(v),
                    None => JsValue::null(),
                },
                JsValue::from_str(links_json),
                JsValue::from_bool(is_public),
            ])?
            .run()
            .await?;

        Ok(())
    }

    /// Look up published profiles by a list of normalized emails.
    /// Only `is_public = 1` rows are returned; passively-created Google
    /// snapshots never leak through public lookup.
    pub async fn lookup_profiles(
        db: &D1Database,
        normalized_emails: &[String],
    ) -> WorkerResult<Vec<Profile>> {
        if normalized_emails.is_empty() {
            return Ok(Vec::new());
        }

        let placeholders = vec!["?"; normalized_emails.len()].join(", ");
        let sql = [
            "SELECT normalized_email, name, picture, updated_at, display_name, title, ",
            "links_json, is_public, profile_updated_at FROM profiles ",
            "WHERE is_public = 1 AND normalized_email IN ",
            "(",
            &placeholders,
            ")",
        ]
        .concat();

        let bind_values: Vec<JsValue> = normalized_emails
            .iter()
            .map(|e| JsValue::from_str(e))
            .collect();

        let result = db.prepare(&sql).bind(&bind_values)?.all().await?;

        result.results::<Profile>()
    }
}
