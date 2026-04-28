//! Shared ECS resources (network, assets, tuning, debug, etc.).

use bevy::ecs::component::ComponentId;
use bevy::ecs::lifecycle::RemovedComponentEntity;
use bevy::ecs::message::MessageCursor;
use bevy::prelude::*;
use sidereal_asset_runtime::AssetCacheIndex;
use sidereal_core::gateway_dtos::{
    AssetBootstrapManifestResponse, AuthTokens, CharactersResponse, EnterWorldRequest,
    EnterWorldResponse, LoginRequest, MeResponse, PasswordLoginResponse,
    StartupAssetManifestResponse, TotpLoginChallengeRequest,
};
use sidereal_game::{EntityAction, ScannerContactDetailTier};
use std::collections::{HashMap, HashSet, VecDeque};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

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
    pub login: fn(String, LoginRequest) -> GatewayFuture<PasswordLoginResponse>,
    pub verify_totp_login_challenge:
        fn(String, TotpLoginChallengeRequest) -> GatewayFuture<AuthTokens>,
    pub fetch_me: fn(String, String) -> GatewayFuture<MeResponse>,
    pub fetch_characters: fn(String, String) -> GatewayFuture<CharactersResponse>,
    pub enter_world: fn(String, String, EnterWorldRequest) -> GatewayFuture<EnterWorldResponse>,
    pub fetch_startup_manifest: fn(String) -> GatewayFuture<StartupAssetManifestResponse>,
    pub fetch_bootstrap_manifest:
        fn(String, String) -> GatewayFuture<AssetBootstrapManifestResponse>,
    pub fetch_public_asset_bytes: fn(String) -> GatewayFuture<Vec<u8>>,
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

#[derive(Debug, Default)]
pub(crate) struct ClientViewModeState {
    pub last_sent_mode: Option<sidereal_net::ClientLocalViewMode>,
    pub last_sent_delivery_range_m: Option<f32>,
    pub last_sent_at_s: f64,
}

impl Resource for ClientViewModeState {}

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
    pub revision: u64,
    pub cell_size_m: f32,
    pub explored_cells: Vec<sidereal_net::GridCell>,
    pub live_cells: Vec<sidereal_net::GridCell>,
    pub revealed_cells: HashSet<sidereal_net::GridCell>,
}

