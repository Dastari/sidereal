use serde::{Deserialize, Serialize};
use sidereal_asset_runtime::AssetCatalogEntry;
use sidereal_game::EntityAction;

/// Client authenticates replication session and binds transport identity.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClientAuthMessage {
    pub player_entity_id: String,
    pub access_token: String,
}

/// Server acknowledges that replication auth/session binding is ready for the selected player.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServerSessionReadyMessage {
    pub player_entity_id: String,
}

/// Client notifies server that it is disconnecting (logout or window close).
/// Server should Unlink the client immediately so it stops sending.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClientDisconnectNotifyMessage {
    pub player_entity_id: String,
}

/// Client requests an authoritative control-target change.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClientControlRequestMessage {
    pub player_entity_id: String,
    pub controlled_entity_id: Option<String>,
    pub request_seq: u64,
}

/// Server acknowledges an authoritative control-target change request.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServerControlAckMessage {
    pub player_entity_id: String,
    pub request_seq: u64,
    pub controlled_entity_id: Option<String>,
}

/// Server rejects an authoritative control-target change request.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServerControlRejectMessage {
    pub player_entity_id: String,
    pub request_seq: u64,
    pub reason: String,
    pub authoritative_controlled_entity_id: Option<String>,
}

/// Latest-wins realtime control intent from client to authoritative server.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClientRealtimeInputMessage {
    pub player_entity_id: String,
    pub controlled_entity_id: String,
    pub actions: Vec<EntityAction>,
    pub tick: u64,
}

/// Client requests one or more assets by known version/checksum.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AssetRequestMessage {
    pub requests: Vec<RequestedAsset>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RequestedAsset {
    pub asset_id: String,
    pub known_asset_version: Option<u64>,
    pub known_sha256_hex: Option<String>,
}

/// Client acknowledges completed asset assembly/write.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AssetAckMessage {
    pub asset_id: String,
    pub asset_version: u64,
    pub sha256_hex: String,
}

/// Reliable manifest for replication-delivered streamed assets.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AssetStreamManifestMessage {
    pub assets: Vec<AssetStreamEntry>,
}

pub type AssetStreamEntry = AssetCatalogEntry;

/// Reliable asset chunk payload sent on the control channel.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AssetStreamChunkMessage {
    pub asset_id: String,
    pub relative_cache_path: String,
    pub chunk_index: u32,
    pub chunk_count: u32,
    pub bytes: Vec<u8>,
}
