use serde::{de, Deserialize, Deserializer, Serialize};

/// D1 stores booleans as 0/1 integers. This deserializer accepts bool, int, or float.
pub fn deserialize_d1_bool<'de, D>(deserializer: D) -> Result<bool, D::Error>
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

/// Singleton row from the `link_page` table.
///
/// `rename_all = "camelCase"` exposes canonical JSON shape to API clients;
/// per-field `alias` keeps SQLite snake_case column names deserializable.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkPage {
    pub title: String,
    pub bio: Option<String>,
    #[serde(alias = "avatar_url")]
    pub avatar_url: Option<String>,
    pub background: String,
    #[serde(alias = "button_style")]
    pub button_style: String,
    #[serde(alias = "updated_at")]
    pub updated_at: String,
}

/// Row from the `link_items` table.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkItem {
    pub id: String,
    pub label: String,
    pub url: String,
    pub icon: Option<String>,
    #[serde(alias = "is_enabled", deserialize_with = "deserialize_d1_bool")]
    pub is_enabled: bool,
    #[serde(alias = "display_order")]
    pub display_order: i32,
    #[serde(alias = "created_at")]
    pub created_at: String,
    #[serde(alias = "updated_at")]
    pub updated_at: String,
}

/// PUT /api/admin/links/page body.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageUpdate {
    pub title: String,
    pub bio: Option<String>,
    pub avatar_url: Option<String>,
    pub background: String,
    pub button_style: String,
}

/// POST /api/admin/links/items body.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkCreate {
    pub label: String,
    pub url: String,
    pub icon: Option<String>,
    pub is_enabled: Option<bool>,
}

/// PUT /api/admin/links/items/:id body.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkUpdate {
    pub label: String,
    pub url: String,
    pub icon: Option<String>,
    pub is_enabled: Option<bool>,
}

/// PUT /api/admin/links/order body.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReorderRequest {
    pub ids: Vec<String>,
}
