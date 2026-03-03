use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub account_id: Uuid,
    pub email: String,
    pub password_hash: String,
    pub player_entity_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthMe {
    pub account_id: Uuid,
    pub email: String,
    pub player_entity_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountCharacter {
    pub player_entity_id: String,
}

#[derive(Debug, Clone)]
pub struct RefreshTokenRecord {
    pub account_id: Uuid,
    pub expires_at_epoch_s: u64,
}

#[derive(Debug, Clone)]
pub struct PasswordResetTokenRecord {
    pub account_id: Uuid,
    pub expires_at_epoch_s: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PasswordResetRequestResult {
    pub accepted: bool,
    pub reset_token: Option<String>,
}
