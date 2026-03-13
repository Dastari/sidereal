use serde::{Deserialize, Serialize};
use serde_json::{Map as JsonMap, Value as JsonValue};
use sidereal_audio::AudioRegistry;

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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ReplicationTransportConfig {
    pub udp_addr: Option<String>,
    pub webtransport_addr: Option<String>,
    pub webtransport_certificate_sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnterWorldResponse {
    pub accepted: bool,
    #[serde(default)]
    pub replication_transport: ReplicationTransportConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetBootstrapManifestEntry {
    pub asset_id: String,
    pub asset_guid: String,
    pub shader_family: Option<String>,
    pub dependencies: Vec<String>,
    pub sha256_hex: String,
    pub relative_cache_path: String,
    pub content_type: String,
    pub byte_len: u64,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetBootstrapManifestResponse {
    pub catalog_version: String,
    pub audio_catalog_version: String,
    pub required_assets: Vec<AssetBootstrapManifestEntry>,
    pub catalog: Vec<AssetBootstrapManifestEntry>,
    pub audio_catalog: AudioRegistry,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminSpawnEntityRequest {
    pub player_entity_id: String,
    pub bundle_id: String,
    #[serde(default)]
    pub overrides: JsonMap<String, JsonValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminSpawnEntityResponse {
    pub ok: bool,
    pub spawned_entity_id: String,
    pub bundle_id: String,
    pub owner_player_entity_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptCatalogDocumentSummaryDto {
    pub script_path: String,
    pub family: String,
    pub active_revision: Option<u64>,
    pub has_draft: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListScriptsResponse {
    pub scripts: Vec<ScriptCatalogDocumentSummaryDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptCatalogDocumentDetailDto {
    pub script_path: String,
    pub family: String,
    pub active_revision: Option<u64>,
    pub active_source: Option<String>,
    pub active_origin: Option<String>,
    pub draft_source: Option<String>,
    pub draft_origin: Option<String>,
    pub draft_updated_at_epoch_s: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveScriptDraftRequest {
    pub source: String,
    pub origin: Option<String>,
    pub family: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveScriptDraftResponse {
    pub ok: bool,
    pub script_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishScriptResponse {
    pub ok: bool,
    pub script_path: String,
    pub published_revision: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscardScriptDraftResponse {
    pub ok: bool,
    pub script_path: String,
    pub discarded: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReloadScriptsFromDiskResponse {
    pub ok: bool,
    pub script_count: usize,
}