#[derive(Debug, Resource, Default)]
pub(crate) struct TacticalContactsCache {
    pub player_entity_id: Option<String>,
    pub sequence: u64,
    pub generated_at_tick: u64,
    pub last_sequence_mismatch_log_at_s: f64,
    pub revision: u64,
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

#[derive(Debug, Resource)]
pub(crate) struct TacticalSensorRingUiState {
    pub enabled: bool,
    pub alpha: f32,
    pub last_controlled_entity_id: Option<String>,
    pub last_unavailable_notice_at_s: f64,
}

impl Default for TacticalSensorRingUiState {
    fn default() -> Self {
        Self {
            enabled: false,
            alpha: 0.0,
            last_controlled_entity_id: None,
            last_unavailable_notice_at_s: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ResolvedScannerProfile {
    pub detail_tier: ScannerContactDetailTier,
    pub level: u8,
    pub effective_range_m: f32,
    pub supports_density: bool,
    pub supports_directional_awareness: bool,
    pub max_contacts: u16,
}

#[derive(Debug, Resource, Default)]
pub(crate) struct ActiveScannerProfileCache {
    pub controlled_entity_id: Option<String>,
    pub profile: Option<ResolvedScannerProfile>,
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
    pub guid: uuid::Uuid,
    pub label: String,
    pub lane: DebugEntityLane,
    pub position_xy: Vec2,
    pub rotation_rad: f32,
    pub velocity_xy: Vec2,
    pub angular_velocity_rps: f32,
    pub collision: DebugCollisionShape,
    pub is_controlled: bool,
    pub is_component: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DebugControlledLane {
    pub guid: uuid::Uuid,
    pub primary_lane: DebugEntityLane,
    pub has_confirmed_ghost: bool,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct DebugOverlayStats {
    pub window_focused: bool,
    pub focus_transitions: u64,
    pub last_focus_change_age_s: f64,
    pub observed_unfocused_duration_s: f64,
    pub observed_unfocused_frames: u64,
    pub prediction_recovery_phase: String,
    pub prediction_recovery_suppressing_input: bool,
    pub prediction_recovery_last_unfocused_s: f64,
    pub prediction_recovery_transitions: u64,
    pub prediction_recovery_neutral_sends: u64,
    pub last_update_delta_ms: f64,
    pub max_update_delta_ms: f64,
    pub last_stall_gap_ms: f64,
    pub last_stall_gap_estimated_ticks: u32,
    pub max_stall_gap_ms: f64,
    pub max_stall_gap_estimated_ticks: u32,
    pub fixed_runs_last_frame: u32,
    pub fixed_runs_max_frame: u32,
    pub fixed_overstep_ms: f64,
    pub rollback_budget_ticks: u16,
    pub rollback_budget_ms: f64,
    pub local_timeline_tick: Option<u32>,
    pub controlled_confirmed_tick: Option<u32>,
    pub controlled_tick_gap: Option<u32>,
    pub control_bootstrap_phase: String,
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
    pub runtime_pending_fetch_count: usize,
    pub runtime_pending_persist_count: usize,
    pub runtime_asset_fetch_poll_last_ms: f64,
    pub runtime_asset_fetch_poll_max_ms: f64,
    pub runtime_asset_persist_task_last_ms: f64,
    pub runtime_asset_persist_task_max_ms: f64,
    pub runtime_asset_save_index_last_ms: f64,
    pub runtime_asset_save_index_max_ms: f64,
    pub render_layer_registry_rebuilds: u64,
    pub render_layer_assignment_recomputes: u64,
    pub render_layer_assignment_skips: u64,
    pub tactical_contacts_last: usize,
    pub tactical_markers_last: usize,
    pub tactical_marker_spawns_last: usize,
    pub tactical_marker_updates_last: usize,
    pub tactical_marker_despawns_last: usize,
    pub tactical_overlay_last_ms: f64,
    pub tactical_overlay_max_ms: f64,
    pub nameplate_targets_last: usize,
    pub nameplate_visible_last: usize,
    pub nameplate_hidden_last: usize,
    pub nameplate_health_updates_last: usize,
    pub nameplate_entity_data_last: usize,
    pub nameplate_sync_last_ms: f64,
    pub nameplate_sync_max_ms: f64,
    pub nameplate_position_last_ms: f64,
    pub nameplate_position_max_ms: f64,
    pub nameplate_camera_candidates_last: usize,
    pub nameplate_camera_active_last: usize,
    pub nameplate_missing_target_last: usize,
    pub nameplate_projection_failures_last: usize,
    pub nameplate_viewport_culled_last: usize,
}

#[derive(Debug, Resource, Clone, Copy)]
pub(crate) struct ClientInputTimelineTuning {
    pub fixed_input_delay_ticks: u16,
    pub max_predicted_ticks: u16,
    pub unfocused_max_predicted_ticks: u16,
}

impl ClientInputTimelineTuning {
    pub fn from_env() -> Self {
        let fixed_input_delay_ticks = std::env::var("SIDEREAL_CLIENT_INPUT_DELAY_TICKS")
            .ok()
            .and_then(|v| v.parse::<u16>().ok())
            .unwrap_or(3);
        let max_predicted_ticks = std::env::var("SIDEREAL_CLIENT_MAX_PREDICTED_TICKS")
            .ok()
            .and_then(|v| v.parse::<u16>().ok())
            .unwrap_or(24);
        let unfocused_max_predicted_ticks =
            std::env::var("SIDEREAL_CLIENT_UNFOCUSED_MAX_PREDICTED_TICKS")
                .ok()
                .and_then(|v| v.parse::<u16>().ok())
                .unwrap_or(max_predicted_ticks);
        Self {
            fixed_input_delay_ticks,
            max_predicted_ticks,
            unfocused_max_predicted_ticks,
        }
    }
}

#[derive(Debug, Resource, Clone, Copy)]
pub(crate) struct ClientInterpolationTimelineTuning {
    pub min_delay_ms: u64,
    pub send_interval_ratio: f32,
}

impl ClientInterpolationTimelineTuning {
    pub fn from_env() -> Self {
        let min_delay_ms = std::env::var("SIDEREAL_CLIENT_INTERPOLATION_MIN_DELAY_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(50);
        let send_interval_ratio =
            std::env::var("SIDEREAL_CLIENT_INTERPOLATION_SEND_INTERVAL_RATIO")
                .ok()
                .and_then(|v| v.parse::<f32>().ok())
                .filter(|v| v.is_finite() && *v > 0.0)
                .unwrap_or(2.0);
        Self {
            min_delay_ms,
            send_interval_ratio,
        }
    }
}

#[derive(Debug, Resource, Default)]
pub(crate) struct ClientTimelineFocusState {
    pub last_window_focused: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum PredictionRecoveryReason {
    FocusStall,
    RollbackGapExceeded,
    ConfirmedTickGapExceeded,
    ConfirmedStateMissing,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum NativePredictionRecoveryPhase {
    Focused,
    Unfocused {
        started_at_s: f64,
    },
    Recovering {
        regain_at_s: f64,
        suppress_input_until_s: f64,
        reason: PredictionRecoveryReason,
    },
}

impl NativePredictionRecoveryPhase {
    pub(crate) fn label(self, now_s: f64) -> String {
        match self {
            Self::Focused => "Focused".to_string(),
            Self::Unfocused { started_at_s } => {
                format!("Unfocused {:>4.1}s", (now_s - started_at_s).max(0.0))
            }
            Self::Recovering {
                suppress_input_until_s,
                reason,
                ..
            } => format!(
                "Recovering {:?} {:>4.2}s",
                reason,
                (suppress_input_until_s - now_s).max(0.0)
            ),
        }
    }
}

#[derive(Debug, Resource, Clone, Copy)]
pub(crate) struct NativePredictionRecoveryTuning {
    pub min_unfocused_s: f64,
    pub suppress_input_s: f64,
    pub resync_after_s: f64,
    pub max_tick_gap: u32,
}

impl NativePredictionRecoveryTuning {
    pub fn from_env() -> Self {
        let parse_f64 = |key: &str, default: f64| {
            std::env::var(key)
                .ok()
                .and_then(|v| v.parse::<f64>().ok())
                .filter(|v| v.is_finite() && *v >= 0.0)
                .unwrap_or(default)
        };
        let max_tick_gap = std::env::var("SIDEREAL_CLIENT_FOCUS_RECOVERY_MAX_TICK_GAP")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(60);
        Self {
            min_unfocused_s: parse_f64("SIDEREAL_CLIENT_FOCUS_RECOVERY_MIN_UNFOCUSED_S", 0.5),
            suppress_input_s: parse_f64("SIDEREAL_CLIENT_FOCUS_RECOVERY_SUPPRESS_INPUT_S", 0.15),
            resync_after_s: parse_f64("SIDEREAL_CLIENT_FOCUS_RECOVERY_RESYNC_AFTER_S", 1.0),
            max_tick_gap,
        }
    }
}

#[derive(Debug, Resource)]
pub(crate) struct NativePredictionRecoveryState {
    pub phase: NativePredictionRecoveryPhase,
    pub last_window_focused: Option<bool>,
    pub last_unfocused_duration_s: f64,
    pub pending_neutral_send: bool,
    pub transition_count: u64,
    pub neutral_send_count: u64,
}

impl Default for NativePredictionRecoveryState {
    fn default() -> Self {
        Self {
            phase: NativePredictionRecoveryPhase::Focused,
            last_window_focused: None,
            last_unfocused_duration_s: 0.0,
            pending_neutral_send: false,
            transition_count: 0,
            neutral_send_count: 0,
        }
    }
}

impl NativePredictionRecoveryState {
    pub(crate) fn is_suppressing_input(&self, now_s: f64) -> bool {
        match self.phase {
            NativePredictionRecoveryPhase::Focused => false,
            NativePredictionRecoveryPhase::Unfocused { .. } => true,
            NativePredictionRecoveryPhase::Recovering {
                suppress_input_until_s,
                ..
            } => now_s < suppress_input_until_s,
        }
    }

    pub(crate) fn complete_recovery_if_elapsed(&mut self, now_s: f64) {
        if let NativePredictionRecoveryPhase::Recovering {
            suppress_input_until_s,
            ..
        } = self.phase
            && now_s >= suppress_input_until_s
        {
            self.phase = NativePredictionRecoveryPhase::Focused;
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct DebugTextRow {
    pub label: String,
    pub value: String,
    // Severity is kept in the snapshot so future per-row styling can use it without
    // re-deriving diagnostic state from the rendered text.
    #[allow(dead_code)]
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

#[derive(Debug, Clone, Copy)]
pub(crate) struct AnnotationCalloutEntry {
    pub root: Entity,
    pub text: Entity,
    pub line: Entity,
}

#[derive(Debug, Resource, Default)]
pub(crate) struct AnnotationCalloutRegistry {
    pub active_by_target: HashMap<Entity, AnnotationCalloutEntry>,
    pub free_entries: Vec<AnnotationCalloutEntry>,
    pub allocated_entries: usize,
}

#[derive(Debug, Resource, Default)]
pub(crate) struct RuntimeStallDiagnostics {
    pub window_focused: bool,
    pub focus_initialized: bool,
    pub focus_transitions: u64,
    pub last_focus_change_at_s: f64,
    pub observed_unfocused_duration_s: f64,
    pub observed_unfocused_frames: u64,
    pub last_update_delta_ms: f64,
    pub max_update_delta_ms: f64,
    pub last_stall_gap_ms: f64,
    pub last_stall_gap_estimated_ticks: u32,
    pub max_stall_gap_ms: f64,
    pub max_stall_gap_estimated_ticks: u32,
    pub fixed_runs_current_frame: u32,
    pub fixed_runs_last_frame: u32,
    pub fixed_runs_max_frame: u32,
    pub fixed_overstep_ms: f64,
}

#[derive(Debug, Resource)]
pub(crate) struct NameplateUiState {
    pub enabled: bool,
}

impl Default for NameplateUiState {
    fn default() -> Self {
        Self { enabled: true }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct NameplateRegistryEntry {
    pub root: Entity,
    pub health_fill: Entity,
}

#[derive(Debug, Resource, Default)]
pub(crate) struct NameplateRegistry {
    pub active_by_target: HashMap<Entity, NameplateRegistryEntry>,
    pub free_entries: Vec<NameplateRegistryEntry>,
    pub allocated_entries: usize,
}

#[derive(Debug, Resource, Default)]
pub(crate) struct RuntimeAssetPerfCounters {
    pub queue_runs: u64,
    pub fetch_poll_runs: u64,
    pub fetches_queued: u64,
    pub fetches_completed: u64,
    pub persists_completed: u64,
    pub save_index_starts: u64,
    pub save_index_completions: u64,
    pub pending_fetch_count: usize,
    pub pending_persist_count: usize,
    pub cache_index_dirty: bool,
    pub fetch_poll_last_ms: f64,
    pub fetch_poll_max_ms: f64,
    pub persist_task_last_ms: f64,
    pub persist_task_max_ms: f64,
    pub save_index_last_ms: f64,
    pub save_index_max_ms: f64,
}

#[derive(Debug, Resource, Default)]
pub(crate) struct HudPerfCounters {
    pub tactical_overlay_runs: u64,
    pub tactical_overlay_last_ms: f64,
    pub tactical_overlay_max_ms: f64,
    pub tactical_contacts_last: usize,
    pub tactical_markers_last: usize,
    pub tactical_marker_spawns_last: usize,
    pub tactical_marker_updates_last: usize,
    pub tactical_marker_despawns_last: usize,
    pub nameplate_sync_runs: u64,
    pub nameplate_sync_last_ms: f64,
    pub nameplate_sync_max_ms: f64,
    pub nameplate_targets_last: usize,
    pub nameplate_spawned_last: usize,
    pub nameplate_despawned_last: usize,
    pub nameplate_position_runs: u64,
    pub nameplate_position_last_ms: f64,
    pub nameplate_position_max_ms: f64,
    pub nameplate_camera_candidates_last: usize,
    pub nameplate_camera_active_last: usize,
    pub nameplate_entity_data_last: usize,
    pub nameplate_visible_last: usize,
    pub nameplate_hidden_last: usize,
    pub nameplate_health_updates_last: usize,
    pub nameplate_missing_target_last: usize,
    pub nameplate_projection_failures_last: usize,
    pub nameplate_viewport_culled_last: usize,
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
    // Sidereal supports dynamic control handoff and free-roam via the persisted player
    // anchor. If the intended controlled entity never gets a Predicted clone, we must not
    // silently bind local control to an Interpolated fallback because that breaks the
    // single-writer motion invariant and feels "jerky" rather than truly predicted.
    pub missing_predicted_control_entity_id: Option<String>,
    pub last_missing_predicted_warn_at_s: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ControlBootstrapPhase {
    Idle,
    PendingPredicted {
        target_entity_id: String,
        generation: u64,
    },
    ActivePredicted {
        target_entity_id: String,
        generation: u64,
        entity: Entity,
    },
}

#[derive(Debug, Resource)]
pub(crate) struct ControlBootstrapState {
    pub authoritative_target_entity_id: Option<String>,
    pub generation: u64,
    pub phase: ControlBootstrapPhase,
    pub last_transition_at_s: f64,
}

impl Default for ControlBootstrapState {
    fn default() -> Self {
        Self {
            authoritative_target_entity_id: None,
            generation: 0,
            phase: ControlBootstrapPhase::Idle,
            last_transition_at_s: 0.0,
        }
    }
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
            Some("disabled") => Self::Disabled,
            Some("check") => Self::Check,
            _ => Self::Always,
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
    pub display: Handle<Font>,
    pub mono: Handle<Font>,
    pub mono_bold: Handle<Font>,
}

#[derive(Resource, Default)]
pub(crate) struct RemoteEntityRegistry {
    pub by_entity_id: HashMap<String, Entity>,
}

#[derive(Debug, Resource)]
#[allow(dead_code)]
pub(crate) struct ClientInputSendState {
    pub last_sent_at_s: f64,
    pub last_sent_actions: Vec<EntityAction>,
    pub last_sent_target_entity_id: Option<String>,
    pub headless_script_started_at_s: Option<f64>,
}

impl Default for ClientInputSendState {
    fn default() -> Self {
        Self {
            last_sent_at_s: f64::NEG_INFINITY,
            last_sent_actions: Vec::new(),
            last_sent_target_entity_id: None,
            headless_script_started_at_s: None,
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

#[derive(Debug, Resource, Default)]
pub(crate) struct ServerDisconnectDialogState {
    pub shown: bool,
}

#[derive(Debug, Resource, Clone, Default)]
pub(crate) struct SharedClientTransportErrorBuffer {
    inner: Arc<Mutex<VecDeque<String>>>,
}

impl SharedClientTransportErrorBuffer {
    pub(crate) fn push(&self, message: String) {
        if let Ok(mut guard) = self.inner.lock() {
            guard.push_back(message);
            while guard.len() > 16 {
                let _ = guard.pop_front();
            }
        }
    }

    pub(crate) fn drain(&self) -> Vec<String> {
        let Ok(mut guard) = self.inner.lock() else {
            return Vec::new();
        };
        guard.drain(..).collect()
    }
}
