//! Shared ECS resources (network, assets, tuning, debug, etc.).

use bevy::ecs::component::ComponentId;
use bevy::ecs::lifecycle::RemovedComponentEntity;
use bevy::ecs::message::MessageCursor;
use bevy::prelude::*;
use sidereal_asset_runtime::AssetCacheIndex;
use sidereal_core::gateway_dtos::{
    AssetBootstrapManifestResponse, AuthTokens, CharactersResponse, EnterWorldRequest,
    EnterWorldResponse, LoginRequest, MeResponse, PasswordResetConfirmRequest,
    PasswordResetRequest, PasswordResetResponse, RegisterRequest,
};
use sidereal_game::EntityAction;
use std::collections::{HashMap, HashSet, VecDeque};
use std::future::Future;
use std::pin::Pin;

#[cfg(target_arch = "wasm32")]
pub(crate) type GatewayFuture<T> = Pin<Box<dyn Future<Output = Result<T, String>> + 'static>>;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) type GatewayFuture<T> =
    Pin<Box<dyn Future<Output = Result<T, String>> + Send + 'static>>;

#[cfg(target_arch = "wasm32")]
pub(crate) type CacheFuture<T> = Pin<Box<dyn Future<Output = Result<T, String>> + 'static>>;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) type CacheFuture<T> = Pin<Box<dyn Future<Output = Result<T, String>> + Send + 'static>>;

#[derive(Clone, Copy, Resource)]
pub(crate) struct GatewayHttpAdapter {
    pub login: fn(String, LoginRequest) -> GatewayFuture<AuthTokens>,
    pub register: fn(String, RegisterRequest) -> GatewayFuture<AuthTokens>,
    pub request_password_reset:
        fn(String, PasswordResetRequest) -> GatewayFuture<PasswordResetResponse>,
    pub confirm_password_reset: fn(String, PasswordResetConfirmRequest) -> GatewayFuture<()>,
    pub fetch_me: fn(String, String) -> GatewayFuture<MeResponse>,
    pub fetch_characters: fn(String, String) -> GatewayFuture<CharactersResponse>,
    pub enter_world: fn(String, String, EnterWorldRequest) -> GatewayFuture<EnterWorldResponse>,
    pub fetch_bootstrap_manifest:
        fn(String, String) -> GatewayFuture<AssetBootstrapManifestResponse>,
    pub fetch_asset_bytes: fn(String, String) -> GatewayFuture<Vec<u8>>,
}

#[derive(Clone, Copy, Resource)]
pub(crate) struct AssetCacheAdapter {
    pub prepare_root: fn(String) -> CacheFuture<()>,
    pub load_index: fn(String) -> CacheFuture<AssetCacheIndex>,
    pub save_index: fn(String, AssetCacheIndex) -> CacheFuture<()>,
    pub read_valid_asset: fn(String, String, String) -> CacheFuture<Option<Vec<u8>>>,
    pub write_asset: fn(String, String, Vec<u8>) -> CacheFuture<()>,
    pub read_valid_asset_sync: fn(&str, &str, &str) -> Option<Vec<u8>>,
}

#[derive(Debug, Resource, Default)]
pub(crate) struct ClientNetworkTick(pub u64);

#[derive(Debug, Resource, Default)]
pub(crate) struct ClientInputAckTracker {
    pub pending_ticks: VecDeque<u64>,
}

#[derive(Debug, Resource, Default)]
pub(crate) struct ClientInputLogState {
    pub last_logged_at_s: f64,
    pub last_logged_actions: Vec<EntityAction>,
    pub last_logged_controlled_entity_id: Option<String>,
    pub last_logged_pending_controlled_entity_id: Option<String>,
}

#[derive(Debug, Resource, Default)]
pub(crate) struct ClientAuthSyncState {
    pub sent_for_client_entities: std::collections::HashSet<Entity>,
    pub last_sent_at_s_by_client_entity: HashMap<Entity, f64>,
    pub last_player_entity_id: Option<String>,
}

#[derive(Debug, Resource, Clone, Copy)]
pub(crate) struct SessionReadyWatchdogConfig {
    pub timeout_s: f64,
}

impl SessionReadyWatchdogConfig {
    pub fn from_env() -> Self {
        let timeout_s = std::env::var("SIDEREAL_CLIENT_SESSION_READY_TIMEOUT_S")
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
            .filter(|v| v.is_finite() && *v >= 0.5)
            .unwrap_or(6.0);
        Self { timeout_s }
    }
}

