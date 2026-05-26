use worker::Env;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub formbricks_base_url: String,
    pub formbricks_api_key: String,
    pub allowed_origins: String,
}

impl AppConfig {
    pub fn from_env(env: &Env) -> Result<Self, worker::Error> {
        let formbricks_base_url = env.var("FORMBRICKS_BASE_URL")?.to_string();
        let formbricks_api_key = env.secret("FORMBRICKS_API_KEY")?.to_string();
        let allowed_origins = env
            .var("ALLOWED_ORIGINS")
            .map(|v| v.to_string())
            .unwrap_or_else(|_| "*".to_string());
        Ok(Self {
            formbricks_base_url,
            formbricks_api_key,
            allowed_origins,
        })
    }
}
