use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub account_id: Uuid,
    pub email: String,
    pub password_hash: String,
    /// Legacy compatibility field. Account rows no longer define a default character.
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
    pub display_name: String,
    pub created_at_epoch_s: u64,
    pub status: String,
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

#[derive(Debug, Clone)]
pub struct EmailLoginChallengeRecord {
    pub challenge_id: Uuid,
    pub account_id: Uuid,
    pub expires_at_epoch_s: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailLoginRequestResult {
    pub accepted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailLoginVerifyResult {
    pub tokens: sidereal_core::gateway_dtos::AuthTokens,
}

#[derive(Debug, Clone)]
pub struct TotpEnrollmentRecord {
    pub enrollment_id: Uuid,
    pub account_id: Uuid,
    pub encrypted_secret: String,
    pub expires_at_epoch_s: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TotpEnrollmentResult {
    pub enrollment_id: Uuid,
    pub issuer: String,
    pub account_label: String,
    pub provisioning_uri: String,
    pub qr_svg: String,
    pub manual_secret: String,
    pub expires_in_s: u64,
}

#[derive(Debug, Clone)]
pub struct TotpLoginChallengeRecord {
    pub challenge_id: Uuid,
    pub account_id: Uuid,
    pub expires_at_epoch_s: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PasswordLoginResult {
    Authenticated {
        tokens: sidereal_core::gateway_dtos::AuthTokens,
    },
    TotpRequired {
        challenge_id: Uuid,
        expires_in_s: u64,
    },
}