#[derive(Debug, Resource, Default)]
pub(crate) struct SessionReadyWatchdogState {
    pub started_at_s: Option<f64>,
}

#[derive(Debug, Resource, Default)]
pub(crate) struct ClientControlRequestState {
    pub next_request_seq: u64,
    pub pending_controlled_entity_id: Option<String>,
    pub pending_request_seq: Option<u64>,
    pub last_sent_request_seq: Option<u64>,
    pub last_sent_at_s: f64,
}

#[derive(Debug, Resource, Default)]
pub(crate) struct ClientControlDebugState {
    pub last_controlled_entity_id: Option<String>,
    pub last_pending_controlled_entity_id: Option<String>,
    pub last_detached_free_camera: bool,
    pub handover_audit_entity_id: Option<String>,
    pub handover_audit_started_at_s: Option<f64>,
    pub last_handover_audit_log_at_s: f64,
}

#[derive(Debug, Resource, Default)]
pub(crate) struct ClientViewModeState {
    pub last_sent_mode: Option<sidereal_net::ClientLocalViewMode>,
    pub last_sent_delivery_range_m: Option<f32>,
    pub last_sent_at_s: f64,
}

#[derive(Debug, Resource, Default)]
pub(crate) struct OwnedAssetManifestCache {
    pub player_entity_id: Option<String>,
    pub sequence: u64,
    pub generated_at_tick: u64,
    pub last_sequence_mismatch_log_at_s: f64,
    pub assets_by_entity_id: HashMap<String, sidereal_net::OwnedAssetEntry>,
}

#[derive(Debug, Resource, Default)]
pub(crate) struct TacticalFogCache {
    pub player_entity_id: Option<String>,
    pub sequence: u64,
    pub generated_at_tick: u64,
    pub last_sequence_mismatch_log_at_s: f64,
    pub cell_size_m: f32,
    pub explored_cells: Vec<sidereal_net::GridCell>,
    pub live_cells: Vec<sidereal_net::GridCell>,
}

#[derive(Debug, Resource, Default)]
pub(crate) struct TacticalContactsCache {
    pub player_entity_id: Option<String>,
    pub sequence: u64,
    pub generated_at_tick: u64,
    pub last_sequence_mismatch_log_at_s: f64,
    pub contacts_by_entity_id: HashMap<String, sidereal_net::TacticalContact>,
}

#[derive(Debug, Resource, Default)]
pub(crate) struct TacticalResnapshotRequestState {
    pub player_entity_id: Option<String>,
    pub pending_fog: bool,
    pub pending_contacts: bool,
    pub last_request_at_s: f64,
    pub last_fog_snapshot_received_at_s: f64,
    pub last_contacts_snapshot_received_at_s: f64,
}

#[derive(Debug, Resource)]
pub(crate) struct TacticalMapUiState {
    pub enabled: bool,
    pub was_enabled: bool,
    pub alpha: f32,
    pub transition_start_distance: f32,
    pub last_non_map_target_distance: f32,
    pub last_non_map_max_distance: f32,
    pub transition_map_zoom_start: f32,
    pub transition_map_zoom_end: f32,
    pub map_zoom: f32,
    pub target_map_zoom: f32,
    pub pan_offset_world: Vec2,
    pub last_pan_cursor_px: Option<Vec2>,
}

impl Default for TacticalMapUiState {
    fn default() -> Self {
        Self {
            enabled: false,
            was_enabled: false,
            alpha: 0.0,
            transition_start_distance: 30.0,
            last_non_map_target_distance: 30.0,
            last_non_map_max_distance: 30.0,
            transition_map_zoom_start: 1.6666666,
            transition_map_zoom_end: 0.22727273,
            map_zoom: 1.6666666,
            target_map_zoom: 0.22727273,
            pan_offset_world: Vec2::ZERO,
            last_pan_cursor_px: None,
        }
    }
}

#[derive(Debug, Resource, Default)]
pub(crate) struct DebugBlueOverlayEnabled(pub bool);

#[derive(Debug, Resource, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct DebugGizmoOnGameplayCamera(pub bool);

