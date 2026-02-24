use serde::{Deserialize, Serialize};
use sidereal_asset_runtime::AssetCatalogEntry;

/// Client authenticates replication session and binds transport identity.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClientAuthMessage {
    pub player_entity_id: String,
    pub access_token: String,
}

/// Client updates camera/focus runtime state for server-side persistence.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClientViewUpdateMessage {
    pub player_entity_id: String,
    pub focused_entity_id: Option<String>,
    pub selected_entity_id: Option<String>,
    pub controlled_entity_id: Option<String>,
    pub camera_position_m: [f32; 3],
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
