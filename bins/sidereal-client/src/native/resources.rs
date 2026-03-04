//! Shared ECS resources (network, assets, tuning, debug, etc.).

use bevy::prelude::*;
use sidereal_game::EntityAction;
use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

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
}

#[derive(Debug, Resource, Default)]
pub(crate) struct DebugBlueOverlayEnabled(pub bool);

/// When true, F3 debug overlay is active: collision AABB wireframes, ship AABB + velocity arrow, hardpoint markers.
#[derive(Debug, Resource, Default)]
pub(crate) struct DebugOverlayEnabled {
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
            prev_position_xy: Vec2::ZERO,
            frame_delta_xy: Vec2::ZERO,
            smoothed_velocity_xy: Vec2::ZERO,
            initialized: false,
        }
    }
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
            .unwrap_or(0);
        Self {
            radius_m,
            max_proxies,
        }
    }
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

#[derive(Resource, Debug)]
/// Caps client frame rate when set. Configure via `SIDEREAL_CLIENT_MAX_FPS` (default 60; 0 = disabled).
pub(crate) struct FrameRateCap {
    pub frame_duration: Duration,
    pub last_frame_end: Instant,
}

impl FrameRateCap {
    pub fn from_env(default_fps: u32) -> Option<Self> {
        let fps = std::env::var("SIDEREAL_CLIENT_MAX_FPS")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(default_fps);
        if fps == 0 {
            return None;
        }
        Some(Self {
            frame_duration: Duration::from_secs_f64(1.0 / fps as f64),
            last_frame_end: Instant::now(),
        })
    }
}
