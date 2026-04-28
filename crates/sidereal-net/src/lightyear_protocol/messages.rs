use serde::{Deserialize, Serialize};
use sidereal_game::EntityAction;

pub const LIGHTYEAR_PROTOCOL_VERSION: u32 = 7;

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
    pub protocol_version: u32,
    pub control_generation: u64,
    pub controlled_entity_id: Option<String>,
}

/// Server denies a replication session for the selected player.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServerSessionDeniedMessage {
    pub player_entity_id: String,
    pub reason: String,
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
    pub control_generation: u64,
    pub controlled_entity_id: Option<String>,
}

/// Server rejects an authoritative control-target change request.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServerControlRejectMessage {
    pub player_entity_id: String,
    pub request_seq: u64,
    pub control_generation: u64,
    pub reason: String,
    pub authoritative_controlled_entity_id: Option<String>,
}

/// Latest-wins realtime control intent from client to authoritative server.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClientRealtimeInputMessage {
    pub player_entity_id: String,
    pub controlled_entity_id: String,
    pub control_generation: u64,
    pub actions: Vec<EntityAction>,
    pub tick: u64,
}

/// Client local view mode used by replication relevance policy.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum ClientLocalViewMode {
    #[default]
    Tactical,
    Map,
}

/// Client informs server which view mode should drive delivery culling.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClientLocalViewModeMessage {
    pub player_entity_id: String,
    pub view_mode: ClientLocalViewMode,
    /// Client-observed delivery radius (meters) derived from current viewport/zoom.
    pub delivery_range_m: f32,
}

/// Client asks server to resend tactical snapshots when delta apply base mismatches
/// or when snapshots have timed out.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClientTacticalResnapshotRequestMessage {
    pub player_entity_id: String,
    pub request_fog_snapshot: bool,
    pub request_contacts_snapshot: bool,
}

/// Server authoritative weapon fire notification for client-side tracer visuals.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServerWeaponFiredMessage {
    pub shooter_entity_id: String,
    pub weapon_guid: String,
    pub audio_profile_id: Option<String>,
    pub cooldown_s: Option<f32>,
    pub origin_xy: [f64; 2],
    pub velocity_xy: [f64; 2],
    pub impact_xy: Option<[f64; 2]>,
    pub ttl_s: f32,
}

/// Server authoritative destruction-effect notification for pre-despawn VFX.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServerEntityDestructionMessage {
    pub entity_id: String,
    pub origin_xy: [f64; 2],
    pub destruction_profile_id: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct GridCell {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TacticalContact {
    pub entity_id: String,
    pub kind: String,
    pub map_icon_asset_id: Option<String>,
    pub faction_id: Option<String>,
    pub position_xy: [f64; 2],
    pub size_m: Option<[f32; 3]>,
    pub mass_kg: Option<f32>,
    pub heading_rad: f64,
    pub velocity_xy: Option<[f64; 2]>,
    pub is_live_now: bool,
    pub last_seen_tick: u64,
    pub classification: Option<String>,
    pub contact_quality: Option<String>,
    pub signal_strength: Option<f32>,
    pub position_accuracy_m: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OwnedAssetEntry {
    pub entity_id: String,
    pub display_name: String,
    pub kind: String,
    pub status: String,
    pub controlled_by_owner: bool,
    pub last_known_position_xy: Option<[f64; 2]>,
    pub health_ratio: Option<f32>,
    pub fuel_ratio: Option<f32>,
    pub updated_at_tick: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServerTacticalFogSnapshotMessage {
    pub player_entity_id: String,
    pub sequence: u64,
    pub cell_size_m: f32,
    pub explored_cells: Vec<GridCell>,
    pub live_cells: Vec<GridCell>,
    pub generated_at_tick: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServerTacticalFogDeltaMessage {
    pub player_entity_id: String,
    pub sequence: u64,
    pub base_sequence: u64,
    pub explored_cells_added: Vec<GridCell>,
    pub live_cells_added: Vec<GridCell>,
    pub live_cells_removed: Vec<GridCell>,
    pub generated_at_tick: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServerTacticalContactsSnapshotMessage {
    pub player_entity_id: String,
    pub sequence: u64,
    pub contacts: Vec<TacticalContact>,
    pub generated_at_tick: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServerTacticalContactsDeltaMessage {
    pub player_entity_id: String,
    pub sequence: u64,
    pub base_sequence: u64,
    pub upserts: Vec<TacticalContact>,
    pub removals: Vec<String>,
    pub generated_at_tick: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServerOwnerAssetManifestSnapshotMessage {
    pub player_entity_id: String,
    pub sequence: u64,
    pub assets: Vec<OwnedAssetEntry>,
    pub generated_at_tick: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServerOwnerAssetManifestDeltaMessage {
    pub player_entity_id: String,
    pub sequence: u64,
    pub base_sequence: u64,
    pub upserts: Vec<OwnedAssetEntry>,
    pub removals: Vec<String>,
    pub generated_at_tick: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServerAssetCatalogVersionMessage {
    pub catalog_version: String,
    pub generated_at_tick: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum NotificationSeverity {
    #[default]
    Info,
    Success,
    Warning,
    Error,
}

impl NotificationSeverity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Success => "success",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
pub enum NotificationPlacement {
    TopLeft,
    TopCenter,
    TopRight,
    BottomLeft,
    BottomCenter,
    #[default]
    BottomRight,
}

impl NotificationPlacement {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TopLeft => "top_left",
            Self::TopCenter => "top_center",
            Self::TopRight => "top_right",
            Self::BottomLeft => "bottom_left",
            Self::BottomCenter => "bottom_center",
            Self::BottomRight => "bottom_right",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NotificationImageRef {
    pub asset_id: String,
    pub alt_text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum NotificationPayload {
    Generic {
        event_type: String,
        data: serde_json::Value,
    },
    LandmarkDiscovery {
        entity_guid: String,
        display_name: String,
        landmark_kind: String,
        map_icon_asset_id: Option<String>,
        world_position_xy: Option<[f64; 2]>,
    },
}

impl NotificationPayload {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Generic { .. } => "generic",
            Self::LandmarkDiscovery { .. } => "landmark_discovery",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServerNotificationMessage {
    pub notification_id: String,
    pub player_entity_id: String,
    pub title: String,
    pub body: String,
    pub severity: NotificationSeverity,
    pub placement: NotificationPlacement,
    pub image: Option<NotificationImageRef>,
    pub payload: NotificationPayload,
    pub created_at_epoch_s: i64,
    pub auto_dismiss_after_s: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClientNotificationDismissedMessage {
    pub player_entity_id: String,
    pub notification_id: String,
}