impl DebugGizmoOnGameplayCamera {
    pub fn from_env() -> Self {
        Self(
            std::env::var("SIDEREAL_CLIENT_DEBUG_GIZMOS_ON_GAMEPLAY_CAMERA")
                .ok()
                .is_some_and(|value| value == "1" || value.eq_ignore_ascii_case("true")),
        )
    }
}

#[derive(Debug, Resource, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct DebugVelocityArrowAsMesh(pub bool);

impl DebugVelocityArrowAsMesh {
    pub fn from_env() -> Self {
        Self(
            std::env::var("SIDEREAL_CLIENT_DEBUG_ARROW_AS_MESH")
                .ok()
                .is_some_and(|value| value == "1" || value.eq_ignore_ascii_case("true")),
        )
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum DebugOverlayMode {
    #[default]
    Minimal,
    Full,
}

/// Toggle state and display mode for the native debug overlay.
#[allow(dead_code)]
#[derive(Debug, Resource, Default)]
pub(crate) struct DebugOverlayState {
    pub enabled: bool,
    pub mode: DebugOverlayMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DebugSeverity {
    Normal,
    Warn,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DebugEntityLane {
    Predicted,
    Confirmed,
    Interpolated,
    ConfirmedGhost,
    Auxiliary,
}

#[derive(Debug, Clone, Default)]
pub(crate) enum DebugCollisionShape {
    #[default]
    None,
    Aabb {
        half_extents: Vec3,
    },
    Outline {
        points: Vec<Vec2>,
    },
    HardpointMarker,
}

#[derive(Debug, Clone)]
pub(crate) struct DebugOverlayEntity {
    pub entity: Entity,
    pub lane: DebugEntityLane,
    pub position_xy: Vec2,
    pub rotation_rad: f32,
    pub velocity_xy: Vec2,
    pub angular_velocity_rps: f32,
    pub collision: DebugCollisionShape,
    pub is_controlled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DebugControlledLane {
    pub guid: uuid::Uuid,
    pub primary_lane: DebugEntityLane,
    pub has_confirmed_ghost: bool,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct DebugOverlayStats {
    pub predicted_count: usize,
    pub confirmed_count: usize,
    pub interpolated_count: usize,
    pub auxiliary_count: usize,
    pub duplicate_guid_groups: usize,
    pub duplicate_winner_swaps: u64,
    pub anomaly_count: usize,
    pub active_camera_count: usize,
    pub mesh_asset_count: usize,
    pub generic_sprite_material_count: usize,
    pub asteroid_material_count: usize,
    pub planet_material_count: usize,
    pub effect_material_count: usize,
    pub streamed_visual_child_count: usize,
    pub planet_pass_count: usize,
    pub tracer_pool_size: usize,
    pub active_tracers: usize,
    pub spark_pool_size: usize,
    pub active_sparks: usize,
    pub bootstrap_ready_bytes: u64,
    pub bootstrap_total_bytes: u64,
    pub runtime_dependency_candidate_count: usize,
    pub runtime_dependency_graph_rebuilds: u64,
    pub runtime_dependency_scan_runs: u64,
    pub runtime_in_flight_fetch_count: usize,
    pub render_layer_registry_rebuilds: u64,
    pub render_layer_assignment_recomputes: u64,
    pub render_layer_assignment_skips: u64,
}

#[derive(Debug, Clone)]
pub(crate) struct DebugTextRow {
    pub label: String,
    pub value: String,
    pub severity: DebugSeverity,
}

#[derive(Debug, Resource, Default)]
pub(crate) struct DebugOverlaySnapshot {
    pub frame_index: u64,
    pub entities: Vec<DebugOverlayEntity>,
    pub controlled_lane: Option<DebugControlledLane>,
    pub stats: DebugOverlayStats,
    pub text_rows: Vec<DebugTextRow>,
}

#[derive(Debug, Resource)]
pub(crate) struct DuplicateVisualResolutionState {
    pub guid_by_entity: HashMap<Entity, uuid::Uuid>,
    pub entities_by_guid: HashMap<uuid::Uuid, HashSet<Entity>>,
    pub winner_by_guid: HashMap<uuid::Uuid, Entity>,
    pub dirty_guids: HashSet<uuid::Uuid>,
    pub dirty_all: bool,
    pub duplicate_guid_groups: usize,
    pub winner_swap_count: u64,
    pub entity_guid_removal_cursor: Option<MessageCursor<RemovedComponentEntity>>,
    pub world_entity_removal_cursor: Option<MessageCursor<RemovedComponentEntity>>,
    pub controlled_entity_guid_removal_cursor: Option<MessageCursor<RemovedComponentEntity>>,
    pub player_tag_removal_cursor: Option<MessageCursor<RemovedComponentEntity>>,
    pub controlled_entity_removal_cursor: Option<MessageCursor<RemovedComponentEntity>>,
    pub interpolated_removal_cursor: Option<MessageCursor<RemovedComponentEntity>>,
    pub predicted_removal_cursor: Option<MessageCursor<RemovedComponentEntity>>,
    pub position_history_removal_cursor: Option<MessageCursor<RemovedComponentEntity>>,
    pub rotation_history_removal_cursor: Option<MessageCursor<RemovedComponentEntity>>,
}

impl Default for DuplicateVisualResolutionState {
    fn default() -> Self {
        Self {
            guid_by_entity: HashMap::new(),
            entities_by_guid: HashMap::new(),
            winner_by_guid: HashMap::new(),
            dirty_guids: HashSet::new(),
            dirty_all: true,
            duplicate_guid_groups: 0,
            winner_swap_count: 0,
            entity_guid_removal_cursor: None,
            world_entity_removal_cursor: None,
            controlled_entity_guid_removal_cursor: None,
            player_tag_removal_cursor: None,
            controlled_entity_removal_cursor: None,
            interpolated_removal_cursor: None,
            predicted_removal_cursor: None,
            position_history_removal_cursor: None,
            rotation_history_removal_cursor: None,
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct DebugOverlayDisplayMetrics {
    pub sampled_fps: Option<f64>,
    pub sampled_frame_ms: Option<f64>,
    pub last_sample_at_s: f64,
    pub initialized: bool,
}

#[derive(Debug, Resource, Default)]
pub(crate) struct NameplateUiState {
    pub enabled: bool,
}

#[derive(Debug, Resource, Default)]
pub(crate) struct StarfieldMotionState {
    pub prev_speed: f32,
    pub initialized: bool,
    /// Accumulated scroll in UV space (distance-over-time). Required for continual parallax; we integrate velocity*dt each frame so scroll reflects total displacement. Shader uses fract() for wrapping.
    pub accumulated_scroll_uv: Vec2,
    #[allow(dead_code)]
    pub starfield_drift_uv: Vec2,
    pub background_drift_uv: Vec2,
    pub smoothed_warp: f32,
}

#[derive(Debug, Resource, Clone)]
pub(crate) struct FullscreenExternalWorldData {
    pub viewport_time: Vec4,
    pub drift_intensity: Vec4,
    pub velocity_dir: Vec4,
}

impl Default for FullscreenExternalWorldData {
    fn default() -> Self {
        Self {
            viewport_time: Vec4::new(1920.0, 1080.0, 0.0, 0.0),
            drift_intensity: Vec4::new(0.0, 0.0, 1.0, 1.0),
            velocity_dir: Vec4::new(0.0, 1.0, 1.0, 0.0),
        }
    }
}

#[derive(Debug, Resource)]
pub(crate) struct CameraMotionState {
    pub world_position_xy: Vec2,
    pub smoothed_position_xy: Vec2,
    pub parallax_position_xy: Vec2,
    pub prev_position_xy: Vec2,
    pub frame_delta_xy: Vec2,
    pub smoothed_velocity_xy: Vec2,
    pub initialized: bool,
}

impl Default for CameraMotionState {
    fn default() -> Self {
        Self {
            world_position_xy: Vec2::ZERO,
            smoothed_position_xy: Vec2::ZERO,
            parallax_position_xy: Vec2::ZERO,
            prev_position_xy: Vec2::ZERO,
            frame_delta_xy: Vec2::ZERO,
            smoothed_velocity_xy: Vec2::ZERO,
            initialized: false,
        }
    }
}

#[derive(Debug, Resource, Clone)]
pub(crate) struct PredictionLifecycleAuditConfig {
    pub enabled: bool,
    pub target_guid: Option<uuid::Uuid>,
    pub interval_s: f64,
}

impl PredictionLifecycleAuditConfig {
    pub fn from_env() -> Self {
        let raw = std::env::var("SIDEREAL_CLIENT_LIFECYCLE_AUDIT_GUID").ok();
        let enabled = raw.is_some();
        let target_guid = raw.as_deref().and_then(|value| {
            let trimmed = value.trim();
            if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("controlled") {
                None
            } else {
                uuid::Uuid::parse_str(trimmed).ok()
            }
        });
        Self {
            enabled,
            target_guid,
            interval_s: 0.5,
        }
    }
}

#[derive(Debug, Resource, Default)]
pub(crate) struct PredictionLifecycleAuditState {
    pub last_logged_at_s: f64,
    pub last_overlay_winner: Option<Entity>,
    pub last_overlay_lane: Option<DebugEntityLane>,
    pub last_visual_winner: Option<Entity>,
    pub last_visual_winner_swap_count: u64,
}

#[derive(Debug, Clone)]
pub(crate) struct CompiledRuntimeRenderLayerRule {
    pub rule_id: String,
    pub target_layer_id: String,
    pub priority: i32,
    pub labels_any: Vec<String>,
    pub labels_all: Vec<String>,
    pub archetypes_any: Vec<String>,
    pub components_all: Vec<ComponentId>,
    pub components_any: Vec<ComponentId>,
}

#[derive(Debug, Resource, Clone)]
pub(crate) struct RuntimeRenderLayerRegistry {
    pub definitions_by_id: HashMap<String, sidereal_game::RuntimeRenderLayerDefinition>,
    pub world_rules: Vec<CompiledRuntimeRenderLayerRule>,
    pub watched_component_ids: Vec<ComponentId>,
    pub default_world_layer: sidereal_game::RuntimeRenderLayerDefinition,
}

impl Default for RuntimeRenderLayerRegistry {
    fn default() -> Self {
        Self {
            definitions_by_id: HashMap::new(),
            world_rules: Vec::new(),
            watched_component_ids: Vec::new(),
            default_world_layer: sidereal_game::default_main_world_render_layer(),
        }
    }
}

#[derive(Debug, Resource, Default)]
pub(crate) struct RuntimeRenderLayerRegistryState {
    pub generation: u64,
    pub generated_registry_signature: u64,
    pub definition_count: usize,
    pub rule_count: usize,
    pub post_process_stack_count: usize,
    pub definition_removal_cursor: Option<MessageCursor<RemovedComponentEntity>>,
    pub rule_removal_cursor: Option<MessageCursor<RemovedComponentEntity>>,
    pub post_process_stack_removal_cursor: Option<MessageCursor<RemovedComponentEntity>>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct CachedRuntimeRenderLayerAssignment {
    pub registry_generation: u64,
    pub input_hash: u64,
}

#[derive(Debug, Resource, Default)]
pub(crate) struct RuntimeRenderLayerAssignmentCache {
    pub by_entity: HashMap<Entity, CachedRuntimeRenderLayerAssignment>,
    pub last_world_entity_count: usize,
    pub label_removal_cursor: Option<MessageCursor<RemovedComponentEntity>>,
    pub override_removal_cursor: Option<MessageCursor<RemovedComponentEntity>>,
    pub watched_component_removal_cursors:
        HashMap<ComponentId, MessageCursor<RemovedComponentEntity>>,
}

#[derive(Debug, Resource, Default)]
pub(crate) struct RenderLayerPerfCounters {
    pub registry_sync_runs: u64,
    pub registry_rebuilds: u64,
    pub assignment_sync_runs: u64,
    pub assignment_full_scans: u64,
    pub assignment_targeted_scans: u64,
    pub assignment_entities_considered: u64,
    pub assignment_recomputes: u64,
    pub assignment_skips: u64,
}

#[derive(Debug, Resource, Default)]
pub(crate) struct RuntimeSharedQuadMesh {
    pub unit_quad: Option<Handle<Mesh>>,
    pub allocations: u64,
}

#[derive(Debug, Resource, Default)]
pub(crate) struct BootstrapWatchdogState {
    pub in_world_entered_at_s: Option<f64>,
    pub replication_state_seen: bool,
    pub asset_manifest_seen: bool,
    pub last_bootstrap_ready_bytes: u64,
    pub last_bootstrap_progress_at_s: f64,
    pub timeout_dialog_shown: bool,
    pub stream_stall_dialog_shown: bool,
    pub no_world_state_dialog_shown: bool,
}

#[derive(Debug, Resource, Default)]
pub(crate) struct DeferredPredictedAdoptionState {
    pub waiting_entity_id: Option<String>,
    pub wait_started_at_s: Option<f64>,
    pub last_warn_at_s: f64,
    pub last_missing_components: String,
    pub dialog_shown: bool,
    pub resolved_samples: u64,
    pub resolved_total_wait_s: f64,
    pub resolved_max_wait_s: f64,
    pub last_summary_at_s: f64,
    pub last_runtime_summary_at_s: f64,
    // Sidereal supports dynamic control handoff and free-roam via the persisted player
    // anchor. If the intended controlled ship never gets a Predicted clone, we must not
    // silently bind local control to an Interpolated fallback because that breaks the
    // single-writer motion invariant and feels "jerky" rather than truly predicted.
    pub missing_predicted_control_entity_id: Option<String>,
    pub last_missing_predicted_warn_at_s: f64,
}

#[derive(Debug, Resource, Clone, Copy)]
pub(crate) struct PredictionBootstrapTuning {
    pub defer_warn_after_s: f64,
    pub defer_warn_interval_s: f64,
    pub defer_dialog_after_s: f64,
    pub defer_summary_interval_s: f64,
}

impl PredictionBootstrapTuning {
    pub fn from_env() -> Self {
        let parse = |key: &str, default: f64| {
            std::env::var(key)
                .ok()
                .and_then(|v| v.parse::<f64>().ok())
                .filter(|v| v.is_finite() && *v >= 0.0)
                .unwrap_or(default)
        };
        Self {
            defer_warn_after_s: parse("SIDEREAL_CLIENT_DEFER_WARN_AFTER_S", 1.0),
            defer_warn_interval_s: parse("SIDEREAL_CLIENT_DEFER_WARN_INTERVAL_S", 1.0),
            defer_dialog_after_s: parse("SIDEREAL_CLIENT_DEFER_DIALOG_AFTER_S", 4.0),
            defer_summary_interval_s: parse("SIDEREAL_CLIENT_DEFER_SUMMARY_INTERVAL_S", 30.0),
        }
    }
}

#[derive(Debug, Resource, Clone, Copy)]
pub(crate) struct PredictionCorrectionTuning {
    pub max_rollback_ticks: u16,
    pub instant_correction: bool,
    pub rollback_state: PredictionRollbackStateTuning,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum PredictionRollbackStateTuning {
    Always,
    Check,
    Disabled,
}

impl PredictionRollbackStateTuning {
    fn from_env() -> Self {
        match std::env::var("SIDEREAL_CLIENT_ROLLBACK_STATE")
            .ok()
            .as_deref()
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("always") => Self::Always,
            Some("disabled") => Self::Disabled,
            _ => Self::Check,
        }
    }
}

impl PredictionCorrectionTuning {
    pub fn from_env() -> Self {
        let max_rollback_ticks = std::env::var("SIDEREAL_CLIENT_MAX_ROLLBACK_TICKS")
            .ok()
            .and_then(|v| v.parse::<u16>().ok())
            .unwrap_or(100);
        let instant_correction = std::env::var("SIDEREAL_CLIENT_INSTANT_CORRECTION")
            .ok()
            .is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"));
        Self {
            max_rollback_ticks,
            instant_correction,
            rollback_state: PredictionRollbackStateTuning::from_env(),
        }
    }
}

#[derive(Debug, Resource, Clone, Copy)]
pub(crate) struct NearbyCollisionProxyTuning {
    pub radius_m: f32,
    pub max_proxies: usize,
    pub reconcile_interval_s: f64,
}

impl NearbyCollisionProxyTuning {
    pub fn from_env() -> Self {
        let radius_m = std::env::var("SIDEREAL_CLIENT_NEARBY_COLLISION_PROXY_RADIUS_M")
            .ok()
            .and_then(|v| v.parse::<f32>().ok())
            .filter(|v| v.is_finite() && *v > 0.0)
            .unwrap_or(200.0);
        let max_proxies = std::env::var("SIDEREAL_CLIENT_NEARBY_COLLISION_PROXY_MAX")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .filter(|v| *v > 0)
            // Sidereal's locally predicted ship should not phase through nearby asteroids and
            // only learn about the collision one server round-trip later. We therefore keep a
            // small local collision-proxy set enabled by default for native prediction, even
            // though many Lightyear examples omit this because they do not support our
            // free-roam-to-ship handoff and dense local obstacle fields.
            .unwrap_or(8);
        let reconcile_interval_s =
            std::env::var("SIDEREAL_CLIENT_MOTION_OWNERSHIP_RECONCILE_INTERVAL_S")
                .ok()
                .and_then(|v| v.parse::<f64>().ok())
                .filter(|v| v.is_finite() && *v >= 0.0)
                .unwrap_or(0.1);
        Self {
            radius_m,
            max_proxies,
            reconcile_interval_s,
        }
    }
}

#[derive(Debug, Resource, Default)]
pub(crate) struct MotionOwnershipReconcileState {
    pub last_target_guid: Option<uuid::Uuid>,
    pub last_target_entity: Option<Entity>,
    pub last_reconcile_at_s: f64,
    pub dirty: bool,
}

#[derive(Resource, Debug, Clone, Copy)]
pub(crate) struct HeadlessTransportMode(pub bool);

#[derive(Resource, Debug)]
pub(crate) struct HeadlessAccountSwitchPlan {
    pub switch_after_s: f64,
    pub switched: bool,
    pub next_player_entity_id: String,
    pub next_access_token: String,
}

#[derive(Resource, Clone)]
pub(crate) struct AssetRootPath(pub String);

#[derive(Resource, Clone)]
pub(crate) struct EmbeddedFonts {
    pub bold: Handle<Font>,
    pub regular: Handle<Font>,
}

#[derive(Resource, Default)]
pub(crate) struct RemoteEntityRegistry {
    pub by_entity_id: HashMap<String, Entity>,
}

#[derive(Debug, Clone, Copy, Resource, Default)]
#[allow(dead_code)]
pub(crate) struct LocalSimulationDebugMode(pub bool);

impl LocalSimulationDebugMode {
    #[allow(dead_code)]
    pub fn from_env() -> Self {
        let enabled = std::env::var("SIDEREAL_CLIENT_PHYSICS_MODE")
            .ok()
            .is_some_and(|v| v.eq_ignore_ascii_case("local"));
        if enabled {
            eprintln!(
                "[sidereal-client] LOCAL DEBUG SIMULATION: enabled (full local simulation, no reconciliation)"
            );
        }
        Self(enabled)
    }
}

#[derive(Debug, Clone, Copy, Resource, Default)]
#[allow(dead_code)]
pub(crate) struct MotionOwnershipAuditEnabled(pub bool);

impl MotionOwnershipAuditEnabled {
    #[allow(dead_code)]
    pub fn from_env() -> Self {
        let enabled = std::env::var("SIDEREAL_CLIENT_MOTION_AUDIT")
            .ok()
            .is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"));
        Self(enabled)
    }
}

#[derive(Debug, Resource, Default)]
#[allow(dead_code)]
pub(crate) struct MotionOwnershipAuditState {
    pub last_logged_at_s: f64,
}

#[derive(Debug, Resource)]
#[allow(dead_code)]
pub(crate) struct ClientInputSendState {
    pub last_sent_at_s: f64,
    pub last_sent_actions: Vec<EntityAction>,
    pub last_sent_target_entity_id: Option<String>,
}

impl Default for ClientInputSendState {
    fn default() -> Self {
        Self {
            last_sent_at_s: f64::NEG_INFINITY,
            last_sent_actions: Vec::new(),
            last_sent_target_entity_id: None,
        }
    }
}

/// When set, the client will send ClientDisconnectNotifyMessage and then disconnect (logout or window close).
#[derive(Debug, Resource, Default)]
pub(crate) struct PendingDisconnectNotify(pub Option<String>);

/// Tracks whether a pending disconnect notify has already been sent once.
/// We delay transport Disconnect by one frame to improve notify delivery reliability.
#[derive(Debug, Resource, Default, PartialEq, Eq)]
pub(crate) struct PendingDisconnectNotifySent(pub bool);

/// When true, logout cleanup (clear state, transition to Auth) should run.
#[derive(Debug, Resource, Default, PartialEq, Eq)]
pub(crate) struct LogoutCleanupRequested(pub bool);

/// UI-driven disconnect request (for example from the in-world Escape menu).
#[derive(Debug, Resource, Default, PartialEq, Eq)]
pub(crate) struct DisconnectRequest(pub bool);

/// In-world Escape menu visibility state.
#[derive(Debug, Resource, Default)]
pub(crate) struct PauseMenuState {
    pub open: bool,
}
