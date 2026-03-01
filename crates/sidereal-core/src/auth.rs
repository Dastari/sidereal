use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthClaims {
    pub sub: String,
    pub player_entity_id: String,
    pub iat: u64,
    pub exp: u64,
    pub jti: String,
}
