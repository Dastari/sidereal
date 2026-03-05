use serde::{Deserialize, Serialize};
use serde_json::{Map as JsonMap, Value as JsonValue};
use std::fmt::{Display, Formatter};
use uuid::Uuid;

pub const BOOTSTRAP_KIND: &str = "bootstrap_player";
pub const ADMIN_SPAWN_ENTITY_KIND: &str = "admin_spawn_entity";
pub const AUTH_CHARACTERS_TABLE: &str = "auth_characters";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum BootstrapWireMessage {
    #[serde(rename = "bootstrap_player")]
    BootstrapPlayer {
        account_id: String,
        player_entity_id: String,
    },
    #[serde(rename = "admin_spawn_entity")]
    AdminSpawnEntity {
        actor_account_id: String,
        actor_player_entity_id: String,
        request_id: String,
        player_entity_id: String,
        bundle_id: String,
        requested_entity_id: String,
        #[serde(default)]
        overrides: JsonMap<String, JsonValue>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BootstrapCommand {
    pub account_id: Uuid,
    pub player_entity_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdminSpawnEntityCommand {
    pub actor_account_id: Uuid,
    pub actor_player_entity_id: String,
    pub request_id: Uuid,
    pub player_entity_id: String,
    pub bundle_id: String,
    pub requested_entity_id: String,
    pub overrides: JsonMap<String, JsonValue>,
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
        let BootstrapWireMessage::BootstrapPlayer {
            account_id,
            player_entity_id,
        } = value
        else {
            return Err(BootstrapWireError::Validation(
                "wire message is not bootstrap_player".to_string(),
            ));
        };
        let account_id = Uuid::parse_str(&account_id)
            .map_err(|_| BootstrapWireError::Validation("invalid account_id uuid".to_string()))?;
        let trimmed_player_id = player_entity_id.trim();
        let Ok(player_entity_id) = Uuid::parse_str(trimmed_player_id) else {
            return Err(BootstrapWireError::Validation(
                "player_entity_id must be a valid UUID".to_string(),
            ));
        };

        Ok(Self {
            account_id,
            player_entity_id: player_entity_id.to_string(),
        })
    }
}

impl TryFrom<BootstrapWireMessage> for AdminSpawnEntityCommand {
    type Error = BootstrapWireError;

    fn try_from(value: BootstrapWireMessage) -> Result<Self, Self::Error> {
        let BootstrapWireMessage::AdminSpawnEntity {
            actor_account_id,
            actor_player_entity_id,
            request_id,
            player_entity_id,
            bundle_id,
            requested_entity_id,
            overrides,
        } = value
        else {
            return Err(BootstrapWireError::Validation(
                "wire message is not admin_spawn_entity".to_string(),
            ));
        };

        let actor_account_id = Uuid::parse_str(actor_account_id.trim()).map_err(|_| {
            BootstrapWireError::Validation("actor_account_id must be a valid UUID".to_string())
        })?;
        let actor_player_entity_id =
            Uuid::parse_str(actor_player_entity_id.trim()).map_err(|_| {
                BootstrapWireError::Validation(
                    "actor_player_entity_id must be a valid UUID".to_string(),
                )
            })?;
        let request_id = Uuid::parse_str(request_id.trim()).map_err(|_| {
            BootstrapWireError::Validation("request_id must be a valid UUID".to_string())
        })?;
        let player_entity_id = Uuid::parse_str(player_entity_id.trim()).map_err(|_| {
            BootstrapWireError::Validation("player_entity_id must be a valid UUID".to_string())
        })?;
        let requested_entity_id = Uuid::parse_str(requested_entity_id.trim()).map_err(|_| {
            BootstrapWireError::Validation("requested_entity_id must be a valid UUID".to_string())
        })?;
        let normalized_bundle_id = bundle_id.trim();
        if normalized_bundle_id.is_empty() {
            return Err(BootstrapWireError::Validation(
                "bundle_id must not be empty".to_string(),
            ));
        }

        Ok(Self {
            actor_account_id,
            actor_player_entity_id: actor_player_entity_id.to_string(),
            request_id,
            player_entity_id: player_entity_id.to_string(),
            bundle_id: normalized_bundle_id.to_string(),
            requested_entity_id: requested_entity_id.to_string(),
            overrides,
        })
    }
}
