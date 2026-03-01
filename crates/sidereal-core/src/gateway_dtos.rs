use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthTokens {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in_s: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PasswordResetRequest {
    pub email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PasswordResetConfirmRequest {
    pub reset_token: String,
    pub new_password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PasswordResetResponse {
    pub accepted: bool,
    pub reset_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PasswordResetConfirmResponse {
    pub accepted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeResponse {
    pub account_id: String,
    pub email: String,
    pub player_entity_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterSummary {
    pub player_entity_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharactersResponse {
    pub characters: Vec<CharacterSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnterWorldRequest {
    pub player_entity_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnterWorldResponse {
    pub accepted: bool,
}
