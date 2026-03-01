use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use uuid::Uuid;

pub const BOOTSTRAP_KIND: &str = "bootstrap_player";
pub const AUTH_CHARACTERS_TABLE: &str = "auth_characters";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootstrapWireMessage {
    pub kind: String,
    pub account_id: String,
    pub player_entity_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BootstrapCommand {
    pub account_id: Uuid,
    pub player_entity_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BootstrapWireError {
    Validation(String),
}

impl Display for BootstrapWireError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Validation(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for BootstrapWireError {}

impl TryFrom<BootstrapWireMessage> for BootstrapCommand {
    type Error = BootstrapWireError;

    fn try_from(value: BootstrapWireMessage) -> Result<Self, Self::Error> {
        if value.kind != BOOTSTRAP_KIND {
            return Err(BootstrapWireError::Validation(format!(
                "unknown bootstrap kind: {}",
                value.kind
            )));
        }
        let account_id = Uuid::parse_str(&value.account_id)
            .map_err(|_| BootstrapWireError::Validation("invalid account_id uuid".to_string()))?;
        let is_bare_uuid = Uuid::parse_str(value.player_entity_id.trim()).is_ok();
        let is_legacy_prefixed = value.player_entity_id.starts_with("player:")
            && value.player_entity_id.trim().len() > "player:".len();
        if !is_bare_uuid && !is_legacy_prefixed {
            return Err(BootstrapWireError::Validation(
                "player_entity_id must be a valid UUID or legacy player:<uuid> value".to_string(),
            ));
        }

        Ok(Self {
            account_id,
            player_entity_id: value.player_entity_id,
        })
    }
}
