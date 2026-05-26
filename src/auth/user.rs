use serde::{Deserialize, Serialize};

/// Represents an authenticated user extracted from request headers/tokens.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthUser {
    pub sub: String,
    pub email: String,
    pub name: Option<String>,
    pub picture: Option<String>,
}

impl AuthUser {
    /// Returns the normalized email for comparison.
    pub fn normalized_email(&self) -> String {
        self.email.trim().to_lowercase()
    }
}
