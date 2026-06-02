use worker::Env;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub formbricks_base_url: String,
    pub formbricks_api_key: String,
    pub formbricks_webhook_secret: String,
    pub allowed_origins: String,
    pub google_client_id: String,
    pub enable_debug_auth: bool,
}

impl AppConfig {
    pub fn from_env(env: &Env) -> Result<Self, worker::Error> {
        let formbricks_base_url = env.var("FORMBRICKS_BASE_URL")?.to_string();
        let formbricks_api_key = env.secret("FORMBRICKS_API_KEY")?.to_string();
        let formbricks_webhook_secret = env
            .secret("FORMBRICKS_WEBHOOK_SECRET")
            .map(|s| s.to_string())
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
        Ok(Self {
            formbricks_base_url,
            formbricks_api_key,
            formbricks_webhook_secret,
            allowed_origins,
            google_client_id,
            enable_debug_auth,
        })
    }
}
