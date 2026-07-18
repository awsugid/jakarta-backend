use worker::Env;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub formbricks_base_url: String,
    pub formbricks_api_key: String,
    pub formbricks_webhook_secret: String,
    pub pretix_base_url: String,
    pub pretix_api_token: String,
    pub pretix_default_organizer: String,
    pub allowed_origins: String,
    pub google_client_id: String,
    pub enable_debug_auth: bool,
    pub admin_emails: Vec<String>,
    pub company_exclusion_keywords: Vec<String>,
}

impl AppConfig {
    pub fn from_env(env: &Env) -> Result<Self, worker::Error> {
        let formbricks_base_url = env.var("FORMBRICKS_BASE_URL")?.to_string();
        let formbricks_api_key = env.secret("FORMBRICKS_API_KEY")?.to_string();
        let formbricks_webhook_secret = env
            .secret("FORMBRICKS_WEBHOOK_SECRET")
            .map(|s| s.to_string())
            .unwrap_or_default();
        let pretix_base_url = env.var("PRETIX_API_BASE_URL")?.to_string();
        let pretix_api_token = env.secret("PRETIX_API_TOKEN")?.to_string();
        let pretix_default_organizer = env
            .var("PRETIX_DEFAULT_ORGANIZER")
            .map(|v| v.to_string())
            .unwrap_or_default();
        let allowed_origins = env
            .var("ALLOWED_ORIGINS")
            .map(|v| v.to_string())
            .unwrap_or_else(|_| "*".to_string());
        let google_client_id = env
            .var("GOOGLE_CLIENT_ID")
            .map(|v| v.to_string())
            .unwrap_or_default();
        let enable_debug_auth = env
            .var("ENABLE_DEBUG_AUTH")
            .map(|v| v.to_string() == "true")
            .unwrap_or(false);
        let admin_emails = env
            .var("ADMIN_EMAILS")
            .map(|v| parse_admin_emails(&v.to_string()))
            .unwrap_or_default();
        let company_exclusion_keywords = env
            .var("COMPANY_EXCLUSION_KEYWORDS")
            .map(|v| parse_keywords(&v.to_string()))
            .unwrap_or_else(|_| default_company_exclusion_keywords());
        Ok(Self {
            formbricks_base_url,
            formbricks_api_key,
            formbricks_webhook_secret,
            pretix_base_url,
            pretix_api_token,
            pretix_default_organizer,
            allowed_origins,
            google_client_id,
            enable_debug_auth,
            admin_emails,
            company_exclusion_keywords,
        })
    }

    /// True if the given email is in the configured admin allow-list.
    /// Comparison is case-insensitive after trim+lowercase.
    pub fn is_admin(&self, email: &str) -> bool {
        let normalized = email.trim().to_lowercase();
        self.admin_emails.iter().any(|e| e == &normalized)
    }
}

/// Parse a comma-separated ADMIN_EMAILS value into a lowercased, trimmed Vec.
fn parse_admin_emails(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Parse a comma-separated keyword list into a lowercased, trimmed Vec.
fn parse_keywords(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Default exclusion keywords for company filter (used when env var unset).
/// Catches universities, schools, and personal/student entries across EN + ID.
fn default_company_exclusion_keywords() -> Vec<String> {
    [
        "univ",
        "college",
        "school",
        "sekolah",
        "akademi",
        "politeknik",
        "poltek",
        "institute",
        "institut",
        "sma",
        "smk",
        "smkn",
        "mts",
        "stt",
        "personal",
        "imam",
        "binus",
    ]
    .into_iter()
    .map(String::from)
    .collect()
}
