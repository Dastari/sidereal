use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthClaims {
    pub sub: String,
    pub player_entity_id: String,
    #[serde(default)]
    pub roles: Vec<String>,
    #[serde(default)]
    pub scope: String,
    #[serde(default)]
    pub session_context: AuthSessionContext,
    pub iat: u64,
    pub exp: u64,
    pub jti: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AuthSessionContext {
    #[serde(default)]
    pub auth_method: String,
    #[serde(default)]
    pub mfa_verified: bool,
    #[serde(default)]
    pub mfa_methods: Vec<String>,
    #[serde(default)]
    pub active_scope: Vec<String>,
    #[serde(default)]
    pub active_character_id: Option<String>,
}
