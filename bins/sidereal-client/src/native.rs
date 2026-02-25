#[path = "auth_ui.rs"]
mod auth_ui;
#[path = "dialog_ui.rs"]
mod dialog_ui;

use avian3d::prelude::*;
use bevy::asset::{AssetApp, AssetPlugin};
use bevy::camera::visibility::RenderLayers;
use bevy::input::mouse::MouseWheel;
use bevy::log::{info, warn};
use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy::render::RenderPlugin;
use bevy::render::render_resource::AsBindGroup;
use bevy::render::settings::{Backends, RenderCreation, WgpuSettings};
use bevy::scene::ScenePlugin;
use bevy::shader::ShaderRef;
use bevy::sprite_render::{
    AlphaMode2d, ColorMaterial, Material2d, Material2dPlugin, MeshMaterial2d,
};
use bevy::state::state_scoped::DespawnOnExit;
use bevy::window::{PresentMode, Window, WindowPlugin};

use crate::client::input::{neutral_player_input, player_input_from_keyboard};
use bevy_remote::RemotePlugin;
use bevy_remote::http::RemoteHttpPlugin;
use lightyear::avian3d::plugin::AvianReplicationMode;
use lightyear::avian3d::prelude::LightyearAvianPlugin;
use lightyear::prediction::correction::CorrectionPolicy;
use lightyear::prediction::prelude::PredictionManager;
use lightyear::prelude::client::ClientPlugins;
use lightyear::prelude::client::{Client, Connect, Connected, RawClient};
use lightyear::prelude::input::native::{ActionState, InputMarker};
use lightyear::prelude::{
    ChannelRegistry, LocalAddr, MessageManager, MessageReceiver, MessageSender, PeerAddr,
    ReplicationReceiver, Transport, UdpIo,
};
use sidereal_asset_runtime::{
    AssetCacheIndex, AssetCacheIndexRecord, cache_index_path, load_cache_index, save_cache_index,
    sha256_hex,
};
use sidereal_core::remote_inspect::RemoteInspectConfig;
use sidereal_game::{
    ActionQueue, ControlledEntityGuid, EntityAction, EntityGuid, FocusedEntityGuid,
    FullscreenLayer, Hardpoint, HealthPool, MountedOn, OwnerId, PlayerTag, ScannerRangeM,
    SelectedEntityGuid, SiderealGamePlugin, SizeM, TotalMassKg, angular_inertia_from_size,
    default_corvette_asset_id, default_corvette_mass_kg, default_corvette_size,
    default_flight_action_capabilities, default_space_background_shader_asset_id,
    default_starfield_shader_asset_id,
};
use sidereal_net::{
    AssetAckMessage, AssetRequestMessage, AssetStreamChunkMessage, AssetStreamManifestMessage,
    ClientAuthMessage, ClientViewUpdateMessage, ControlChannel, PlayerInput, RequestedAsset,
    register_lightyear_protocol,
};
use sidereal_runtime_sync::{
    RuntimeEntityHierarchy, parse_guid_from_entity_id, register_runtime_entity,
};
use std::collections::{HashMap, HashSet, VecDeque};
use std::net::SocketAddr;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

#[derive(Debug, Resource, Clone)]
#[allow(dead_code)]
struct BrpAuthToken(String);

#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[states(scoped_entities)]
enum ClientAppState {
    #[default]
    Auth,
    CharacterSelect,
    InWorld,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AuthAction {
    Login,
    Register,
    ForgotRequest,
    ForgotConfirm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FocusField {
    Email,
    Password,
    ResetToken,
    NewPassword,
}

#[derive(Debug, Resource)]
struct ClientSession {
    gateway_url: String,
    selected_action: AuthAction,
    focus: FocusField,
    email: String,
    password: String,
    reset_token: String,
    new_password: String,
    access_token: Option<String>,
    refresh_token: Option<String>,
    account_id: Option<String>,
    player_entity_id: Option<String>,
    status: String,
    ui_dirty: bool,
}

#[derive(Debug, Resource, Default)]
struct CharacterSelectionState {
    characters: Vec<String>,
    selected_player_entity_id: Option<String>,
}

#[derive(Debug, Resource, Default)]
struct ClientNetworkTick(u64);

#[derive(Debug, Resource, Default)]
struct ClientInputAckTracker {
    pending_ticks: VecDeque<u64>,
}

#[derive(Debug, Resource, Default)]
struct ClientInputLogState {
    last_logged_at_s: f64,
}

#[derive(Debug, Resource, Default)]
struct ClientAuthSyncState {
    sent_for_client_entities: std::collections::HashSet<Entity>,
    last_sent_at_s_by_client_entity: HashMap<Entity, f64>,
    last_player_entity_id: Option<String>,
}

#[derive(Debug, Resource, Default)]
struct ClientViewUpdateTick(u64);

#[derive(Debug, Resource, Default)]
struct CameraFocusState {
    focused_entity_id: Option<String>,
}

impl CameraFocusState {
    fn set(&mut self, entity_id: Option<String>) {
        self.focused_entity_id = entity_id;
    }
}

#[derive(Debug, Resource, Default)]
struct LocalPlayerViewState {
    controlled_entity_id: Option<String>,
    selected_entity_id: Option<String>,
    focused_entity_id: Option<String>,
    desired_controlled_entity_id: Option<String>,
    detached_free_camera: bool,
}

#[derive(Debug, Resource, Default)]
struct FreeCameraState {
    position_xy: Vec2,
    initialized: bool,
}

#[derive(Debug, Resource, Default)]
struct OwnedShipsPanelState {
    last_ship_ids: Vec<String>,
    last_selected_id: Option<String>,
    last_detached_mode: bool,
}

#[derive(Debug, Clone, Default)]
struct PendingAssetChunks {
    relative_cache_path: String,
    byte_len: u64,
    chunk_count: u32,
    chunks: Vec<Option<Vec<u8>>>,
    counts_toward_bootstrap: bool,
}

#[derive(Debug, Clone, Default)]
struct LocalAssetRecord {
    relative_cache_path: String,
    _content_type: String,
    _byte_len: u64,
    _chunk_count: u32,
    asset_version: u64,
    sha256_hex: String,
    ready: bool,
}

#[derive(Debug, Resource, Default)]
struct LocalAssetManager {
    records_by_asset_id: HashMap<String, LocalAssetRecord>,
    pending_assets: HashMap<String, PendingAssetChunks>,
    requested_asset_ids: std::collections::HashSet<String>,
    cache_index: AssetCacheIndex,
    cache_index_loaded: bool,
    bootstrap_manifest_seen: bool,
    bootstrap_phase_complete: bool,
    bootstrap_total_bytes: u64,
    bootstrap_ready_bytes: u64,
}

impl LocalAssetManager {
    fn bootstrap_complete(&self) -> bool {
        self.bootstrap_phase_complete
    }

    fn bootstrap_progress(&self) -> f32 {
        if self.bootstrap_total_bytes == 0 {
            return if self.bootstrap_manifest_seen {
                1.0
            } else {
                0.0
            };
        }
        (self.bootstrap_ready_bytes as f32 / self.bootstrap_total_bytes as f32).clamp(0.0, 1.0)
    }

    fn cached_relative_path(&self, asset_id: &str) -> Option<&str> {
        self.records_by_asset_id
            .get(asset_id)
            .filter(|record| record.ready)
            .map(|record| record.relative_cache_path.as_str())
    }

    fn should_show_runtime_stream_indicator(&self) -> bool {
        self.bootstrap_complete() && !self.pending_assets.is_empty()
    }

    fn is_cache_fresh(&self, asset_id: &str, asset_version: u64, sha256_hex: &str) -> bool {
        self.cache_index
            .by_asset_id
            .get(asset_id)
            .is_some_and(|entry| {
                entry.asset_version == asset_version && entry.sha256_hex == sha256_hex
            })
    }
}

#[derive(Debug, Resource, Default)]
struct RuntimeAssetStreamIndicatorState {
    blinking_phase_s: f32,
}

#[derive(Debug, Resource, Default)]
struct CriticalAssetRequestState {
    last_request_at_s: f64,
}

#[derive(Debug, Resource, Default)]
struct DebugBlueOverlayEnabled(bool);

/// When true, F3 debug overlay is active: collision AABB wireframes, ship AABB + velocity arrow, hardpoint markers.
#[derive(Debug, Resource, Default)]
struct DebugOverlayEnabled {
    enabled: bool,
}

#[derive(Debug, Resource, Default)]
struct StarfieldMotionState {
    prev_speed: f32,
    initialized: bool,
    starfield_drift_uv: Vec2,
    background_drift_uv: Vec2,
    smoothed_warp: f32,
}

#[derive(Debug, Resource)]
struct CameraMotionState {
    world_position_xy: Vec2,
    smoothed_position_xy: Vec2,
    prev_position_xy: Vec2,
    frame_delta_xy: Vec2,
    smoothed_velocity_xy: Vec2,
    initialized: bool,
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
struct BootstrapWatchdogState {
    in_world_entered_at_s: Option<f64>,
    replication_state_seen: bool,
    asset_manifest_seen: bool,
    last_bootstrap_ready_bytes: u64,
    last_bootstrap_progress_at_s: f64,
    timeout_dialog_shown: bool,
    stream_stall_dialog_shown: bool,
    no_world_state_dialog_shown: bool,
}

#[derive(Debug, Resource, Default)]
struct DeferredPredictedAdoptionState {
    waiting_entity_id: Option<String>,
    wait_started_at_s: Option<f64>,
    last_warn_at_s: f64,
    last_missing_components: String,
    dialog_shown: bool,
    resolved_samples: u64,
    resolved_total_wait_s: f64,
    resolved_max_wait_s: f64,
    last_summary_at_s: f64,
    last_runtime_summary_at_s: f64,
}

#[derive(Debug, Resource, Clone, Copy)]
struct PredictionBootstrapTuning {
    defer_warn_after_s: f64,
    defer_warn_interval_s: f64,
    defer_dialog_after_s: f64,
    defer_summary_interval_s: f64,
}

impl PredictionBootstrapTuning {
    fn from_env() -> Self {
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
struct PredictionCorrectionTuning {
    max_rollback_ticks: u16,
    instant_correction: bool,
}

impl PredictionCorrectionTuning {
    fn from_env() -> Self {
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
        }
    }
}

#[derive(Resource, Debug, Clone, Copy)]
struct HeadlessTransportMode(bool);

#[derive(Resource, Debug)]
struct HeadlessAccountSwitchPlan {
    switch_after_s: f64,
    switched: bool,
    next_player_entity_id: String,
    next_access_token: String,
}

impl Default for ClientSession {
    fn default() -> Self {
        Self {
            gateway_url: std::env::var("GATEWAY_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:8080".to_string()),
            selected_action: AuthAction::Login,
            focus: FocusField::Email,
            email: "pilot@example.com".to_string(),
            password: "very-strong-password".to_string(),
            reset_token: String::new(),
            new_password: "new-very-strong-password".to_string(),
            access_token: None,
            refresh_token: None,
            account_id: None,
            player_entity_id: None,
            status: "Ready. F1 Login, F2 Register, F3 Forgot Request, F4 Forgot Confirm."
                .to_string(),
            ui_dirty: true,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct RegisterRequest {
    email: String,
    password: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct LoginRequest {
    email: String,
    password: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct ForgotRequest {
    email: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct ForgotConfirmRequest {
    reset_token: String,
    new_password: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct AuthTokens {
    access_token: String,
    refresh_token: String,
    token_type: String,
    expires_in_s: u64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct ForgotResponse {
    accepted: bool,
    reset_token: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct ForgotConfirmResponse {
    accepted: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct AuthMeResponse {
    account_id: String,
    email: String,
    player_entity_id: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct CharactersResponse {
    characters: Vec<CharacterSummary>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct CharacterSummary {
    player_entity_id: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct EnterWorldRequest {
    player_entity_id: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct EnterWorldResponse {
    accepted: bool,
}

#[derive(Resource, Clone)]
struct AssetRootPath(String);

#[derive(Resource, Clone)]
struct EmbeddedFonts {
    bold: Handle<Font>,
    regular: Handle<Font>,
}

#[derive(Component)]
struct WorldEntity;
#[derive(Component)]
struct HudText;
#[derive(Component)]
struct LoadingOverlayText;
#[derive(Component)]
struct LoadingProgressBarFill;
#[derive(Component)]
struct LoadingOverlayRoot;
#[derive(Component)]
struct RuntimeStreamingIconText;
#[derive(Component)]
struct GameplayCamera;
#[derive(Component)]
struct GameplayHud;
#[derive(Component)]
struct UiOverlayCamera;
#[derive(Component)]
struct CharacterSelectRoot;
#[derive(Component)]
struct CharacterSelectStatusText;
#[derive(Component)]
struct CharacterSelectButton {
    player_entity_id: String,
}
#[derive(Component)]
struct CharacterSelectEnterButton;
#[derive(Component)]
struct OwnedShipsPanelRoot;
#[derive(Component)]
struct OwnedShipsPanelButton {
    action: OwnedShipsPanelAction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum OwnedShipsPanelAction {
    FreeRoam,
    ControlEntity(String),
}

#[derive(Component)]
struct ControlledEntity {
    entity_id: String,
    #[allow(dead_code)]
    player_entity_id: String,
}

#[derive(Component)]
struct RemoteVisibleEntity {
    #[allow(dead_code)]
    entity_id: String,
}

#[derive(Component)]
struct RemoteEntity;

#[derive(Component, Clone)]
struct StreamedModelAssetId(String);

#[derive(Component)]
struct StreamedModelVisualAttached;

#[derive(Resource, Default)]
struct RemoteEntityRegistry {
    by_entity_id: HashMap<String, Entity>,
}

fn should_defer_controlled_predicted_adoption(
    is_local_controlled: bool,
    has_position: bool,
    has_rotation: bool,
    has_linear_velocity: bool,
) -> bool {
    is_local_controlled && (!has_position || !has_rotation || !has_linear_velocity)
}

fn candidate_runtime_entity_score(
    is_root_entity: bool,
    is_local_controlled_entity: bool,
    predicted_mode: bool,
) -> i32 {
    if is_local_controlled_entity {
        if predicted_mode { 500 } else { 400 }
    } else if is_root_entity {
        if predicted_mode { 200 } else { 100 }
    } else {
        50
    }
}

fn runtime_entity_id_from_guid(
    entity_registry: &RuntimeEntityHierarchy,
    local_player_entity_id: &str,
    guid: &str,
) -> Option<String> {
    if parse_guid_from_entity_id(local_player_entity_id)
        .is_some_and(|player_guid| player_guid.to_string() == guid)
    {
        return Some(local_player_entity_id.to_string());
    }
    for prefix in ["ship", "player", "module", "hardpoint"] {
        let candidate = format!("{prefix}:{guid}");
        if entity_registry.by_entity_id.contains_key(&candidate) {
            return Some(candidate);
        }
    }
    None
}

fn existing_runtime_entity_score(
    is_world_entity: bool,
    is_controlled: bool,
    is_predicted: bool,
    is_interpolated: bool,
    is_remote: bool,
) -> i32 {
    if is_controlled {
        if is_predicted { 500 } else { 400 }
    } else if is_remote {
        if is_predicted {
            200
        } else if is_interpolated {
            100
        } else {
            90
        }
    } else if is_world_entity {
        80
    } else {
        0
    }
}

#[derive(Debug, Clone, Copy, Resource, Default)]
struct LocalSimulationDebugMode(bool);

impl LocalSimulationDebugMode {
    fn from_env() -> Self {
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

const BACKDROP_RENDER_LAYER: usize = 1;

#[derive(Component)]
struct StarfieldBackdrop;

#[derive(Component)]
struct SpaceBackgroundBackdrop;

#[derive(Component)]
struct DebugBlueBackdrop;

#[derive(Component)]
struct SpaceBackdropFallback;

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
struct StarfieldMaterial {
    #[uniform(0)]
    viewport_time: Vec4,
    #[uniform(1)]
    drift_intensity: Vec4,
    #[uniform(2)]
    velocity_dir: Vec4,
}

impl Default for StarfieldMaterial {
    fn default() -> Self {
        Self {
            viewport_time: Vec4::new(1920.0, 1080.0, 0.0, 0.0),
            drift_intensity: Vec4::new(0.0, 0.0, 1.0, 1.0),
            velocity_dir: Vec4::new(0.0, 1.0, 0.0, 0.0),
        }
    }
}

impl Material2d for StarfieldMaterial {
    fn fragment_shader() -> ShaderRef {
        "data/cache_stream/shaders/starfield.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
struct SpaceBackgroundMaterial {
    #[uniform(0)]
    viewport_time: Vec4,
    #[uniform(1)]
    colors: Vec4,
    #[uniform(2)]
    motion: Vec4,
}

#[derive(Component)]
struct FullscreenLayerRenderable {
    layer_kind: String,
    layer_order: i32,
}

#[derive(Component)]
struct FallbackFullscreenLayer;

impl Default for SpaceBackgroundMaterial {
    fn default() -> Self {
        Self {
            viewport_time: Vec4::new(1920.0, 1080.0, 0.0, 1.0),
            colors: Vec4::new(0.05, 0.08, 0.15, 1.0),
            motion: Vec4::ZERO,
        }
    }
}

impl Material2d for SpaceBackgroundMaterial {
    fn fragment_shader() -> ShaderRef {
        "data/cache_stream/shaders/simple_space_background.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Opaque
    }
}

#[derive(Component)]
struct TopDownCamera {
    distance: f32,
    target_distance: f32,
    min_distance: f32,
    max_distance: f32,
    zoom_units_per_wheel: f32,
    zoom_smoothness: f32,
    look_ahead_offset: Vec2,
    filtered_focus_xy: Vec2,
    focus_initialized: bool,
}

#[derive(Resource, Debug)]
/// Caps client frame rate when set. Configure via `SIDEREAL_CLIENT_MAX_FPS` (default 60; 0 = disabled).
struct FrameRateCap {
    frame_duration: Duration,
    last_frame_end: Instant,
}

impl FrameRateCap {
    fn from_env(default_fps: u32) -> Option<Self> {
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

pub(crate) fn run() {
    let headless_transport = std::env::var("SIDEREAL_CLIENT_HEADLESS")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let remote_cfg = match RemoteInspectConfig::from_env("CLIENT", 15714) {
        Ok(cfg) => cfg,
        Err(err) => {
            eprintln!("invalid CLIENT BRP config: {err}");
            std::process::exit(2);
        }
    };

    let asset_root = std::env::var("SIDEREAL_ASSET_ROOT").unwrap_or_else(|_| ".".to_string());

    let mut app = App::new();
    if headless_transport {
        app.add_plugins(MinimalPlugins);
        app.add_plugins(bevy::log::LogPlugin::default());
        app.add_plugins(AssetPlugin::default());
        app.add_plugins(ScenePlugin);
        // Avian's collider cache reads mesh asset events even in headless mode.
        app.add_message::<bevy::asset::AssetEvent<Mesh>>();
        app.init_asset::<Mesh>();
    } else {
        app.insert_resource(ClearColor(Color::BLACK));
        app.add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        present_mode: PresentMode::AutoVsync,
                        ..default()
                    }),
                    ..default()
                })
                .set(bevy::asset::AssetPlugin {
                    file_path: asset_root.clone(),
                    ..Default::default()
                })
                .set(RenderPlugin {
                    render_creation: RenderCreation::Automatic(configured_wgpu_settings()),
                    ..Default::default()
                }),
        );
        ensure_shader_placeholders(&asset_root);
        app.add_plugins(Material2dPlugin::<StarfieldMaterial>::default());
        app.add_plugins(Material2dPlugin::<SpaceBackgroundMaterial>::default());
        // FPS cap: SIDEREAL_CLIENT_MAX_FPS (default 60). Set to 0 to disable (uncapped).
        if let Some(frame_cap) = FrameRateCap::from_env(60) {
            app.insert_resource(frame_cap);
            app.add_systems(Last, enforce_frame_rate_cap_system);
        }
    }

    app.add_plugins(
        PhysicsPlugins::default()
            .with_length_unit(1.0)
            .build()
            .disable::<PhysicsTransformPlugin>()
            .disable::<PhysicsInterpolationPlugin>(),
    );
    app.insert_resource(Gravity(Vec3::ZERO));
    // Predicted mode must run full gameplay systems so rollback can resimulate real flight/mass.
    // Local mode remains a debug path and also uses full gameplay systems.
    app.add_plugins(SiderealGamePlugin);
    app.add_plugins(ClientPlugins {
        tick_duration: Duration::from_secs_f64(1.0 / 30.0),
    });
    app.add_plugins(LightyearAvianPlugin {
        replication_mode: AvianReplicationMode::Position,
        update_syncs_manually: false,
        rollback_resources: false,
        rollback_islands: false,
    });
    register_lightyear_protocol(&mut app);
    configure_remote(&mut app, &remote_cfg);
    // Lightyear/Bevy plugins can initialize Fixed time; set project-authoritative 30 Hz after plugin wiring.
    app.insert_resource(Time::<Fixed>::from_hz(30.0));
    app.insert_resource(AssetRootPath(asset_root));
    app.insert_resource(LocalSimulationDebugMode::from_env());
    app.insert_resource(ClientSession::default());
    app.insert_resource(ClientNetworkTick::default());
    app.insert_resource(ClientInputAckTracker::default());
    app.insert_resource(ClientInputLogState::default());
    app.insert_resource(ClientAuthSyncState::default());
    app.insert_resource(ClientViewUpdateTick::default());
    app.insert_resource(LocalAssetManager::default());
    app.insert_resource(RuntimeAssetStreamIndicatorState::default());
    app.insert_resource(CriticalAssetRequestState::default());
    let debug_blue_overlay = std::env::var("SIDEREAL_DEBUG_BLUE_FULLSCREEN")
        .is_ok_and(|v| v == "1" || v.eq_ignore_ascii_case("true"));
    app.insert_resource(DebugBlueOverlayEnabled(debug_blue_overlay));
    app.insert_resource(DebugOverlayEnabled { enabled: false });
    app.insert_resource(CameraFocusState::default());
    app.insert_resource(LocalPlayerViewState::default());
    app.insert_resource(CharacterSelectionState::default());
    app.insert_resource(FreeCameraState::default());
    app.insert_resource(OwnedShipsPanelState::default());
    app.insert_resource(RuntimeEntityHierarchy::default());
    app.insert_resource(StarfieldMotionState::default());
    app.insert_resource(CameraMotionState::default());
    app.insert_resource(BootstrapWatchdogState::default());
    app.insert_resource(DeferredPredictedAdoptionState::default());
    app.insert_resource(PredictionBootstrapTuning::from_env());
    app.insert_resource(PredictionCorrectionTuning::from_env());
    app.insert_resource(RemoteEntityRegistry::default());
    app.insert_resource(HeadlessTransportMode(headless_transport));
    if headless_transport {
        app.init_resource::<dialog_ui::DialogQueue>();
    }
    app.add_observer(log_native_client_connected);
    if headless_transport {
        app.add_systems(Startup, start_lightyear_client_transport);
    }
    if !headless_transport {
        app.add_systems(Startup, spawn_ui_overlay_camera);
    }

    if headless_transport {
        app.add_systems(Startup, configure_headless_session_from_env);
        app.add_systems(
            FixedPreUpdate,
            send_lightyear_input_messages
                .in_set(lightyear::prelude::client::input::InputSystems::WriteClientInputs),
        );
        app.add_systems(
            Update,
            (
                apply_headless_account_switch_system,
                configure_prediction_manager_tuning,
                ensure_client_transport_channels,
                send_lightyear_auth_messages,
                receive_lightyear_asset_stream_messages,
                ensure_critical_assets_available_system
                    .after(receive_lightyear_asset_stream_messages),
                adopt_native_lightyear_replicated_entities,
                sync_world_entity_transforms_from_physics
                    .after(adopt_native_lightyear_replicated_entities),
                sync_local_player_view_state_system
                    .after(adopt_native_lightyear_replicated_entities),
                sync_controlled_entity_tags_system.after(sync_local_player_view_state_system),
                update_focus_target_system,
                send_lightyear_view_updates.after(update_focus_target_system),
                log_prediction_runtime_state,
            ),
        );
        app.add_systems(Startup, || {
            info!("sidereal-client headless transport mode");
        });
    } else {
        insert_embedded_fonts(&mut app);
        app.init_state::<ClientAppState>();
        app.add_systems(
            OnEnter(ClientAppState::CharacterSelect),
            setup_character_select_screen,
        );
        auth_ui::register_auth_ui(&mut app);
        dialog_ui::register_dialog_ui(&mut app);
        app.add_systems(
            OnEnter(ClientAppState::InWorld),
            (
                ensure_lightyear_client_system,
                spawn_world_scene,
                reset_bootstrap_watchdog_on_enter_in_world,
            )
                .chain(),
        );
        app.add_systems(
            Update,
            (
                handle_character_select_buttons,
                ensure_client_transport_channels,
                configure_prediction_manager_tuning,
                send_lightyear_auth_messages,
                receive_lightyear_asset_stream_messages,
                ensure_critical_assets_available_system
                    .after(receive_lightyear_asset_stream_messages),
                adopt_native_lightyear_replicated_entities,
                sync_local_player_view_state_system
                    .after(adopt_native_lightyear_replicated_entities),
                sync_controlled_entity_tags_system.after(sync_local_player_view_state_system),
                update_focus_target_system,
                send_lightyear_view_updates.after(update_focus_target_system),
                log_prediction_runtime_state,
            ),
        );
        app.add_systems(
            Update,
            (
                ensure_fullscreen_layer_fallback_system
                    .after(adopt_native_lightyear_replicated_entities),
                attach_streamed_model_visuals_system.after(receive_lightyear_asset_stream_messages),
                sync_fullscreen_layer_renderables_system
                    .after(adopt_native_lightyear_replicated_entities),
                sync_backdrop_fullscreen_system.after(sync_fullscreen_layer_renderables_system),
                gate_gameplay_camera_system,
                update_owned_ships_panel_system,
                handle_owned_ships_panel_buttons,
                update_loading_overlay_system,
                update_runtime_stream_icon_system,
                watch_in_world_bootstrap_failures,
                update_topdown_camera_system.after(adopt_native_lightyear_replicated_entities),
                update_camera_motion_state.after(update_topdown_camera_system),
                update_hud_system,
                update_starfield_material_system.after(update_camera_motion_state),
                update_space_background_material_system.after(update_camera_motion_state),
                toggle_debug_overlay_system,
                draw_debug_overlay_system.after(toggle_debug_overlay_system),
            )
                .run_if(in_state(ClientAppState::InWorld)),
        );
        app.add_systems(
            Last,
            lock_camera_to_controlled_entity_end_of_frame.run_if(in_state(ClientAppState::InWorld)),
        );
        app.add_systems(
            FixedPreUpdate,
            send_lightyear_input_messages
                .in_set(lightyear::prelude::client::input::InputSystems::WriteClientInputs)
                .run_if(in_state(ClientAppState::InWorld)),
        );
        app.add_systems(
            PreUpdate,
            logout_to_auth_system.run_if(in_state(ClientAppState::InWorld)),
        );
        app.add_systems(
            PreUpdate,
            logout_to_auth_system.run_if(in_state(ClientAppState::CharacterSelect)),
        );
        app.add_systems(
            FixedUpdate,
            (
                apply_predicted_input_to_action_queue,
                enforce_controlled_planar_motion,
            )
                .chain()
                .before(avian3d::prelude::PhysicsSystems::StepSimulation)
                .run_if(in_state(ClientAppState::InWorld)),
        );
    }
    app.run();
}

fn configure_headless_session_from_env(
    mut commands: Commands<'_, '_>,
    mut session: ResMut<'_, ClientSession>,
) {
    if let Ok(player_entity_id) = std::env::var("SIDEREAL_CLIENT_HEADLESS_PLAYER_ENTITY_ID") {
        session.player_entity_id = Some(player_entity_id);
    }
    if let Ok(access_token) = std::env::var("SIDEREAL_CLIENT_HEADLESS_ACCESS_TOKEN") {
        session.access_token = Some(access_token);
    }
    let next_player = std::env::var("SIDEREAL_CLIENT_HEADLESS_SWITCH_PLAYER_ENTITY_ID").ok();
    let next_token = std::env::var("SIDEREAL_CLIENT_HEADLESS_SWITCH_ACCESS_TOKEN").ok();
    if let (Some(next_player_entity_id), Some(next_access_token)) = (next_player, next_token) {
        let switch_after_s = std::env::var("SIDEREAL_CLIENT_HEADLESS_SWITCH_AFTER_S")
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(1.0)
            .max(0.0);
        commands.insert_resource(HeadlessAccountSwitchPlan {
            switch_after_s,
            switched: false,
            next_player_entity_id,
            next_access_token,
        });
    }
}

fn apply_headless_account_switch_system(
    time: Res<'_, Time>,
    mut session: ResMut<'_, ClientSession>,
    plan: Option<ResMut<'_, HeadlessAccountSwitchPlan>>,
) {
    let Some(mut plan) = plan else {
        return;
    };
    if plan.switched || time.elapsed_secs_f64() < plan.switch_after_s {
        return;
    }
    session.player_entity_id = Some(plan.next_player_entity_id.clone());
    session.access_token = Some(plan.next_access_token.clone());
    plan.switched = true;
    info!(
        "headless account switch applied player_entity_id={}",
        plan.next_player_entity_id
    );
}

fn spawn_ui_overlay_camera(mut commands: Commands<'_, '_>) {
    commands.spawn((
        Camera2d,
        Camera {
            // Keep UI rendering independent from auth/world camera lifecycles.
            order: 100,
            clear_color: ClearColorConfig::None,
            ..default()
        },
        UiOverlayCamera,
    ));
}

fn insert_embedded_fonts(app: &mut App) {
    static BOLD: &[u8] = include_bytes!("../../../data/fonts/FiraSans-Bold.ttf");
    static REGULAR: &[u8] = include_bytes!("../../../data/fonts/FiraSans-Regular.ttf");

    let mut fonts = app.world_mut().resource_mut::<Assets<Font>>();
    let bold = fonts
        .add(Font::try_from_bytes(BOLD.to_vec()).expect("embedded FiraSans-Bold.ttf is valid"));
    let regular = fonts.add(
        Font::try_from_bytes(REGULAR.to_vec()).expect("embedded FiraSans-Regular.ttf is valid"),
    );
    app.insert_resource(EmbeddedFonts { bold, regular });
}

const STREAMED_SHADER_PATHS: &[&str] = &[
    "data/cache_stream/shaders/starfield.wgsl",
    "data/cache_stream/shaders/simple_space_background.wgsl",
];

const LOCAL_SHADER_FALLBACK_PATHS: &[&str] = &[
    "data/shaders/starfield.wgsl",
    "data/shaders/simple_space_background.wgsl",
];

fn ensure_shader_placeholders(asset_root: &str) {
    const STARFIELD_PLACEHOLDER: &str = "\
#import bevy_sprite::mesh2d_vertex_output::VertexOutput
@group(2) @binding(0) var<uniform> viewport_time: vec4<f32>;
@group(2) @binding(1) var<uniform> drift_intensity: vec4<f32>;
@group(2) @binding(2) var<uniform> velocity_dir: vec4<f32>;
@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(0.0, 0.0, 0.0, 0.0);
}
";

    const BACKGROUND_PLACEHOLDER: &str = "\
#import bevy_sprite::mesh2d_vertex_output::VertexOutput
@group(2) @binding(0) var<uniform> viewport_time: vec4<f32>;
@group(2) @binding(1) var<uniform> colors: vec4<f32>;
@group(2) @binding(2) var<uniform> motion: vec4<f32>;
@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(colors.r, colors.g, colors.b, 1.0);
}
";

    let placeholders: &[(&str, &str, &str)] = &[
        (
            STREAMED_SHADER_PATHS[0],
            LOCAL_SHADER_FALLBACK_PATHS[0],
            STARFIELD_PLACEHOLDER,
        ),
        (
            STREAMED_SHADER_PATHS[1],
            LOCAL_SHADER_FALLBACK_PATHS[1],
            BACKGROUND_PLACEHOLDER,
        ),
    ];

    for &(cache_rel_path, source_rel_path, placeholder_content) in placeholders {
        let cache_path = std::path::PathBuf::from(asset_root).join(cache_rel_path);
        if cache_path.exists() {
            continue;
        }
        if let Some(parent) = cache_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let source_path = std::path::PathBuf::from(asset_root).join(source_rel_path);
        let content = std::fs::read_to_string(&source_path)
            .ok()
            .unwrap_or_else(|| placeholder_content.to_string());
        std::fs::write(&cache_path, content).ok();
    }
}

fn reload_streamed_shaders(
    asset_server: &AssetServer,
    shaders: &mut Assets<bevy::shader::Shader>,
    asset_root: &str,
) {
    for (idx, &path) in STREAMED_SHADER_PATHS.iter().enumerate() {
        let cache_path = std::path::PathBuf::from(asset_root).join(path);
        let local_fallback_path = std::path::PathBuf::from(asset_root).join(
            LOCAL_SHADER_FALLBACK_PATHS
                .get(idx)
                .copied()
                .unwrap_or(path),
        );

        let selected_path = match (
            std::fs::metadata(&cache_path).and_then(|m| m.modified()),
            std::fs::metadata(&local_fallback_path).and_then(|m| m.modified()),
        ) {
            (Ok(cache_modified), Ok(local_modified)) if local_modified > cache_modified => {
                local_fallback_path
            }
            _ => cache_path,
        };

        if let Ok(content) = std::fs::read_to_string(&selected_path) {
            let handle: Handle<bevy::shader::Shader> = asset_server.load(path);
            let _ = shaders.insert(handle.id(), bevy::shader::Shader::from_wgsl(content, path));
        }
    }
}

fn streamed_shader_path_for_asset_id(shader_asset_id: &str) -> Option<&'static str> {
    match shader_asset_id {
        "starfield_wgsl" => Some(STREAMED_SHADER_PATHS[0]),
        "space_background_wgsl" => Some(STREAMED_SHADER_PATHS[1]),
        _ => None,
    }
}

fn fullscreen_layer_shader_ready(
    asset_root: &str,
    asset_manager: &LocalAssetManager,
    shader_asset_id: &str,
) -> bool {
    if let Some(relative_cache_path) = asset_manager.cached_relative_path(shader_asset_id) {
        let rooted_stream_path = std::path::PathBuf::from(asset_root)
            .join("data/cache_stream")
            .join(relative_cache_path);
        let rooted_direct_path = std::path::PathBuf::from(asset_root).join(relative_cache_path);
        if rooted_stream_path.exists() || rooted_direct_path.exists() {
            return true;
        }
    }

    let Some(streamed_shader_rel_path) = streamed_shader_path_for_asset_id(shader_asset_id) else {
        return false;
    };
    std::path::PathBuf::from(asset_root)
        .join(streamed_shader_rel_path)
        .exists()
}

fn configure_remote(app: &mut App, cfg: &RemoteInspectConfig) {
    if !cfg.enabled {
        return;
    }

    app.add_plugins(RemotePlugin::default());
    app.add_plugins(
        RemoteHttpPlugin::default()
            .with_address(cfg.bind_addr)
            .with_port(cfg.port),
    );
    app.insert_resource(BrpAuthToken(
        cfg.auth_token.clone().expect("validated token"),
    ));
}

/// Spawns the Lightyear client and triggers Connect if no client entity exists.
/// Used on Enter Auth so we have a connection for sending auth after (re)login.
fn ensure_lightyear_client_system(
    mut commands: Commands<'_, '_>,
    existing: Query<'_, '_, Entity, With<RawClient>>,
) {
    if existing.is_empty() {
        start_lightyear_client_transport_inner(&mut commands);
    }
}

fn start_lightyear_client_transport(mut commands: Commands<'_, '_>) {
    start_lightyear_client_transport_inner(&mut commands);
}

fn start_lightyear_client_transport_inner(commands: &mut Commands<'_, '_>) {
    let local_addr = std::env::var("CLIENT_UDP_BIND")
        .unwrap_or_else(|_| "127.0.0.1:7003".to_string())
        .parse::<SocketAddr>();
    let local_addr = match local_addr {
        Ok(v) => v,
        Err(err) => {
            eprintln!("invalid CLIENT_UDP_BIND: {err}");
            return;
        }
    };
    let remote_addr = std::env::var("REPLICATION_UDP_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:7001".to_string())
        .parse::<SocketAddr>();
    let remote_addr = match remote_addr {
        Ok(v) => v,
        Err(err) => {
            eprintln!("invalid REPLICATION_UDP_ADDR: {err}");
            return;
        }
    };

    let client = commands
        .spawn((
            Name::new("native-client-lightyear"),
            RawClient,
            UdpIo::default(),
            MessageManager::default(),
            ReplicationReceiver::default(),
            LocalAddr(local_addr),
            PeerAddr(remote_addr),
        ))
        .id();
    commands.trigger(Connect { entity: client });
    info!(
        "native client lightyear UDP connecting {} -> {}",
        local_addr, remote_addr
    );
}

fn decode_api_json<T: serde::de::DeserializeOwned>(
    response: reqwest::blocking::Response,
) -> Result<T, String> {
    let status = response.status();
    let body = response.text().map_err(|err| err.to_string())?;
    if !status.is_success() {
        if let Ok(error_json) = serde_json::from_str::<serde_json::Value>(&body)
            && let Some(message) = error_json.get("error").and_then(|v| v.as_str())
        {
            return Err(format!("{status}: {message}"));
        }
        if body.trim().is_empty() {
            return Err(status.to_string());
        }
        return Err(format!("{status}: {body}"));
    }
    serde_json::from_str::<T>(&body).map_err(|err| err.to_string())
}

fn submit_auth_request(
    session: &mut ClientSession,
    character_selection: &mut CharacterSelectionState,
    next_state: &mut NextState<ClientAppState>,
    dialog_queue: &mut dialog_ui::DialogQueue,
    _asset_root: &AssetRootPath,
) {
    let client = reqwest::blocking::Client::new();
    let gateway_url = session.gateway_url.clone();
    let result = match session.selected_action {
        AuthAction::Login => (|| -> Result<(Option<AuthTokens>, Option<String>), String> {
            let response = client
                .post(format!("{gateway_url}/auth/login"))
                .json(&LoginRequest {
                    email: session.email.clone(),
                    password: session.password.clone(),
                })
                .send()
                .map_err(|err| err.to_string())?;
            let tokens = decode_api_json::<AuthTokens>(response)?;
            session.status = "Login succeeded. Fetching world snapshot...".to_string();
            Ok((Some(tokens), None::<String>))
        })(),
        AuthAction::Register => (|| -> Result<(Option<AuthTokens>, Option<String>), String> {
            let response = client
                .post(format!("{gateway_url}/auth/register"))
                .json(&RegisterRequest {
                    email: session.email.clone(),
                    password: session.password.clone(),
                })
                .send()
                .map_err(|err| err.to_string())?;
            let tokens = decode_api_json::<AuthTokens>(response)?;
            session.status = "Registration succeeded. Fetching world snapshot...".to_string();
            Ok((Some(tokens), None::<String>))
        })(),
        AuthAction::ForgotRequest => {
            (|| -> Result<(Option<AuthTokens>, Option<String>), String> {
                let response = client
                    .post(format!("{gateway_url}/auth/password-reset/request"))
                    .json(&ForgotRequest {
                        email: session.email.clone(),
                    })
                    .send()
                    .map_err(|err| err.to_string())?;
                let resp = decode_api_json::<ForgotResponse>(response)?;
                if let Some(token) = resp.reset_token {
                    session.reset_token = token;
                }
                session.status =
                    "Password reset token requested. Use F4 to confirm reset.".to_string();
                Ok((None, None::<String>))
            })()
        }
        AuthAction::ForgotConfirm => {
            (|| -> Result<(Option<AuthTokens>, Option<String>), String> {
                let response = client
                    .post(format!("{gateway_url}/auth/password-reset/confirm"))
                    .json(&ForgotConfirmRequest {
                        reset_token: session.reset_token.clone(),
                        new_password: session.new_password.clone(),
                    })
                    .send()
                    .map_err(|err| err.to_string())?;
                let _ = decode_api_json::<ForgotConfirmResponse>(response)?;
                session.status = "Password reset confirmed. Switch to Login (F1).".to_string();
                Ok((None, None::<String>))
            })()
        }
    };

    match result {
        Ok((Some(tokens), _)) => {
            session.access_token = Some(tokens.access_token.clone());
            session.refresh_token = Some(tokens.refresh_token);
            match fetch_auth_me(&client, &gateway_url, &tokens.access_token) {
                Ok(me) => {
                    session.account_id = Some(me.account_id.clone());
                    match fetch_auth_characters(&client, &gateway_url, &tokens.access_token) {
                        Ok(characters) => {
                            character_selection.characters = characters
                                .characters
                                .into_iter()
                                .map(|c| c.player_entity_id)
                                .collect();
                            if character_selection.characters.is_empty() {
                                session.status =
                                    "Authenticated but no characters are available.".to_string();
                                dialog_queue.push_error(
                                    "No Characters",
                                    "This account has no characters. Character creation UI is not implemented yet."
                                        .to_string(),
                                );
                                return;
                            }
                            character_selection.selected_player_entity_id =
                                character_selection.characters.first().cloned();
                            session.player_entity_id = None;
                            session.status =
                                "Authenticated. Select a character and press Enter World."
                                    .to_string();
                            next_state.set(ClientAppState::CharacterSelect);
                        }
                        Err(err) => {
                            session.status = format!("Auth OK but character lookup failed: {err}");
                            dialog_queue.push_error(
                                "Character Lookup Failed",
                                format!(
                                    "Authentication succeeded, but failed to fetch /auth/characters.\n\nDetails: {err}"
                                ),
                            );
                        }
                    }
                }
                Err(err) => {
                    session.status = format!("Auth OK but profile lookup failed: {err}");
                    dialog_queue.push_error(
                        "Profile Lookup Failed",
                        format!(
                            "Authentication succeeded, but failed to fetch /auth/me.\n\n\
                             Details: {err}\n\n\
                             This usually means:\n\
                             • Backend server needs to be restarted/recompiled\n\
                             • Protocol version mismatch between client and server\n\
                             • Network connectivity issue"
                        ),
                    );
                }
            }
        }
        Ok((None, _)) => {}
        Err(err) => {
            session.status = format!("Request failed: {err}");
            dialog_queue.push_error(
                "Authentication Failed",
                format!("Failed to connect or authenticate.\n\nDetails: {err}"),
            );
        }
    }
    session.ui_dirty = true;
}

fn fetch_auth_me(
    client: &reqwest::blocking::Client,
    gateway_url: &str,
    access_token: &str,
) -> Result<AuthMeResponse, String> {
    client
        .get(format!("{gateway_url}/auth/me"))
        .bearer_auth(access_token)
        .send()
        .map_err(|err| err.to_string())?
        .error_for_status()
        .map_err(|err| err.to_string())?
        .json::<AuthMeResponse>()
        .map_err(|err| err.to_string())
}

fn fetch_auth_characters(
    client: &reqwest::blocking::Client,
    gateway_url: &str,
    access_token: &str,
) -> Result<CharactersResponse, String> {
    client
        .get(format!("{gateway_url}/auth/characters"))
        .bearer_auth(access_token)
        .send()
        .map_err(|err| err.to_string())?
        .error_for_status()
        .map_err(|err| err.to_string())?
        .json::<CharactersResponse>()
        .map_err(|err| err.to_string())
}

fn enter_world_request(
    client: &reqwest::blocking::Client,
    gateway_url: &str,
    access_token: &str,
    player_entity_id: &str,
) -> Result<EnterWorldResponse, String> {
    client
        .post(format!("{gateway_url}/world/enter"))
        .bearer_auth(access_token)
        .json(&EnterWorldRequest {
            player_entity_id: player_entity_id.to_string(),
        })
        .send()
        .map_err(|err| err.to_string())?
        .error_for_status()
        .map_err(|err| err.to_string())?
        .json::<EnterWorldResponse>()
        .map_err(|err| err.to_string())
}

fn setup_character_select_screen(
    mut commands: Commands<'_, '_>,
    fonts: Res<'_, EmbeddedFonts>,
    character_selection: Res<'_, CharacterSelectionState>,
) {
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            CharacterSelectRoot,
            DespawnOnExit(ClientAppState::CharacterSelect),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: Val::Px(560.0),
                    padding: UiRect::all(Val::Px(24.0)),
                    border: UiRect::all(Val::Px(2.0)),
                    border_radius: BorderRadius::all(Val::Px(12.0)),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(12.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.06, 0.08, 0.12, 0.95)),
                BorderColor::all(Color::srgba(0.2, 0.3, 0.45, 0.8)),
            ))
            .with_children(|panel| {
                panel.spawn((
                    Text::new("Character Select"),
                    TextFont {
                        font: fonts.bold.clone(),
                        font_size: 34.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.85, 0.92, 1.0)),
                ));
                panel.spawn((
                    Text::new("Choose a character, then Enter World."),
                    TextFont {
                        font: fonts.regular.clone(),
                        font_size: 15.0,
                        ..default()
                    },
                    TextColor(Color::srgba(0.78, 0.84, 0.92, 0.95)),
                ));

                for player_entity_id in &character_selection.characters {
                    panel
                        .spawn((
                            Button,
                            CharacterSelectButton {
                                player_entity_id: player_entity_id.clone(),
                            },
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Px(38.0),
                                justify_content: JustifyContent::FlexStart,
                                align_items: AlignItems::Center,
                                padding: UiRect::axes(Val::Px(10.0), Val::Px(0.0)),
                                border_radius: BorderRadius::all(Val::Px(7.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.14, 0.18, 0.24, 0.9)),
                        ))
                        .with_children(|button| {
                            button.spawn((
                                Text::new(player_entity_id.clone()),
                                TextFont {
                                    font: fonts.regular.clone(),
                                    font_size: 14.0,
                                    ..default()
                                },
                                TextColor(Color::srgb(0.92, 0.95, 1.0)),
                            ));
                        });
                }

                panel
                    .spawn((
                        Button,
                        CharacterSelectEnterButton,
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Px(44.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            border_radius: BorderRadius::all(Val::Px(8.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.2, 0.46, 0.85)),
                    ))
                    .with_children(|button| {
                        button.spawn((
                            Text::new("Enter World"),
                            TextFont {
                                font: fonts.bold.clone(),
                                font_size: 17.0,
                                ..default()
                            },
                            TextColor(Color::WHITE),
                        ));
                    });

                panel.spawn((
                    Text::new(""),
                    TextFont {
                        font: fonts.regular.clone(),
                        font_size: 13.0,
                        ..default()
                    },
                    TextColor(Color::srgba(0.75, 0.83, 0.9, 0.95)),
                    CharacterSelectStatusText,
                ));
            });
        });
}

#[allow(clippy::type_complexity)]
fn handle_character_select_buttons(
    app_state: Option<Res<'_, State<ClientAppState>>>,
    mut interactions: Query<
        '_,
        '_,
        (
            &Interaction,
            Option<&CharacterSelectButton>,
            Option<&CharacterSelectEnterButton>,
            &mut BackgroundColor,
        ),
        Changed<Interaction>,
    >,
    mut next_state: ResMut<'_, NextState<ClientAppState>>,
    mut session: ResMut<'_, ClientSession>,
    mut auth_state: ResMut<'_, ClientAuthSyncState>,
    mut character_selection: ResMut<'_, CharacterSelectionState>,
    mut status_texts: Query<'_, '_, &mut Text, With<CharacterSelectStatusText>>,
) {
    if !app_state
        .as_ref()
        .is_some_and(|state| **state == ClientAppState::CharacterSelect)
    {
        return;
    }
    let client = reqwest::blocking::Client::new();
    let gateway_url = session.gateway_url.clone();
    for (interaction, select_button, enter_button, mut bg) in &mut interactions {
        match *interaction {
            Interaction::Pressed => {
                if let Some(select_button) = select_button {
                    character_selection.selected_player_entity_id =
                        Some(select_button.player_entity_id.clone());
                    *bg = BackgroundColor(Color::srgba(0.22, 0.3, 0.42, 0.98));
                } else if enter_button.is_some() {
                    let Some(access_token) = session.access_token.as_ref() else {
                        session.status = "No access token; please log in again.".to_string();
                        continue;
                    };
                    let Some(selected_player_entity_id) =
                        character_selection.selected_player_entity_id.clone()
                    else {
                        session.status = "No character selected.".to_string();
                        continue;
                    };
                    match enter_world_request(
                        &client,
                        &gateway_url,
                        access_token,
                        &selected_player_entity_id,
                    ) {
                        Ok(response) if response.accepted => {
                            session.player_entity_id = Some(selected_player_entity_id);
                            auth_state.sent_for_client_entities.clear();
                            auth_state.last_player_entity_id = None;
                            session.status = "Entering world...".to_string();
                            next_state.set(ClientAppState::InWorld);
                        }
                        Ok(_) => {
                            session.status = "Enter World request rejected by gateway.".to_string();
                        }
                        Err(err) => {
                            session.status = format!("Enter World failed: {err}");
                        }
                    }
                    *bg = BackgroundColor(Color::srgb(0.16, 0.38, 0.74));
                }
            }
            Interaction::Hovered => {
                if enter_button.is_some() {
                    *bg = BackgroundColor(Color::srgb(0.24, 0.5, 0.9));
                } else {
                    *bg = BackgroundColor(Color::srgba(0.18, 0.24, 0.33, 0.95));
                }
            }
            Interaction::None => {
                if enter_button.is_some() {
                    *bg = BackgroundColor(Color::srgb(0.2, 0.46, 0.85));
                } else {
                    *bg = BackgroundColor(Color::srgba(0.14, 0.18, 0.24, 0.9));
                }
            }
        }
    }
    for mut text in &mut status_texts {
        text.0 = session.status.clone();
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_world_scene(
    mut commands: Commands<'_, '_>,
    asset_server: Res<'_, AssetServer>,
    fonts: Res<'_, EmbeddedFonts>,
    mut session: ResMut<'_, ClientSession>,
    mut shaders: ResMut<'_, Assets<bevy::shader::Shader>>,
    mut meshes: ResMut<'_, Assets<Mesh>>,
    mut color_materials: ResMut<'_, Assets<ColorMaterial>>,
    mut starfield_motion: ResMut<'_, StarfieldMotionState>,
    mut camera_motion: ResMut<'_, CameraMotionState>,
    asset_root: Res<'_, AssetRootPath>,
    debug_blue_overlay: Res<'_, DebugBlueOverlayEnabled>,
) {
    *starfield_motion = StarfieldMotionState::default();
    *camera_motion = CameraMotionState::default();
    reload_streamed_shaders(&asset_server, &mut shaders, &asset_root.0);
    commands.spawn((
        Camera2d,
        Camera {
            order: -1,
            clear_color: ClearColorConfig::Custom(Color::BLACK),
            ..default()
        },
        RenderLayers::layer(BACKDROP_RENDER_LAYER),
        WorldEntity,
        DespawnOnExit(ClientAppState::InWorld),
    ));

    // Guaranteed visible dark space background (no shader dependency).
    let fallback_mesh = meshes.add(Rectangle::new(1.0, 1.0));
    let fallback_material = color_materials.add(ColorMaterial::from(Color::srgb(0.02, 0.03, 0.08)));
    commands.spawn((
        Mesh2d(fallback_mesh),
        MeshMaterial2d(fallback_material),
        Transform::from_xyz(0.0, 0.0, -210.0),
        RenderLayers::layer(BACKDROP_RENDER_LAYER),
        SpaceBackdropFallback,
        WorldEntity,
        DespawnOnExit(ClientAppState::InWorld),
    ));

    commands.spawn((
        Camera3d::default(),
        Camera {
            order: 0,
            is_active: false,
            clear_color: ClearColorConfig::None,
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 80.0).looking_at(Vec3::ZERO, Vec3::Y),
        GameplayCamera,
        TopDownCamera {
            distance: 420.0,
            target_distance: 420.0,
            min_distance: 420.0,
            max_distance: 1260.0,
            zoom_units_per_wheel: 16.0,
            zoom_smoothness: 8.0,
            look_ahead_offset: Vec2::ZERO,
            filtered_focus_xy: Vec2::ZERO,
            focus_initialized: false,
        },
        WorldEntity,
        DespawnOnExit(ClientAppState::InWorld),
    ));

    commands.spawn((
        DirectionalLight {
            illuminance: 20_000.0,
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 40.0).looking_at(Vec3::ZERO, Vec3::Y),
        WorldEntity,
        DespawnOnExit(ClientAppState::InWorld),
    ));

    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(12),
            top: px(12),
            ..default()
        },
        Text::new(""),
        TextFont {
            font_size: 18.0,
            ..default()
        },
        TextColor(Color::srgb(0.8, 0.95, 0.9)),
        HudText,
        GameplayHud,
        WorldEntity,
        DespawnOnExit(ClientAppState::InWorld),
    ));
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: percent(50.0),
                top: percent(50.0),
                width: px(460),
                margin: UiRect::all(px(-230.0)),
                flex_direction: FlexDirection::Column,
                row_gap: px(12),
                ..default()
            },
            Visibility::Visible,
            LoadingOverlayRoot,
            DespawnOnExit(ClientAppState::InWorld),
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("Loading world assets..."),
                TextFont {
                    font: fonts.bold.clone(),
                    font_size: 26.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                LoadingOverlayText,
            ));
            parent
                .spawn((
                    Node {
                        width: percent(100.0),
                        height: px(16),
                        border: UiRect::all(px(1.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.1, 0.1, 0.14, 0.85)),
                    BorderColor::all(Color::srgba(0.8, 0.9, 1.0, 0.8)),
                ))
                .with_children(|bar| {
                    bar.spawn((
                        Node {
                            width: percent(0.0),
                            height: percent(100.0),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.35, 0.85, 1.0)),
                        LoadingProgressBarFill,
                    ));
                });
        });
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            right: px(20),
            bottom: px(16),
            ..default()
        },
        Text::new("NET"),
        TextFont {
            font: fonts.regular.clone(),
            font_size: 18.0,
            ..default()
        },
        TextColor(Color::srgba(1.0, 1.0, 1.0, 0.0)),
        RuntimeStreamingIconText,
        DespawnOnExit(ClientAppState::InWorld),
    ));
    if debug_blue_overlay.0 {
        let mesh = meshes.add(Rectangle::new(1.0, 1.0));
        let material = color_materials.add(ColorMaterial::from(Color::srgb(0.1, 0.35, 1.0)));
        commands.spawn((
            Mesh2d(mesh),
            MeshMaterial2d(material),
            Transform::from_xyz(0.0, 0.0, -180.0),
            RenderLayers::layer(BACKDROP_RENDER_LAYER),
            DebugBlueBackdrop,
            WorldEntity,
            DespawnOnExit(ClientAppState::InWorld),
        ));
        info!("client debug blue fullscreen overlay enabled");
    }
    commands.spawn((
        FullscreenLayer {
            layer_kind: "space_background".to_string(),
            shader_asset_id: default_space_background_shader_asset_id().to_string(),
            layer_order: -200,
        },
        FallbackFullscreenLayer,
        WorldEntity,
        DespawnOnExit(ClientAppState::InWorld),
    ));
    commands.spawn((
        FullscreenLayer {
            layer_kind: "starfield".to_string(),
            shader_asset_id: default_starfield_shader_asset_id().to_string(),
            layer_order: -190,
        },
        FallbackFullscreenLayer,
        WorldEntity,
        DespawnOnExit(ClientAppState::InWorld),
    ));
    session.status = "Scene ready. Waiting for replicated entities...".to_string();
}

#[allow(clippy::too_many_arguments)]
#[allow(clippy::type_complexity)]
fn update_topdown_camera_system(
    time: Res<'_, Time>,
    input: Option<Res<'_, ButtonInput<KeyCode>>>,
    mut mouse_wheel_events: MessageReader<'_, '_, MouseWheel>,
    session: Res<'_, ClientSession>,
    player_view_state: Res<'_, LocalPlayerViewState>,
    entity_registry: Res<'_, RuntimeEntityHierarchy>,
    anchor_query: Query<
        '_,
        '_,
        (&Transform, Option<&Position>),
        (Without<Camera3d>, Without<GameplayCamera>),
    >,
    mut free_camera: ResMut<'_, FreeCameraState>,
    mut camera_query: Query<
        '_,
        '_,
        (&mut Transform, &mut TopDownCamera),
        (With<Camera3d>, Without<ControlledEntity>),
    >,
) {
    let Ok((mut camera_transform, mut camera)) = camera_query.single_mut() else {
        return;
    };

    let mut wheel_delta_y = 0.0f32;
    for event in mouse_wheel_events.read() {
        wheel_delta_y += event.y;
    }
    if wheel_delta_y != 0.0 {
        camera.target_distance = (camera.target_distance
            - wheel_delta_y * camera.zoom_units_per_wheel)
            .clamp(camera.min_distance, camera.max_distance);
    }
    let dt = time.delta_secs();
    let zoom_alpha = 1.0 - (-camera.zoom_smoothness * dt).exp();
    camera.distance = camera.distance.lerp(camera.target_distance, zoom_alpha);

    let player_anchor = session
        .player_entity_id
        .as_ref()
        .and_then(|id| entity_registry.by_entity_id.get(id).copied())
        .and_then(|entity| anchor_query.get(entity).ok())
        .map(|(anchor_transform, anchor_position)| {
            anchor_position
                .map(|p| p.0.truncate())
                .unwrap_or_else(|| anchor_transform.translation.truncate())
        });

    let (focus_xy, snapped_follow) = if player_view_state.detached_free_camera {
        if !free_camera.initialized {
            free_camera.position_xy = camera_transform.translation.truncate();
            free_camera.initialized = true;
        }
        let mut axis = Vec2::ZERO;
        if let Some(keys) = input.as_ref() {
            if keys.pressed(KeyCode::ArrowUp) {
                axis.y += 1.0;
            }
            if keys.pressed(KeyCode::ArrowDown) {
                axis.y -= 1.0;
            }
            if keys.pressed(KeyCode::ArrowLeft) {
                axis.x -= 1.0;
            }
            if keys.pressed(KeyCode::ArrowRight) {
                axis.x += 1.0;
            }
        }
        let dt = time.delta_secs();
        let speed = 220.0;
        if axis != Vec2::ZERO {
            free_camera.position_xy += axis.normalize() * speed * dt;
        }
        (free_camera.position_xy, false)
    } else if let Some(player_xy) = player_anchor {
        free_camera.position_xy = player_xy;
        free_camera.initialized = true;
        (player_xy, true)
    } else {
        let fallback_xy = camera_transform.translation.truncate();
        free_camera.position_xy = fallback_xy;
        free_camera.initialized = true;
        (fallback_xy, true)
    };
    if !camera.focus_initialized {
        camera.filtered_focus_xy = focus_xy;
        camera.focus_initialized = true;
    } else if snapped_follow {
        camera.filtered_focus_xy = focus_xy;
    } else {
        let follow_smoothness = 60.0;
        let alpha = 1.0 - (-follow_smoothness * dt).exp();
        camera.filtered_focus_xy = camera.filtered_focus_xy.lerp(focus_xy, alpha);
    }
    camera.look_ahead_offset = Vec2::ZERO;

    let render_focus_xy = camera.filtered_focus_xy + camera.look_ahead_offset;
    camera_transform.translation.x = render_focus_xy.x;
    camera_transform.translation.y = render_focus_xy.y;
    camera_transform.translation.z = camera.distance;
    camera_transform.rotation = Quat::IDENTITY;
}

fn update_camera_motion_state(
    time: Res<'_, Time>,
    camera_query: Query<'_, '_, &Transform, With<GameplayCamera>>,
    mut motion: ResMut<'_, CameraMotionState>,
) {
    let Ok(camera_transform) = camera_query.single() else {
        return;
    };
    let dt = time.delta_secs();
    let current_xy = camera_transform.translation.truncate();

    if !motion.initialized {
        motion.world_position_xy = current_xy;
        motion.smoothed_position_xy = current_xy;
        motion.prev_position_xy = current_xy;
        motion.frame_delta_xy = Vec2::ZERO;
        motion.initialized = true;
        return;
    }

    motion.world_position_xy = current_xy;
    let frame_delta_xy = current_xy - motion.prev_position_xy;
    motion.frame_delta_xy = frame_delta_xy;

    // Smooth position for starfield parallax to avoid jitter from reconciliation snaps.
    // Tight enough to track well, loose enough to filter fixed-tick stepping.
    let pos_alpha = 1.0 - (-20.0 * dt).exp();
    motion.smoothed_position_xy = motion.smoothed_position_xy.lerp(current_xy, pos_alpha);

    if dt > 0.0 {
        let raw_velocity = frame_delta_xy / dt;
        let vel_alpha = 1.0 - (-12.0 * dt).exp();
        motion.smoothed_velocity_xy = motion.smoothed_velocity_xy.lerp(raw_velocity, vel_alpha);
    }
    motion.prev_position_xy = current_xy;
}

#[allow(clippy::type_complexity)]
fn lock_camera_to_controlled_entity_end_of_frame(
    session: Res<'_, ClientSession>,
    player_view_state: Res<'_, LocalPlayerViewState>,
    entity_registry: Res<'_, RuntimeEntityHierarchy>,
    anchor_query: Query<
        '_,
        '_,
        (&Transform, Option<&Position>),
        (Without<Camera3d>, Without<GameplayCamera>),
    >,
    mut camera_query: Query<
        '_,
        '_,
        (&mut Transform, &mut TopDownCamera),
        (With<GameplayCamera>, Without<ControlledEntity>),
    >,
) {
    if player_view_state.detached_free_camera {
        return;
    }
    let Some(player_entity) = session
        .player_entity_id
        .as_ref()
        .and_then(|id| entity_registry.by_entity_id.get(id).copied())
    else {
        return;
    };
    let Ok((anchor_transform, anchor_position)) = anchor_query.get(player_entity) else {
        return;
    };
    let Ok((mut camera_transform, mut camera)) = camera_query.single_mut() else {
        return;
    };
    let controlled_xy = anchor_position
        .map(|p| p.0.truncate())
        .unwrap_or_else(|| anchor_transform.translation.truncate());
    camera.look_ahead_offset = Vec2::ZERO;
    camera.filtered_focus_xy = controlled_xy;
    camera.focus_initialized = true;
    camera_transform.translation.x = controlled_xy.x;
    camera_transform.translation.y = controlled_xy.y;
}

fn update_focus_target_system(
    input: Option<Res<'_, ButtonInput<KeyCode>>>,
    mut focus_state: ResMut<'_, CameraFocusState>,
    mut player_view_state: ResMut<'_, LocalPlayerViewState>,
    entity_registry: Res<'_, RuntimeEntityHierarchy>,
    controlled_query: Query<'_, '_, (&ControlledEntity, &Transform)>,
    remote_query: Query<'_, '_, (&RemoteVisibleEntity, &Transform)>,
) {
    if let Some(focused_id) = focus_state.focused_entity_id.as_ref()
        && !entity_registry.by_entity_id.contains_key(focused_id)
    {
        focus_state.set(None);
    }

    let Some(input) = input else {
        return;
    };
    if input.just_pressed(KeyCode::KeyC)
        && let Some(target_id) = focus_state.focused_entity_id.clone().or_else(|| {
            controlled_query
                .iter()
                .next()
                .map(|(c, _)| c.entity_id.clone())
        })
    {
        player_view_state.desired_controlled_entity_id = Some(target_id.clone());
        focus_state.set(Some(target_id));
    }
    if input.just_pressed(KeyCode::KeyF)
        && let Some((_, controlled_transform)) = controlled_query.iter().next()
    {
        let anchor = controlled_transform.translation;
        let nearest_remote = remote_query
            .iter()
            .map(|(remote, transform)| (remote.entity_id.clone(), transform.translation))
            .min_by(|(_, a), (_, b)| {
                anchor
                    .distance_squared(*a)
                    .partial_cmp(&anchor.distance_squared(*b))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(entity_id, _)| entity_id);
        if nearest_remote.is_some() {
            focus_state.set(nearest_remote.clone());
            player_view_state.selected_entity_id = nearest_remote;
        }
    }
}

fn gate_gameplay_camera_system(
    mut camera_query: Query<'_, '_, &mut Camera, With<GameplayCamera>>,
    mut hud_query: Query<'_, '_, &mut Visibility, With<GameplayHud>>,
) {
    for mut camera in &mut camera_query {
        camera.is_active = true;
    }
    for mut visibility in &mut hud_query {
        *visibility = Visibility::Visible;
    }
}

fn update_loading_overlay_system(
    asset_manager: Res<'_, LocalAssetManager>,
    mut overlay_query: Query<'_, '_, &mut Visibility, With<LoadingOverlayRoot>>,
    mut text_query: Query<'_, '_, (&mut Text, &mut TextColor), With<LoadingOverlayText>>,
    mut fill_query: Query<'_, '_, (&mut Node, &mut BackgroundColor), With<LoadingProgressBarFill>>,
) {
    let Ok((mut text, mut color)) = text_query.single_mut() else {
        return;
    };
    let Ok((mut fill_node, mut fill_color)) = fill_query.single_mut() else {
        return;
    };
    if asset_manager.bootstrap_complete() {
        if let Ok(mut visibility) = overlay_query.single_mut() {
            *visibility = Visibility::Hidden;
        }
        color.0.set_alpha(0.0);
        text.0 = "".to_string();
        fill_node.width = percent(0.0);
        fill_color.0.set_alpha(0.0);
        return;
    }
    if let Ok(mut visibility) = overlay_query.single_mut() {
        *visibility = Visibility::Visible;
    }
    let pct = (asset_manager.bootstrap_progress() * 100.0).round();
    fill_node.width = percent(pct.clamp(0.0, 100.0));
    fill_color.0.set_alpha(1.0);
    text.0 = if asset_manager.bootstrap_manifest_seen {
        format!("Loading assets... {}%", pct as i32)
    } else {
        "Waiting for asset manifest...".to_string()
    };
    color.0.set_alpha(1.0);
}

fn update_runtime_stream_icon_system(
    time: Res<'_, Time>,
    asset_manager: Res<'_, LocalAssetManager>,
    mut indicator_state: ResMut<'_, RuntimeAssetStreamIndicatorState>,
    mut text_query: Query<'_, '_, &mut TextColor, With<RuntimeStreamingIconText>>,
) {
    let Ok(mut color) = text_query.single_mut() else {
        return;
    };
    if !asset_manager.should_show_runtime_stream_indicator() {
        color.0.set_alpha(0.0);
        indicator_state.blinking_phase_s = 0.0;
        return;
    }
    indicator_state.blinking_phase_s += time.delta_secs();
    let pulse = (indicator_state.blinking_phase_s * 8.0).sin().abs();
    color.0 = Color::srgba(0.3 + pulse * 0.7, 0.85, 1.0, 0.5 + pulse * 0.5);
}

fn ensure_fullscreen_layer_fallback_system(
    mut commands: Commands<'_, '_>,
    layers: Query<'_, '_, (Entity, Option<&FallbackFullscreenLayer>), With<FullscreenLayer>>,
    asset_manager: Res<'_, LocalAssetManager>,
    watchdog: Res<'_, BootstrapWatchdogState>,
) {
    let mut fallback_entities = Vec::new();
    let mut has_authoritative_layer = false;
    for (entity, fallback_marker) in &layers {
        if fallback_marker.is_some() {
            fallback_entities.push(entity);
        } else {
            has_authoritative_layer = true;
        }
    }
    if has_authoritative_layer {
        for entity in fallback_entities {
            if let Ok(mut entity_commands) = commands.get_entity(entity) {
                entity_commands.despawn();
            }
        }
        return;
    }
    if !layers.is_empty()
        || (!asset_manager.bootstrap_complete() && !watchdog.replication_state_seen)
    {
        return;
    }
    commands.spawn((
        FullscreenLayer {
            layer_kind: "space_background".to_string(),
            shader_asset_id: default_space_background_shader_asset_id().to_string(),
            layer_order: -200,
        },
        FallbackFullscreenLayer,
        WorldEntity,
        DespawnOnExit(ClientAppState::InWorld),
    ));
    commands.spawn((
        FullscreenLayer {
            layer_kind: "starfield".to_string(),
            shader_asset_id: default_starfield_shader_asset_id().to_string(),
            layer_order: -190,
        },
        FallbackFullscreenLayer,
        WorldEntity,
        DespawnOnExit(ClientAppState::InWorld),
    ));
    info!("client spawned fallback fullscreen layers (authoritative layers missing)");
}

fn sync_fullscreen_layer_renderables_system(
    mut commands: Commands<'_, '_>,
    layers: Query<'_, '_, (Entity, &FullscreenLayer, Option<&FullscreenLayerRenderable>)>,
    mut meshes: ResMut<'_, Assets<Mesh>>,
    mut starfield_materials: ResMut<'_, Assets<StarfieldMaterial>>,
    mut space_background_materials: ResMut<'_, Assets<SpaceBackgroundMaterial>>,
    asset_root: Res<'_, AssetRootPath>,
    asset_manager: Res<'_, LocalAssetManager>,
) {
    for (entity, layer, rendered) in &layers {
        let Ok(mut entity_commands) = commands.get_entity(entity) else {
            continue;
        };
        let has_streamed_shader =
            fullscreen_layer_shader_ready(&asset_root.0, &asset_manager, &layer.shader_asset_id);
        let is_supported_kind =
            layer.layer_kind == "starfield" || layer.layer_kind == "space_background";
        let needs_rebuild = rendered.is_none_or(|existing| {
            existing.layer_kind != layer.layer_kind || existing.layer_order != layer.layer_order
        });

        if !is_supported_kind || !has_streamed_shader {
            if !is_supported_kind {
                warn!(
                    "unsupported fullscreen layer kind={} shader_asset_id={}",
                    layer.layer_kind, layer.shader_asset_id
                );
            } else {
                warn!(
                    "fullscreen layer waiting for shader readiness layer_kind={} shader_asset_id={}",
                    layer.layer_kind, layer.shader_asset_id
                );
            }
            if rendered.is_some() {
                entity_commands
                    .remove::<FullscreenLayerRenderable>()
                    .remove::<StarfieldBackdrop>()
                    .remove::<SpaceBackgroundBackdrop>()
                    .remove::<Mesh2d>()
                    .remove::<MeshMaterial2d<StarfieldMaterial>>()
                    .remove::<MeshMaterial2d<SpaceBackgroundMaterial>>();
            }
            continue;
        }

        if needs_rebuild {
            let mesh = meshes.add(Rectangle::new(1.0, 1.0));
            entity_commands
                .try_insert((
                    Mesh2d(mesh),
                    Transform::from_xyz(0.0, 0.0, layer.layer_order as f32),
                    RenderLayers::layer(BACKDROP_RENDER_LAYER),
                    FullscreenLayerRenderable {
                        layer_kind: layer.layer_kind.clone(),
                        layer_order: layer.layer_order,
                    },
                ))
                .remove::<StarfieldBackdrop>()
                .remove::<SpaceBackgroundBackdrop>()
                .remove::<MeshMaterial2d<StarfieldMaterial>>()
                .remove::<MeshMaterial2d<SpaceBackgroundMaterial>>();

            if layer.layer_kind == "starfield" {
                let material = starfield_materials.add(StarfieldMaterial::default());
                entity_commands.try_insert((StarfieldBackdrop, MeshMaterial2d(material)));
            } else {
                let material = space_background_materials.add(SpaceBackgroundMaterial::default());
                entity_commands.try_insert((SpaceBackgroundBackdrop, MeshMaterial2d(material)));
            }
            info!(
                "fullscreen layer renderable ready layer_kind={} order={} shader_asset_id={}",
                layer.layer_kind, layer.layer_order, layer.shader_asset_id
            );
        } else {
            entity_commands.try_insert(Transform::from_xyz(0.0, 0.0, layer.layer_order as f32));
        }
    }
}

#[allow(clippy::type_complexity)]
fn sync_backdrop_fullscreen_system(
    window_query: Query<'_, '_, &Window, With<bevy::window::PrimaryWindow>>,
    mut backdrop_query: Query<
        '_,
        '_,
        &mut Transform,
        (
            Or<(
                With<StarfieldBackdrop>,
                With<SpaceBackgroundBackdrop>,
                With<DebugBlueBackdrop>,
                With<SpaceBackdropFallback>,
            )>,
        ),
    >,
) {
    let Ok(window) = window_query.single() else {
        return;
    };
    let width = window.resolution.width();
    let height = window.resolution.height();
    if width <= 0.0 || height <= 0.0 {
        return;
    }
    for mut transform in &mut backdrop_query {
        transform.translation.x = 0.0;
        transform.translation.y = 0.0;
        // Mesh2d uses screen-space-like world units with the 2D camera, so size against viewport.
        transform.scale = Vec3::new(width, height, 1.0);
    }
}

fn enforce_frame_rate_cap_system(mut frame_cap: ResMut<'_, FrameRateCap>) {
    let elapsed = frame_cap.last_frame_end.elapsed();
    if elapsed < frame_cap.frame_duration {
        std::thread::sleep(frame_cap.frame_duration - elapsed);
    }
    frame_cap.last_frame_end = Instant::now();
}

#[allow(clippy::type_complexity)]
fn sync_world_entity_transforms_from_physics(
    mut entities: Query<
        '_,
        '_,
        (&mut Transform, Option<&Position>, Option<&Rotation>),
        (With<WorldEntity>, Without<Camera3d>, Without<Camera2d>),
    >,
) {
    for (mut transform, position, rotation) in &mut entities {
        if let Some(position) = position {
            transform.translation = position.0;
        }
        if let Some(rotation) = rotation {
            transform.rotation = rotation.0;
        }
        // Keep 2D gameplay entities constrained to planar render depth.
        transform.translation.z = 0.0;
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn send_lightyear_input_messages(
    input: Option<Res<'_, ButtonInput<KeyCode>>>,
    app_state: Option<Res<'_, State<ClientAppState>>>,
    headless_mode: Res<'_, HeadlessTransportMode>,
    time: Res<'_, Time>,
    mut commands: Commands<'_, '_>,
    session: Res<'_, ClientSession>,
    player_view_state: Res<'_, LocalPlayerViewState>,
    mut tick: ResMut<'_, ClientNetworkTick>,
    mut ack_tracker: ResMut<'_, ClientInputAckTracker>,
    mut input_log_state: ResMut<'_, ClientInputLogState>,
    mut controlled_input_history: Query<
        '_,
        '_,
        (
            Entity,
            &ControlledEntity,
            Option<&mut ActionState<PlayerInput>>,
        ),
    >,
) {
    tick.0 = tick.0.saturating_add(1);

    let in_world_state = app_state
        .as_ref()
        .is_some_and(|state| **state == ClientAppState::InWorld)
        || headless_mode.0;

    let (player_entity_id, player_input) = if in_world_state {
        let Some(player_entity_id) = session.player_entity_id.clone() else {
            return;
        };
        let (player_input, _axes) = if player_view_state.detached_free_camera {
            neutral_player_input()
        } else {
            player_input_from_keyboard(input.as_deref())
        };
        (player_entity_id, player_input)
    } else {
        return;
    };

    ack_tracker.pending_ticks.push_back(tick.0);
    while ack_tracker.pending_ticks.len() > 512 {
        ack_tracker.pending_ticks.pop_front();
    }
    let has_active_input = player_input.actions.iter().any(|a| {
        !matches!(
            a,
            EntityAction::ThrustNeutral
                | EntityAction::YawNeutral
                | EntityAction::LongitudinalNeutral
                | EntityAction::LateralNeutral
        )
    });
    if has_active_input && client_input_debug_logging_enabled() {
        let now = time.elapsed_secs_f64();
        if now - input_log_state.last_logged_at_s >= 0.5 {
            input_log_state.last_logged_at_s = now;
            info!(
                actions = ?player_input.actions,
                tick = tick.0,
                "client sending active input"
            );
        }
    }

    for (entity, controlled, maybe_action_state) in &mut controlled_input_history {
        if controlled.player_entity_id != player_entity_id {
            continue;
        }
        if let Some(mut state) = maybe_action_state {
            state.0 = player_input.clone();
        } else {
            commands.entity(entity).insert((
                InputMarker::<PlayerInput>::default(),
                ActionState(player_input.clone()),
            ));
        }
    }
}

fn client_input_debug_logging_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("SIDEREAL_DEBUG_INPUT_LOGS")
            .is_ok_and(|v| v == "1" || v.eq_ignore_ascii_case("true"))
    })
}

#[allow(clippy::type_complexity)]
fn send_lightyear_auth_messages(
    app_state: Option<Res<'_, State<ClientAppState>>>,
    headless_mode: Res<'_, HeadlessTransportMode>,
    time: Res<'_, Time>,
    watchdog: Res<'_, BootstrapWatchdogState>,
    session: Res<'_, ClientSession>,
    mut auth_state: ResMut<'_, ClientAuthSyncState>,
    mut senders: Query<
        '_,
        '_,
        (Entity, &mut MessageSender<ClientAuthMessage>),
        (With<Client>, With<Connected>),
    >,
) {
    let in_world_state = app_state
        .as_ref()
        .is_some_and(|state| **state == ClientAppState::InWorld)
        || headless_mode.0;
    if !in_world_state {
        return;
    }
    let Some(access_token) = session.access_token.as_ref() else {
        return;
    };
    let Some(player_entity_id) = session.player_entity_id.as_ref() else {
        return;
    };
    if auth_state.last_player_entity_id.as_deref() != Some(player_entity_id.as_str()) {
        auth_state.sent_for_client_entities.clear();
        auth_state.last_sent_at_s_by_client_entity.clear();
        auth_state.last_player_entity_id = Some(player_entity_id.clone());
    }
    let now_s = time.elapsed_secs_f64();

    for (client_entity, mut sender) in &mut senders {
        let sent_before = auth_state.sent_for_client_entities.contains(&client_entity);
        let last_sent_at_s = auth_state
            .last_sent_at_s_by_client_entity
            .get(&client_entity)
            .copied()
            .unwrap_or(0.0);
        let should_resend_while_unbound =
            !watchdog.replication_state_seen && now_s - last_sent_at_s >= 0.5;
        if sent_before && !should_resend_while_unbound {
            continue;
        }
        let auth_message = ClientAuthMessage {
            player_entity_id: player_entity_id.clone(),
            access_token: access_token.clone(),
        };
        sender.send::<ControlChannel>(auth_message);
        info!(
            "client auth bind message sent for player_entity_id={} client_entity={:?}",
            player_entity_id, client_entity
        );
        auth_state.sent_for_client_entities.insert(client_entity);
        auth_state
            .last_sent_at_s_by_client_entity
            .insert(client_entity, now_s);
    }
}

#[allow(clippy::too_many_arguments)]
fn send_lightyear_view_updates(
    app_state: Option<Res<'_, State<ClientAppState>>>,
    headless_mode: Res<'_, HeadlessTransportMode>,
    session: Res<'_, ClientSession>,
    mut view_tick: ResMut<'_, ClientViewUpdateTick>,
    mut senders: Query<
        '_,
        '_,
        &mut MessageSender<ClientViewUpdateMessage>,
        (With<Client>, With<Connected>),
    >,
    camera_query: Query<'_, '_, &Transform, With<Camera3d>>,
    controlled_query: Query<'_, '_, &Transform, With<ControlledEntity>>,
    focus_state: Res<'_, CameraFocusState>,
    player_view_state: Res<'_, LocalPlayerViewState>,
) {
    let in_world_state = app_state
        .as_ref()
        .is_some_and(|state| **state == ClientAppState::InWorld)
        || headless_mode.0;
    if !in_world_state {
        return;
    }
    let Some(player_entity_id) = session.player_entity_id.as_ref() else {
        return;
    };
    if senders.is_empty() {
        return;
    }

    view_tick.0 = view_tick.0.saturating_add(1);
    let pending_control_handoff =
        player_view_state.desired_controlled_entity_id != player_view_state.controlled_entity_id;
    if !pending_control_handoff && !view_tick.0.is_multiple_of(10) {
        return;
    }

    let camera_position = camera_query
        .single()
        .ok()
        .map(|t| t.translation)
        .or_else(|| controlled_query.iter().next().map(|t| t.translation))
        .unwrap_or(Vec3::ZERO);
    let focused_entity_id = focus_state.focused_entity_id.clone();
    // Preserve authoritative control by default until local desired control is initialized.
    let desired_controlled_entity_id = player_view_state
        .desired_controlled_entity_id
        .clone()
        .or_else(|| player_view_state.controlled_entity_id.clone());
    let message = ClientViewUpdateMessage {
        player_entity_id: player_entity_id.clone(),
        focused_entity_id,
        selected_entity_id: player_view_state.selected_entity_id.clone(),
        controlled_entity_id: desired_controlled_entity_id,
        camera_position_m: [camera_position.x, camera_position.y, camera_position.z],
    };

    for mut sender in &mut senders {
        sender.send::<ControlChannel>(message.clone());
    }
}

fn reset_bootstrap_watchdog_on_enter_in_world(
    time: Res<'_, Time>,
    mut watchdog: ResMut<'_, BootstrapWatchdogState>,
) {
    info!("client entered in-world state; bootstrap watchdog armed");
    *watchdog = BootstrapWatchdogState {
        in_world_entered_at_s: Some(time.elapsed_secs_f64()),
        last_bootstrap_progress_at_s: time.elapsed_secs_f64(),
        ..Default::default()
    };
}

fn log_native_client_connected(
    trigger: On<Add, Connected>,
    clients: Query<'_, '_, (), With<Client>>,
) {
    if clients.get(trigger.entity).is_ok() {
        info!("native client lightyear transport connected");
    }
}

fn configure_prediction_manager_tuning(
    tuning: Res<'_, PredictionCorrectionTuning>,
    mut managers: Query<'_, '_, &mut PredictionManager, (With<Client>, Added<PredictionManager>)>,
) {
    for mut manager in &mut managers {
        manager.rollback_policy.max_rollback_ticks = tuning.max_rollback_ticks;
        manager.correction_policy = if tuning.instant_correction {
            CorrectionPolicy::instant_correction()
        } else {
            CorrectionPolicy::default()
        };
        info!(
            "configured prediction manager (max_rollback_ticks={}, correction_mode={})",
            tuning.max_rollback_ticks,
            if tuning.instant_correction {
                "instant"
            } else {
                "smooth"
            }
        );
    }
}

#[allow(clippy::too_many_arguments)]
#[allow(clippy::type_complexity)]
fn adopt_native_lightyear_replicated_entities(
    mut commands: Commands<'_, '_>,
    session: Res<'_, ClientSession>,
    local_mode: Res<'_, LocalSimulationDebugMode>,
    tuning: Res<'_, PredictionBootstrapTuning>,
    time: Res<'_, Time>,
    mut adoption_state: ResMut<'_, DeferredPredictedAdoptionState>,
    mut watchdog: ResMut<'_, BootstrapWatchdogState>,
    mut focus_state: ResMut<'_, CameraFocusState>,
    mut player_view_state: ResMut<'_, LocalPlayerViewState>,
    mut entity_registry: ResMut<'_, RuntimeEntityHierarchy>,
    mut remote_registry: ResMut<'_, RemoteEntityRegistry>,
    replicated_entities: Query<
        '_,
        '_,
        (
            Entity,
            Option<&'_ EntityGuid>,
            Option<&'_ OwnerId>,
            Option<&'_ MountedOn>,
            Option<&'_ Hardpoint>,
            Option<&'_ PlayerTag>,
            Option<&'_ Position>,
            Option<&'_ Rotation>,
            Option<&'_ LinearVelocity>,
            Option<&'_ SizeM>,
            Option<&'_ TotalMassKg>,
            Option<&'_ ControlledEntityGuid>,
            Option<&'_ SelectedEntityGuid>,
            Option<&'_ FocusedEntityGuid>,
        ),
        (
            With<lightyear::prelude::Replicated>,
            Without<WorldEntity>,
            Without<DespawnOnExit<ClientAppState>>,
        ),
    >,
    controlled_query: Query<'_, '_, Entity, With<ControlledEntity>>,
    adopted_entity_state: Query<
        '_,
        '_,
        (
            Has<WorldEntity>,
            Has<ControlledEntity>,
            Has<lightyear::prelude::Predicted>,
            Has<lightyear::prelude::Interpolated>,
            Has<RemoteEntity>,
        ),
    >,
) {
    let Some(local_player_entity_id) = session.player_entity_id.as_ref() else {
        return;
    };

    // Resolve authoritative controlled/selected/focused IDs from the replicated local player entity
    // before classifying predicted vs interpolated entities. This avoids order-dependent tagging.
    // Prediction ownership must follow authoritative control, not local desired handoff state.
    let mut authoritative_controlled_entity_id = player_view_state.controlled_entity_id.clone();
    for (
        _,
        guid,
        _,
        mounted_on,
        hardpoint,
        player_tag,
        _,
        _,
        _,
        _,
        _,
        controlled_entity_guid,
        selected_entity_guid,
        focused_entity_guid,
    ) in &replicated_entities
    {
        let Some(guid) = guid else {
            continue;
        };
        if mounted_on.is_some() || hardpoint.is_some() || player_tag.is_none() {
            continue;
        }
        let runtime_entity_id = format!("player:{}", guid.0);
        if runtime_entity_id != *local_player_entity_id {
            continue;
        }
        let controlled_id = controlled_entity_guid
            .and_then(|c| c.0.as_ref())
            .and_then(|guid| {
                runtime_entity_id_from_guid(&entity_registry, local_player_entity_id, guid)
            });
        player_view_state.controlled_entity_id = controlled_id.clone();
        player_view_state.selected_entity_id = selected_entity_guid
            .and_then(|v| v.0.as_ref())
            .and_then(|guid| {
                runtime_entity_id_from_guid(&entity_registry, local_player_entity_id, guid)
            });
        player_view_state.focused_entity_id = focused_entity_guid
            .and_then(|v| v.0.as_ref())
            .and_then(|guid| {
                runtime_entity_id_from_guid(&entity_registry, local_player_entity_id, guid)
            });
        if player_view_state.desired_controlled_entity_id.is_none() {
            player_view_state.desired_controlled_entity_id = controlled_id.clone();
        }
        authoritative_controlled_entity_id = controlled_id;
        break;
    }

    let mut seen_runtime_entity_ids = HashSet::<String>::new();

    for (
        entity,
        guid,
        _owner_id,
        mounted_on,
        hardpoint,
        player_tag,
        position,
        rotation,
        linear_velocity,
        size_m,
        total_mass_kg,
        controlled_entity_guid,
        selected_entity_guid,
        focused_entity_guid,
    ) in &replicated_entities
    {
        let Some(guid) = guid else {
            continue;
        };
        watchdog.replication_state_seen = true;
        let runtime_entity_id = if player_tag.is_some() {
            format!("player:{}", guid.0)
        } else if mounted_on.is_some() {
            format!("module:{}", guid.0)
        } else if hardpoint.is_some() {
            format!("hardpoint:{}", guid.0)
        } else {
            format!("ship:{}", guid.0)
        };
        if !seen_runtime_entity_ids.insert(runtime_entity_id.clone()) {
            continue;
        }
        let is_root_entity = mounted_on.is_none() && hardpoint.is_none() && player_tag.is_none();
        let is_local_controlled_entity = is_root_entity
            && authoritative_controlled_entity_id.as_deref() == Some(runtime_entity_id.as_str());
        let is_local_player_entity = runtime_entity_id == *local_player_entity_id;
        if is_local_player_entity {
            player_view_state.controlled_entity_id = controlled_entity_guid
                .and_then(|c| c.0.as_ref())
                .and_then(|guid| {
                    runtime_entity_id_from_guid(&entity_registry, local_player_entity_id, guid)
                });
            player_view_state.selected_entity_id = selected_entity_guid
                .and_then(|v| v.0.as_ref())
                .and_then(|guid| {
                    runtime_entity_id_from_guid(&entity_registry, local_player_entity_id, guid)
                });
            player_view_state.focused_entity_id = focused_entity_guid
                .and_then(|v| v.0.as_ref())
                .and_then(|guid| {
                    runtime_entity_id_from_guid(&entity_registry, local_player_entity_id, guid)
                });
            if player_view_state.desired_controlled_entity_id.is_none() {
                player_view_state.desired_controlled_entity_id =
                    player_view_state.controlled_entity_id.clone();
            }
        }
        let predicted_mode = !local_mode.0;
        let candidate_score = candidate_runtime_entity_score(
            is_root_entity,
            is_local_controlled_entity,
            predicted_mode,
        );
        if predicted_mode
            && should_defer_controlled_predicted_adoption(
                is_local_controlled_entity,
                position.is_some(),
                rotation.is_some(),
                linear_velocity.is_some(),
            )
        {
            let now_s = time.elapsed_secs_f64();
            let mut missing = Vec::new();
            if position.is_none() {
                missing.push("Position");
            }
            if rotation.is_none() {
                missing.push("Rotation");
            }
            if linear_velocity.is_none() {
                missing.push("LinearVelocity");
            }
            let missing_summary = missing.join(", ");
            if adoption_state.waiting_entity_id.as_deref() != Some(runtime_entity_id.as_str()) {
                adoption_state.waiting_entity_id = Some(runtime_entity_id.clone());
                adoption_state.wait_started_at_s = Some(now_s);
                adoption_state.last_warn_at_s = 0.0;
                adoption_state.dialog_shown = false;
            }
            adoption_state.last_missing_components = missing_summary.clone();
            if let Some(started_at_s) = adoption_state.wait_started_at_s {
                let wait_s = (now_s - started_at_s).max(0.0);
                if wait_s >= tuning.defer_warn_after_s
                    && now_s - adoption_state.last_warn_at_s >= tuning.defer_warn_interval_s
                {
                    warn!(
                        "deferring predicted controlled adoption for {} (wait {:.2}s, missing: {})",
                        runtime_entity_id, wait_s, missing_summary
                    );
                    adoption_state.last_warn_at_s = now_s;
                }
            }
            // Delay adoption until authoritative replicated Avian state is present.
            continue;
        }

        // Canonical identity reconciliation:
        // exactly one adopted client entity per logical runtime ID.
        if let Some(&existing_entity) = entity_registry.by_entity_id.get(runtime_entity_id.as_str())
            && existing_entity != entity
        {
            if let Ok((is_world, is_controlled, is_predicted, is_interpolated, is_remote)) =
                adopted_entity_state.get(existing_entity)
            {
                let existing_score = existing_runtime_entity_score(
                    is_world,
                    is_controlled,
                    is_predicted,
                    is_interpolated,
                    is_remote,
                );
                if candidate_score <= existing_score {
                    continue;
                }

                // Candidate is a better representative for this runtime ID; demote the old one.
                commands.entity(existing_entity).remove::<Name>();
                if is_world {
                    commands
                        .entity(existing_entity)
                        .insert(Visibility::Hidden)
                        .remove::<(
                            WorldEntity,
                            RemoteEntity,
                            RemoteVisibleEntity,
                            ControlledEntity,
                            StreamedModelAssetId,
                            StreamedModelVisualAttached,
                        )>();
                }
                if entity_registry.by_entity_id.get(runtime_entity_id.as_str())
                    == Some(&existing_entity)
                {
                    entity_registry
                        .by_entity_id
                        .remove(runtime_entity_id.as_str());
                }
                if remote_registry.by_entity_id.get(runtime_entity_id.as_str())
                    == Some(&existing_entity)
                {
                    remote_registry
                        .by_entity_id
                        .remove(runtime_entity_id.as_str());
                }
            } else {
                // Stale registry entry pointing at a despawned entity.
                entity_registry
                    .by_entity_id
                    .remove(runtime_entity_id.as_str());
                if remote_registry.by_entity_id.get(runtime_entity_id.as_str())
                    == Some(&existing_entity)
                {
                    remote_registry
                        .by_entity_id
                        .remove(runtime_entity_id.as_str());
                }
            }
        }

        if adoption_state.waiting_entity_id.as_deref() == Some(runtime_entity_id.as_str()) {
            if let Some(started_at_s) = adoption_state.wait_started_at_s {
                let resolved_wait_s = (time.elapsed_secs_f64() - started_at_s).max(0.0);
                adoption_state.resolved_samples = adoption_state.resolved_samples.saturating_add(1);
                adoption_state.resolved_total_wait_s += resolved_wait_s;
                adoption_state.resolved_max_wait_s =
                    adoption_state.resolved_max_wait_s.max(resolved_wait_s);
                info!(
                    "predicted controlled adoption resolved for {} after {:.2}s (samples={}, max_wait_s={:.2})",
                    runtime_entity_id,
                    resolved_wait_s,
                    adoption_state.resolved_samples,
                    adoption_state.resolved_max_wait_s
                );
            }
            adoption_state.waiting_entity_id = None;
            adoption_state.wait_started_at_s = None;
            adoption_state.last_warn_at_s = 0.0;
            adoption_state.last_missing_components.clear();
            adoption_state.dialog_shown = false;
        }

        register_runtime_entity(&mut entity_registry, runtime_entity_id.clone(), entity);
        let mut entity_commands = commands.entity(entity);
        entity_commands.insert((
            Name::new(runtime_entity_id.clone()),
            Transform::default(),
            GlobalTransform::default(),
            WorldEntity,
            DespawnOnExit(ClientAppState::InWorld),
            Visibility::Visible,
            InheritedVisibility::default(),
            ViewVisibility::default(),
        ));

        if mounted_on.is_none() && hardpoint.is_none() && player_tag.is_none() {
            entity_commands.insert(StreamedModelAssetId(
                default_corvette_asset_id().to_string(),
            ));
        }

        if is_local_controlled_entity {
            let size = size_m.copied().unwrap_or_else(default_corvette_size);
            let mass_kg = total_mass_kg
                .map(|m| m.0)
                .filter(|m| *m > 0.0)
                .unwrap_or_else(default_corvette_mass_kg);
            let position = position.map(|p| p.0).unwrap_or(Vec3::ZERO);
            let rotation = rotation.map(|r| r.0).unwrap_or(Quat::IDENTITY);
            let velocity = linear_velocity.map(|v| v.0).unwrap_or(Vec3::ZERO);
            entity_commands.insert((
                RigidBody::Dynamic,
                Collider::cuboid(size.width * 0.5, size.length * 0.5, size.height * 0.5),
                Mass(mass_kg),
                angular_inertia_from_size(mass_kg, &size),
                Position(position),
                Rotation(rotation),
                LinearVelocity(velocity),
                AngularVelocity::default(),
                LockedAxes::new()
                    .lock_translation_z()
                    .lock_rotation_x()
                    .lock_rotation_y(),
                LinearDamping(0.0),
                AngularDamping(0.0),
            ));
            if predicted_mode {
                entity_commands
                    .insert(lightyear::prelude::Predicted)
                    .remove::<lightyear::prelude::Interpolated>();
            }
            entity_commands.remove::<RemoteEntity>();
            entity_commands.insert(RemoteVisibleEntity {
                entity_id: runtime_entity_id.clone(),
            });
        } else if is_root_entity {
            entity_commands.insert((
                RemoteEntity,
                RemoteVisibleEntity {
                    entity_id: runtime_entity_id.clone(),
                },
            ));
            remote_registry
                .by_entity_id
                .insert(runtime_entity_id, entity);
            if predicted_mode {
                entity_commands
                    .insert(lightyear::prelude::Interpolated)
                    .remove::<lightyear::prelude::Predicted>();
            }
            entity_commands.remove::<(
                RigidBody,
                Collider,
                Mass,
                AngularInertia,
                LockedAxes,
                LinearDamping,
                AngularDamping,
            )>();
        }
        if is_local_player_entity && focus_state.focused_entity_id.is_none() {
            focus_state.set(player_view_state.focused_entity_id.clone());
        }
    }

    let now_s = time.elapsed_secs_f64();
    if adoption_state.resolved_samples > 0
        && now_s - adoption_state.last_summary_at_s >= tuning.defer_summary_interval_s
    {
        let avg_wait_s =
            adoption_state.resolved_total_wait_s / adoption_state.resolved_samples as f64;
        info!(
            "predicted adoption delay summary samples={} avg_wait_s={:.2} max_wait_s={:.2}",
            adoption_state.resolved_samples, avg_wait_s, adoption_state.resolved_max_wait_s
        );
        adoption_state.last_summary_at_s = now_s;
    }

    // Native replication can promote ownership after initial spawn; avoid duplicated controlled tags.
    let controlled_count = controlled_query.iter().count();
    if controlled_count > 1 {
        warn!(
            "multiple controlled entities detected under native replication; keeping latest focus target"
        );
    }
    if controlled_count > 0 {
        adoption_state.waiting_entity_id = None;
        adoption_state.wait_started_at_s = None;
        adoption_state.last_warn_at_s = 0.0;
        adoption_state.last_missing_components.clear();
        adoption_state.dialog_shown = false;
    }
}

#[allow(clippy::type_complexity)]
fn sync_local_player_view_state_system(
    session: Res<'_, ClientSession>,
    mut player_view_state: ResMut<'_, LocalPlayerViewState>,
    mut focus_state: ResMut<'_, CameraFocusState>,
    entity_registry: Res<'_, RuntimeEntityHierarchy>,
    player_query: Query<
        '_,
        '_,
        (
            &'_ Name,
            Option<&'_ ControlledEntityGuid>,
            Option<&'_ SelectedEntityGuid>,
            Option<&'_ FocusedEntityGuid>,
        ),
        With<PlayerTag>,
    >,
) {
    let Some(local_player_entity_id) = session.player_entity_id.as_ref() else {
        return;
    };
    for (name, controlled, selected, focused) in &player_query {
        if name.as_str() != local_player_entity_id {
            continue;
        }
        player_view_state.controlled_entity_id =
            controlled.and_then(|c| c.0.as_ref()).and_then(|guid| {
                runtime_entity_id_from_guid(&entity_registry, local_player_entity_id, guid)
            });
        player_view_state.selected_entity_id =
            selected.and_then(|v| v.0.as_ref()).and_then(|guid| {
                runtime_entity_id_from_guid(&entity_registry, local_player_entity_id, guid)
            });
        player_view_state.focused_entity_id = focused.and_then(|v| v.0.as_ref()).and_then(|guid| {
            runtime_entity_id_from_guid(&entity_registry, local_player_entity_id, guid)
        });
        if player_view_state.desired_controlled_entity_id.is_none() {
            player_view_state.desired_controlled_entity_id =
                player_view_state.controlled_entity_id.clone();
        }
        if focus_state.focused_entity_id.is_none() {
            focus_state.focused_entity_id = player_view_state.focused_entity_id.clone();
        }
        break;
    }
}

fn sync_controlled_entity_tags_system(
    mut commands: Commands<'_, '_>,
    session: Res<'_, ClientSession>,
    player_view_state: ResMut<'_, LocalPlayerViewState>,
    entity_registry: Res<'_, RuntimeEntityHierarchy>,
    controlled_query: Query<'_, '_, (Entity, &'_ ControlledEntity)>,
) {
    let Some(local_player_entity_id) = session.player_entity_id.as_ref() else {
        return;
    };

    // Route local input to authoritative control first. During handoff, keep writing
    // to the currently-authoritative entity until server confirms the new target.
    // This prevents "dead input" windows where desired target is not yet bound.
    let target_entity_id = player_view_state
        .controlled_entity_id
        .as_ref()
        .and_then(|id| entity_registry.by_entity_id.contains_key(id).then_some(id))
        .or_else(|| {
            player_view_state
                .desired_controlled_entity_id
                .as_ref()
                .and_then(|id| entity_registry.by_entity_id.contains_key(id).then_some(id))
        })
        .or(Some(local_player_entity_id));
    let target_entity = target_entity_id
        .as_ref()
        .and_then(|id| entity_registry.by_entity_id.get(id.as_str()).copied());

    for (entity, controlled) in &controlled_query {
        if Some(entity) != target_entity {
            commands.entity(entity).remove::<ControlledEntity>();
        } else if controlled.player_entity_id != *local_player_entity_id {
            commands.entity(entity).insert(ControlledEntity {
                entity_id: controlled.entity_id.clone(),
                player_entity_id: local_player_entity_id.clone(),
            });
        }
    }

    if let Some(entity) = target_entity {
        commands.entity(entity).insert(ControlledEntity {
            entity_id: target_entity_id.cloned().unwrap_or_default(),
            player_entity_id: local_player_entity_id.clone(),
        });
    }
}

#[allow(clippy::too_many_arguments)]
#[allow(clippy::type_complexity)]
fn log_prediction_runtime_state(
    time: Res<'_, Time>,
    tuning: Res<'_, PredictionBootstrapTuning>,
    local_mode: Res<'_, LocalSimulationDebugMode>,
    watchdog: Res<'_, BootstrapWatchdogState>,
    mut adoption_state: ResMut<'_, DeferredPredictedAdoptionState>,
    world_entities: Query<'_, '_, Entity, With<WorldEntity>>,
    replicated_entities: Query<'_, '_, Entity, With<lightyear::prelude::Replicated>>,
    predicted_entities: Query<'_, '_, Entity, With<lightyear::prelude::Predicted>>,
    interpolated_entities: Query<'_, '_, Entity, With<lightyear::prelude::Interpolated>>,
    controlled_entities: Query<'_, '_, Entity, With<ControlledEntity>>,
) {
    let now_s = time.elapsed_secs_f64();
    if now_s - adoption_state.last_runtime_summary_at_s < tuning.defer_summary_interval_s {
        return;
    }
    adoption_state.last_runtime_summary_at_s = now_s;
    let world_count = world_entities.iter().count();
    let replicated_count = replicated_entities.iter().count();
    let predicted_count = predicted_entities.iter().count();
    let interpolated_count = interpolated_entities.iter().count();
    let controlled_count = controlled_entities.iter().count();
    let mode = if local_mode.0 { "local" } else { "predicted" };
    info!(
        "prediction runtime summary mode={} world={} replicated={} predicted={} interpolated={} controlled={} deferred_waiting={}",
        mode,
        world_count,
        replicated_count,
        predicted_count,
        interpolated_count,
        controlled_count,
        adoption_state
            .waiting_entity_id
            .as_deref()
            .unwrap_or("<none>")
    );
    if !local_mode.0 && watchdog.replication_state_seen {
        let in_world_age_s = watchdog
            .in_world_entered_at_s
            .map(|entered_at_s| (now_s - entered_at_s).max(0.0))
            .unwrap_or_default();
        if in_world_age_s > tuning.defer_dialog_after_s && controlled_count == 0 {
            warn!(
                "prediction runtime anomaly: no controlled entity after {:.2}s in predicted mode (replicated={} predicted={} interpolated={})",
                in_world_age_s, replicated_count, predicted_count, interpolated_count
            );
        }
        if replicated_count > 0 && predicted_count == 0 {
            warn!(
                "prediction runtime anomaly: replicated entities present but zero Predicted markers (replicated={} interpolated={})",
                replicated_count, interpolated_count
            );
        }
    }
}

fn streamed_model_scene_path(asset_id: &str, asset_manager: &LocalAssetManager) -> Option<String> {
    let relative = asset_manager.cached_relative_path(asset_id)?;
    if !(relative.ends_with(".gltf") || relative.ends_with(".glb")) {
        return None;
    }
    Some(format!("data/cache_stream/{relative}"))
}

#[allow(clippy::type_complexity)]
fn attach_streamed_model_visuals_system(
    mut commands: Commands<'_, '_>,
    asset_server: Res<'_, AssetServer>,
    asset_root: Res<'_, AssetRootPath>,
    asset_manager: Res<'_, LocalAssetManager>,
    candidates: Query<
        '_,
        '_,
        (Entity, &StreamedModelAssetId),
        (With<WorldEntity>, Without<StreamedModelVisualAttached>),
    >,
) {
    for (entity, asset_id) in &candidates {
        if let Some(path) = streamed_model_scene_path(&asset_id.0, &asset_manager)
            && gltf_scene_dependencies_ready(&asset_root.0, &path)
        {
            let Ok(mut entity_commands) = commands.get_entity(entity) else {
                continue;
            };
            let scene_handle =
                asset_server.load(bevy::gltf::GltfAssetLabel::Scene(0).from_asset(path));
            entity_commands.with_children(|child| {
                child.spawn((
                    SceneRoot(scene_handle),
                    Transform::from_scale(Vec3::splat(2.5)),
                ));
            });
            entity_commands.try_insert(StreamedModelVisualAttached);
        }
    }
}

fn gltf_scene_dependencies_ready(asset_root: &str, scene_path: &str) -> bool {
    if !scene_path.ends_with(".gltf") {
        return true;
    }
    let full_path = std::path::PathBuf::from(asset_root).join(scene_path);
    let Ok(text) = std::fs::read_to_string(&full_path) else {
        return false;
    };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else {
        return false;
    };
    let base_dir = full_path
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| std::path::PathBuf::from(asset_root));
    for section in ["buffers", "images"] {
        let Some(entries) = json.get(section).and_then(|v| v.as_array()) else {
            continue;
        };
        for entry in entries {
            let Some(uri) = entry.get("uri").and_then(|v| v.as_str()) else {
                continue;
            };
            if uri.starts_with("data:") {
                continue;
            }
            if !base_dir.join(uri).is_file() {
                return false;
            }
        }
    }
    true
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn receive_lightyear_asset_stream_messages(
    mut manifest_receivers: Query<
        '_,
        '_,
        &mut MessageReceiver<AssetStreamManifestMessage>,
        (With<Client>, With<Connected>),
    >,
    mut chunk_receivers: Query<
        '_,
        '_,
        &mut MessageReceiver<AssetStreamChunkMessage>,
        (With<Client>, With<Connected>),
    >,
    mut request_senders: Query<
        '_,
        '_,
        &mut MessageSender<AssetRequestMessage>,
        (With<Client>, With<Connected>),
    >,
    mut ack_senders: Query<
        '_,
        '_,
        &mut MessageSender<AssetAckMessage>,
        (With<Client>, With<Connected>),
    >,
    mut asset_manager: ResMut<'_, LocalAssetManager>,
    mut session: ResMut<'_, ClientSession>,
    asset_root: Res<'_, AssetRootPath>,
    mut watchdog: ResMut<'_, BootstrapWatchdogState>,
) {
    for mut receiver in &mut manifest_receivers {
        for manifest in receiver.receive() {
            watchdog.asset_manifest_seen = true;
            info!(
                "client received asset manifest entries={}",
                manifest.assets.len()
            );
            let is_bootstrap_manifest = !asset_manager.bootstrap_phase_complete;
            if !asset_manager.bootstrap_manifest_seen {
                asset_manager.bootstrap_manifest_seen = true;
                asset_manager.bootstrap_total_bytes = 0;
                asset_manager.bootstrap_ready_bytes = 0;
            }
            if !asset_manager.cache_index_loaded {
                let index_path = cache_index_path(&asset_root.0);
                asset_manager.cache_index = load_cache_index(&index_path).unwrap_or_default();
                asset_manager.cache_index_loaded = true;
            }
            let mut requested_assets = Vec::<RequestedAsset>::new();
            for asset in &manifest.assets {
                let target = std::path::PathBuf::from(&asset_root.0)
                    .join("data/cache_stream")
                    .join(&asset.relative_cache_path);
                let has_cached_file = std::fs::metadata(&target)
                    .ok()
                    .is_some_and(|meta| meta.len() > 0);
                let already_cached = has_cached_file
                    && asset_manager.is_cache_fresh(
                        &asset.asset_id,
                        asset.asset_version,
                        &asset.sha256_hex,
                    );
                let mut record = LocalAssetRecord {
                    relative_cache_path: asset.relative_cache_path.clone(),
                    _content_type: asset.content_type.clone(),
                    _byte_len: asset.byte_len,
                    _chunk_count: asset.chunk_count,
                    asset_version: asset.asset_version,
                    sha256_hex: asset.sha256_hex.clone(),
                    ready: already_cached,
                };
                if already_cached {
                    if is_bootstrap_manifest {
                        asset_manager.bootstrap_ready_bytes = asset_manager
                            .bootstrap_ready_bytes
                            .saturating_add(asset.byte_len);
                    }
                } else {
                    let chunk_slots = vec![None; asset.chunk_count as usize];
                    asset_manager.pending_assets.insert(
                        asset.asset_id.clone(),
                        PendingAssetChunks {
                            relative_cache_path: asset.relative_cache_path.clone(),
                            byte_len: asset.byte_len,
                            chunk_count: asset.chunk_count,
                            chunks: chunk_slots,
                            counts_toward_bootstrap: is_bootstrap_manifest,
                        },
                    );
                    record.ready = false;
                    if asset_manager
                        .requested_asset_ids
                        .insert(asset.asset_id.clone())
                    {
                        requested_assets.push(RequestedAsset {
                            asset_id: asset.asset_id.clone(),
                            known_asset_version: asset_manager
                                .cache_index
                                .by_asset_id
                                .get(&asset.asset_id)
                                .map(|entry| entry.asset_version),
                            known_sha256_hex: asset_manager
                                .cache_index
                                .by_asset_id
                                .get(&asset.asset_id)
                                .map(|entry| entry.sha256_hex.clone()),
                        });
                    }
                }
                if is_bootstrap_manifest {
                    asset_manager.bootstrap_total_bytes = asset_manager
                        .bootstrap_total_bytes
                        .saturating_add(asset.byte_len);
                }
                asset_manager
                    .records_by_asset_id
                    .insert(asset.asset_id.clone(), record);
            }
            session.status = format!(
                "Asset stream manifest received ({} assets).",
                manifest.assets.len()
            );
            if !requested_assets.is_empty() {
                let request_message = AssetRequestMessage {
                    requests: requested_assets,
                };
                for mut sender in &mut request_senders {
                    sender.send::<ControlChannel>(request_message.clone());
                }
            }
        }
    }

    for mut receiver in &mut chunk_receivers {
        for chunk in receiver.receive() {
            let mut completed_payload: Option<(String, Vec<u8>)> = None;
            if let Some(pending) = asset_manager.pending_assets.get_mut(&chunk.asset_id) {
                if pending.chunk_count != chunk.chunk_count {
                    continue;
                }
                let idx = chunk.chunk_index as usize;
                if idx >= pending.chunks.len() {
                    continue;
                }
                pending.chunks[idx] = Some(chunk.bytes.clone());
                if pending.chunks.iter().all(Option::is_some) {
                    let mut payload = Vec::<u8>::new();
                    for bytes in pending.chunks.iter().flatten() {
                        payload.extend_from_slice(bytes);
                    }
                    completed_payload = Some((pending.relative_cache_path.clone(), payload));
                }
            } else {
                continue;
            }

            if let Some((relative_cache_path, payload)) = completed_payload {
                let target = std::path::PathBuf::from(&asset_root.0)
                    .join("data/cache_stream")
                    .join(&relative_cache_path);
                if let Some(parent) = target.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                let _ = std::fs::write(&target, &payload);
                session.status = format!("Asset streamed: {}", relative_cache_path);
                let mut ack_to_send: Option<AssetAckMessage> = None;
                if let Some(record) = asset_manager.records_by_asset_id.get_mut(&chunk.asset_id) {
                    let payload_sha = sha256_hex(&payload);
                    if payload_sha != record.sha256_hex {
                        warn!(
                            "client asset checksum mismatch asset_id={} expected={} got={}",
                            chunk.asset_id, record.sha256_hex, payload_sha
                        );
                        continue;
                    }
                    record.ready = true;
                    ack_to_send = Some(AssetAckMessage {
                        asset_id: chunk.asset_id.clone(),
                        asset_version: record.asset_version,
                        sha256_hex: record.sha256_hex.clone(),
                    });
                }
                if let Some(ack) = ack_to_send {
                    asset_manager.cache_index.by_asset_id.insert(
                        ack.asset_id.clone(),
                        AssetCacheIndexRecord {
                            asset_version: ack.asset_version,
                            sha256_hex: ack.sha256_hex.clone(),
                        },
                    );
                    let index_path = cache_index_path(&asset_root.0);
                    let _ = save_cache_index(&index_path, &asset_manager.cache_index);
                    for mut sender in &mut ack_senders {
                        sender.send::<ControlChannel>(ack.clone());
                    }
                }
                if let Some(pending) = asset_manager.pending_assets.remove(&chunk.asset_id)
                    && pending.counts_toward_bootstrap
                {
                    asset_manager.bootstrap_ready_bytes = asset_manager
                        .bootstrap_ready_bytes
                        .saturating_add(pending.byte_len);
                }
                asset_manager.requested_asset_ids.remove(&chunk.asset_id);
            }
        }
    }
    if asset_manager.bootstrap_manifest_seen
        && !asset_manager.bootstrap_phase_complete
        && asset_manager
            .pending_assets
            .values()
            .all(|pending| !pending.counts_toward_bootstrap)
    {
        info!(
            "client bootstrap asset phase complete (ready_bytes={} total_bytes={})",
            asset_manager.bootstrap_ready_bytes, asset_manager.bootstrap_total_bytes
        );
        asset_manager.bootstrap_phase_complete = true;
    }
}

fn ensure_critical_assets_available_system(
    time: Res<'_, Time>,
    mut request_senders: Query<
        '_,
        '_,
        &mut MessageSender<AssetRequestMessage>,
        (With<Client>, With<Connected>),
    >,
    mut request_state: ResMut<'_, CriticalAssetRequestState>,
    asset_manager: Res<'_, LocalAssetManager>,
    asset_root: Res<'_, AssetRootPath>,
) {
    let critical_asset_ids = [
        default_corvette_asset_id(),
        default_starfield_shader_asset_id(),
        default_space_background_shader_asset_id(),
    ];
    let now = time.elapsed_secs_f64();
    let mut missing = Vec::new();
    for asset_id in critical_asset_ids {
        if !asset_present_on_disk(asset_id, &asset_manager, &asset_root.0) {
            missing.push(asset_id.to_string());
        }
    }
    if missing.is_empty() {
        return;
    }
    if now - request_state.last_request_at_s < 2.0 {
        return;
    }
    request_state.last_request_at_s = now;
    let requests = missing
        .into_iter()
        .map(|asset_id| {
            let known = asset_manager.cache_index.by_asset_id.get(&asset_id);
            RequestedAsset {
                asset_id,
                known_asset_version: known.map(|entry| entry.asset_version),
                known_sha256_hex: known.map(|entry| entry.sha256_hex.clone()),
            }
        })
        .collect::<Vec<_>>();
    let request_message = AssetRequestMessage { requests };
    for mut sender in &mut request_senders {
        sender.send::<ControlChannel>(request_message.clone());
    }
}

fn asset_present_on_disk(
    asset_id: &str,
    asset_manager: &LocalAssetManager,
    asset_root: &str,
) -> bool {
    let Some(relative_cache_path) = asset_manager
        .records_by_asset_id
        .get(asset_id)
        .map(|record| record.relative_cache_path.as_str())
        .or_else(|| match asset_id {
            id if id == default_corvette_asset_id() => Some("models/corvette_01/corvette_01.gltf"),
            id if id == default_starfield_shader_asset_id() => Some("shaders/starfield.wgsl"),
            id if id == default_space_background_shader_asset_id() => {
                Some("shaders/simple_space_background.wgsl")
            }
            _ => None,
        })
    else {
        return false;
    };
    let rooted_stream_path = std::path::PathBuf::from(asset_root)
        .join("data/cache_stream")
        .join(relative_cache_path);
    if rooted_stream_path.exists() {
        return true;
    }
    std::path::PathBuf::from(asset_root)
        .join(relative_cache_path)
        .exists()
}

#[allow(clippy::too_many_arguments)]
fn watch_in_world_bootstrap_failures(
    time: Res<'_, Time>,
    tuning: Res<'_, PredictionBootstrapTuning>,
    auth_state: Res<'_, ClientAuthSyncState>,
    mut session: ResMut<'_, ClientSession>,
    mut asset_manager: ResMut<'_, LocalAssetManager>,
    mut watchdog: ResMut<'_, BootstrapWatchdogState>,
    mut adoption_state: ResMut<'_, DeferredPredictedAdoptionState>,
    mut dialog_queue: ResMut<'_, dialog_ui::DialogQueue>,
    replicated_entities: Query<'_, '_, Entity, With<lightyear::prelude::Replicated>>,
) {
    let now = time.elapsed_secs_f64();
    if watchdog.in_world_entered_at_s.is_none() {
        watchdog.in_world_entered_at_s = Some(now);
        watchdog.last_bootstrap_progress_at_s = now;
    }

    if asset_manager.bootstrap_ready_bytes != watchdog.last_bootstrap_ready_bytes {
        watchdog.last_bootstrap_ready_bytes = asset_manager.bootstrap_ready_bytes;
        watchdog.last_bootstrap_progress_at_s = now;
    }

    let entered_at = watchdog.in_world_entered_at_s.unwrap_or(now);
    if !watchdog.replication_state_seen && !replicated_entities.is_empty() {
        watchdog.replication_state_seen = true;
    }
    let auth_bind_sent = !auth_state.sent_for_client_entities.is_empty();
    if !watchdog.timeout_dialog_shown
        && now - entered_at > 3.0
        && !asset_manager.bootstrap_manifest_seen
        && !watchdog.replication_state_seen
    {
        warn!(
            "client bootstrap timeout waiting for manifest/auth bind (auth_bind_sent={} replication_seen={} manifest_seen={})",
            auth_bind_sent, watchdog.replication_state_seen, watchdog.asset_manifest_seen
        );
        session.status = "World bootstrap timed out. Check error dialog.".to_string();
        session.ui_dirty = true;
        dialog_queue.push_error(
            "World Bootstrap Timeout",
            format!(
                "Connected to transport, but world bootstrap did not begin within 3 seconds.\n\n\
                 Diagnostics:\n\
                 - Auth bind sent: {}\n\
                 - Replication state received: {}\n\
                 - Asset manifest received: {}\n\n\
                 Likely causes:\n\
                 - Replication rejected client auth bind (JWT mismatch/missing secret)\n\
                 - Replication auth/visibility flow not bound for this player\n\n\
                 Check replication logs for: 'replication client authenticated and bound'.",
                if auth_bind_sent { "yes" } else { "no" },
                if watchdog.replication_state_seen {
                    "yes"
                } else {
                    "no"
                },
                if watchdog.asset_manifest_seen {
                    "yes"
                } else {
                    "no"
                },
            ),
        );
        watchdog.timeout_dialog_shown = true;
        if watchdog.replication_state_seen && !asset_manager.bootstrap_phase_complete {
            warn!(
                "forcing bootstrap completion in degraded mode after timeout (replication active, no manifest)"
            );
            asset_manager.bootstrap_phase_complete = true;
            session.status =
                "Replication active without manifest; continuing in degraded bootstrap mode."
                    .to_string();
            session.ui_dirty = true;
        }
    }

    if !watchdog.no_world_state_dialog_shown
        && asset_manager.bootstrap_complete()
        && !watchdog.replication_state_seen
        && now - entered_at > 10.0
    {
        warn!(
            "client bootstrap completed but no replication world state received (auth_bind_sent={} manifest_seen={})",
            auth_bind_sent, watchdog.asset_manifest_seen
        );
        session.status = "No world state received. Check error dialog.".to_string();
        session.ui_dirty = true;
        dialog_queue.push_error(
            "No World State Received",
            "Asset bootstrap completed, but no replication world state updates arrived.\n\n\
             Most likely cause: gateway bootstrap dispatch is not notifying live replication simulation.\n\
             Ensure gateway uses UDP bootstrap handoff (`GATEWAY_BOOTSTRAP_MODE=udp`) and restart gateway + replication."
                .to_string(),
        );
        watchdog.no_world_state_dialog_shown = true;
    }

    if !adoption_state.dialog_shown
        && watchdog.replication_state_seen
        && adoption_state.waiting_entity_id.is_some()
        && adoption_state
            .wait_started_at_s
            .is_some_and(|started_at_s| now - started_at_s > tuning.defer_dialog_after_s)
    {
        let wait_s = adoption_state
            .wait_started_at_s
            .map(|started_at_s| (now - started_at_s).max(0.0))
            .unwrap_or_default();
        let waiting_entity = adoption_state
            .waiting_entity_id
            .as_deref()
            .unwrap_or("<unknown>");
        warn!(
            "controlled predicted adoption stalled for {} (wait {:.2}s, missing: {})",
            waiting_entity, wait_s, adoption_state.last_missing_components
        );
        session.status = "Controlled entity adoption delayed. Check warning dialog.".to_string();
        session.ui_dirty = true;
        dialog_queue.push_warning(
            "Controlled Entity Adoption Delayed",
            format!(
                "Replication is active, but the controlled predicted entity is still waiting for required replicated Avian components.\n\n\
                 Entity: {}\n\
                 Wait time: {:.1}s\n\
                 Missing: {}\n\n\
                 This usually means component replication for the controlled entity is arriving out-of-order under load.",
                waiting_entity,
                wait_s,
                adoption_state.last_missing_components
            ),
        );
        adoption_state.dialog_shown = true;
    }

    if asset_manager.bootstrap_complete() {
        return;
    }

    if !watchdog.stream_stall_dialog_shown
        && asset_manager.bootstrap_manifest_seen
        && !asset_manager.pending_assets.is_empty()
        && now - watchdog.last_bootstrap_progress_at_s > 6.0
    {
        warn!(
            "client bootstrap stream stalled (ready_bytes={} total_bytes={} pending_assets={})",
            asset_manager.bootstrap_ready_bytes,
            asset_manager.bootstrap_total_bytes,
            asset_manager.pending_assets.len()
        );
        session.status = "Asset streaming stalled. Check error dialog.".to_string();
        session.ui_dirty = true;
        dialog_queue.push_error(
            "Asset Streaming Stalled",
            format!(
                "Received asset manifest, but bootstrap download progress has not changed for 6 seconds.\n\n\
                 Diagnostics:\n\
                 - Bootstrap ready bytes: {}\n\
                 - Bootstrap total bytes: {}\n\
                 - Pending assets: {}\n\n\
                 Check replication asset stream logs for chunk send/request/ack activity.",
                asset_manager.bootstrap_ready_bytes,
                asset_manager.bootstrap_total_bytes,
                asset_manager.pending_assets.len(),
            ),
        );
        watchdog.stream_stall_dialog_shown = true;
        if !asset_manager.bootstrap_phase_complete {
            warn!("forcing bootstrap completion in degraded mode after asset stream stall");
            asset_manager.bootstrap_phase_complete = true;
            session.status =
                "Asset bootstrap stalled; continuing in degraded mode while streaming retries."
                    .to_string();
            session.ui_dirty = true;
        }
    }
}

fn ensure_client_transport_channels(
    mut transports: Query<'_, '_, &mut Transport, With<Client>>,
    registry: Res<'_, ChannelRegistry>,
) {
    for mut transport in &mut transports {
        if !transport.has_sender::<ControlChannel>() {
            transport.add_sender_from_registry::<ControlChannel>(&registry);
        }
        if !transport.has_receiver::<ControlChannel>() {
            transport.add_receiver_from_registry::<ControlChannel>(&registry);
        }
    }
}

#[allow(clippy::type_complexity)]
fn update_owned_ships_panel_system(
    mut commands: Commands<'_, '_>,
    fonts: Res<'_, EmbeddedFonts>,
    session: Res<'_, ClientSession>,
    player_view_state: Res<'_, LocalPlayerViewState>,
    mut panel_state: ResMut<'_, OwnedShipsPanelState>,
    existing_panels: Query<'_, '_, Entity, With<OwnedShipsPanelRoot>>,
    ships: Query<
        '_,
        '_,
        (
            &Name,
            Option<&OwnerId>,
            Option<&MountedOn>,
            Option<&Hardpoint>,
            Option<&PlayerTag>,
        ),
        With<lightyear::prelude::Replicated>,
    >,
) {
    let Some(local_player_entity_id) = session.player_entity_id.as_ref() else {
        return;
    };
    let mut ship_ids = ships
        .iter()
        .filter_map(|(name, owner, mounted_on, hardpoint, player_tag)| {
            if mounted_on.is_some() || hardpoint.is_some() || player_tag.is_some() {
                return None;
            }
            if !name.as_str().starts_with("ship:") {
                return None;
            }
            if owner.is_none_or(|owner| owner.0 != *local_player_entity_id) {
                return None;
            }
            Some(name.as_str().to_string())
        })
        .collect::<Vec<_>>();
    ship_ids.sort();
    ship_ids.dedup();
    let selected_id = player_view_state
        .desired_controlled_entity_id
        .clone()
        .or_else(|| player_view_state.controlled_entity_id.clone());

    if panel_state.last_ship_ids == ship_ids
        && panel_state.last_selected_id == selected_id
        && panel_state.last_detached_mode == player_view_state.detached_free_camera
        && !existing_panels.is_empty()
    {
        return;
    }
    panel_state.last_ship_ids = ship_ids.clone();
    panel_state.last_selected_id = selected_id.clone();
    panel_state.last_detached_mode = player_view_state.detached_free_camera;

    for panel in &existing_panels {
        commands.entity(panel).despawn();
    }

    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: px(12),
                top: px(12),
                width: px(280),
                padding: UiRect::all(px(10)),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(8)),
                flex_direction: FlexDirection::Column,
                row_gap: px(8),
                ..default()
            },
            BackgroundColor(Color::srgba(0.04, 0.07, 0.11, 0.88)),
            BorderColor::all(Color::srgba(0.22, 0.34, 0.48, 0.92)),
            OwnedShipsPanelRoot,
            GameplayHud,
            WorldEntity,
            DespawnOnExit(ClientAppState::InWorld),
        ))
        .with_children(|panel| {
            panel.spawn((
                Text::new("Owned Ships"),
                TextFont {
                    font: fonts.bold.clone(),
                    font_size: 18.0,
                    ..default()
                },
                TextColor(Color::srgb(0.9, 0.95, 1.0)),
            ));

            let free_roam_selected = selected_id.as_deref()
                == Some(local_player_entity_id.as_str())
                && !player_view_state.detached_free_camera;
            panel
                .spawn((
                    Button,
                    OwnedShipsPanelButton {
                        action: OwnedShipsPanelAction::FreeRoam,
                    },
                    Node {
                        width: percent(100.0),
                        height: px(34),
                        justify_content: JustifyContent::FlexStart,
                        align_items: AlignItems::Center,
                        padding: UiRect::axes(px(10), px(0)),
                        border_radius: BorderRadius::all(px(6)),
                        ..default()
                    },
                    BackgroundColor(if free_roam_selected {
                        Color::srgba(0.26, 0.4, 0.56, 0.96)
                    } else {
                        Color::srgba(0.15, 0.2, 0.28, 0.92)
                    }),
                ))
                .with_children(|button| {
                    button.spawn((
                        Text::new("Free Roam"),
                        TextFont {
                            font: fonts.regular.clone(),
                            font_size: 14.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.95, 0.97, 1.0)),
                    ));
                });
            if ship_ids.is_empty() {
                panel.spawn((
                    Text::new("No owned ships visible"),
                    TextFont {
                        font: fonts.regular.clone(),
                        font_size: 13.0,
                        ..default()
                    },
                    TextColor(Color::srgba(0.75, 0.82, 0.9, 0.9)),
                ));
            } else {
                for ship_id in ship_ids {
                    let is_selected = selected_id.as_deref() == Some(ship_id.as_str());
                    panel
                        .spawn((
                            Button,
                            OwnedShipsPanelButton {
                                action: OwnedShipsPanelAction::ControlEntity(ship_id.clone()),
                            },
                            Node {
                                width: percent(100.0),
                                height: px(34),
                                justify_content: JustifyContent::FlexStart,
                                align_items: AlignItems::Center,
                                padding: UiRect::axes(px(10), px(0)),
                                border_radius: BorderRadius::all(px(6)),
                                ..default()
                            },
                            BackgroundColor(if is_selected {
                                Color::srgba(0.26, 0.4, 0.56, 0.96)
                            } else {
                                Color::srgba(0.15, 0.2, 0.28, 0.92)
                            }),
                        ))
                        .with_children(|button| {
                            button.spawn((
                                Text::new(ship_id),
                                TextFont {
                                    font: fonts.regular.clone(),
                                    font_size: 14.0,
                                    ..default()
                                },
                                TextColor(Color::srgb(0.95, 0.97, 1.0)),
                            ));
                        });
                }
            }
        });
}

#[allow(clippy::type_complexity)]
fn handle_owned_ships_panel_buttons(
    mut interactions: Query<
        '_,
        '_,
        (&Interaction, &OwnedShipsPanelButton, &mut BackgroundColor),
        Changed<Interaction>,
    >,
    session: Res<'_, ClientSession>,
    mut player_view_state: ResMut<'_, LocalPlayerViewState>,
    mut focus_state: ResMut<'_, CameraFocusState>,
    mut panel_state: ResMut<'_, OwnedShipsPanelState>,
) {
    for (interaction, button, mut color) in &mut interactions {
        match *interaction {
            Interaction::Pressed => {
                match &button.action {
                    OwnedShipsPanelAction::FreeRoam => {
                        player_view_state.desired_controlled_entity_id =
                            session.player_entity_id.clone();
                        player_view_state.detached_free_camera = false;
                        player_view_state.selected_entity_id = None;
                        focus_state.set(None);
                    }
                    OwnedShipsPanelAction::ControlEntity(entity_id) => {
                        player_view_state.desired_controlled_entity_id = Some(entity_id.clone());
                        player_view_state.detached_free_camera = false;
                        player_view_state.selected_entity_id = Some(entity_id.clone());
                        focus_state.set(Some(entity_id.clone()));
                    }
                }
                panel_state.last_selected_id = None;
                *color = BackgroundColor(Color::srgba(0.26, 0.4, 0.56, 0.96));
            }
            Interaction::Hovered => {
                *color = BackgroundColor(Color::srgba(0.2, 0.29, 0.41, 0.96));
            }
            Interaction::None => {
                let is_selected = match &button.action {
                    OwnedShipsPanelAction::FreeRoam => {
                        player_view_state.desired_controlled_entity_id.as_ref()
                            == session.player_entity_id.as_ref()
                            && !player_view_state.detached_free_camera
                    }
                    OwnedShipsPanelAction::ControlEntity(entity_id) => {
                        player_view_state.desired_controlled_entity_id.as_ref() == Some(entity_id)
                    }
                };
                *color = BackgroundColor(if is_selected {
                    Color::srgba(0.26, 0.4, 0.56, 0.96)
                } else {
                    Color::srgba(0.15, 0.2, 0.28, 0.92)
                });
            }
        }
    }
}

#[allow(clippy::type_complexity)]
fn update_hud_system(
    controlled_query: Query<
        '_,
        '_,
        (&Transform, Option<&LinearVelocity>, &HealthPool),
        With<ControlledEntity>,
    >,
    camera_query: Query<'_, '_, &Transform, With<GameplayCamera>>,
    mut hud_query: Query<'_, '_, &mut Text, With<HudText>>,
    focus_state: Res<'_, CameraFocusState>,
) {
    let (pos, vel, health_text) =
        if let Ok((transform, maybe_velocity, health)) = controlled_query.single() {
            let vel = maybe_velocity.map_or(Vec3::ZERO, |velocity| velocity.0);
            (
                transform.translation,
                vel,
                format!("{:.0}/{:.0}", health.current, health.maximum),
            )
        } else {
            let Ok(camera_transform) = camera_query.single() else {
                return;
            };
            (
                camera_transform.translation,
                Vec3::ZERO,
                "--/--".to_string(),
            )
        };
    let Ok(mut text) = hud_query.single_mut() else {
        return;
    };

    let heading_rad = vel.truncate().to_angle();
    // Convert math convention (CCW from +Y) to compass convention (CW from north).
    let heading_deg = {
        let raw = (-heading_rad.to_degrees()).rem_euclid(360.0);
        if raw == 0.0 { 0.0_f32 } else { raw }
    };
    let speed = Vec2::new(vel.x, vel.y).length();
    let content = format!(
        "SIDEREAL FLIGHT\nPos: ({:.0}, {:.0})\nSpeed: {:.1} m/s\nVel: ({:.1}, {:.1})\nHeading: {:.0}\u{00b0}\nHealth: {}\nFocus: {}\nControls: W/S thrust, A/D turn, SPACE brake, F focus nearest, C focus controlled, F3 debug overlay, ESC logout",
        pos.x,
        pos.y,
        speed,
        vel.x,
        vel.y,
        heading_deg,
        health_text,
        focus_state.focused_entity_id.as_deref().unwrap_or("<none>")
    );
    content.clone_into(&mut **text);
}

/// Translates the Lightyear-managed `ActionState<PlayerInput>` into `ActionQueue`
/// entries each `FixedUpdate` tick. This runs during normal simulation and during
/// rollback resimulation so the flight systems always see the correct input.
#[allow(clippy::type_complexity)]
fn apply_predicted_input_to_action_queue(
    mut commands: Commands<'_, '_>,
    mut query: Query<
        '_,
        '_,
        (Entity, &ActionState<PlayerInput>, Option<&mut ActionQueue>),
        With<ControlledEntity>,
    >,
) {
    for (entity, action_state, maybe_queue) in &mut query {
        if let Some(mut queue) = maybe_queue {
            for action in &action_state.0.actions {
                queue.push(*action);
            }
        } else {
            commands.entity(entity).insert((
                ActionQueue {
                    pending: action_state.0.actions.clone(),
                },
                default_flight_action_capabilities(),
            ));
        }
    }
}

#[allow(clippy::type_complexity)]
fn enforce_controlled_planar_motion(
    mut controlled_query: Query<
        '_,
        '_,
        (
            &mut Transform,
            Option<&mut Position>,
            Option<&mut Rotation>,
            Option<&mut LinearVelocity>,
            Option<&mut AngularVelocity>,
        ),
        With<ControlledEntity>,
    >,
) {
    for (mut transform, position, rotation, velocity, angular_velocity) in &mut controlled_query {
        if let Some(mut pos) = position {
            if !pos.0.is_finite() {
                pos.0 = Vec3::ZERO;
            }
            pos.0.z = 0.0;
        }
        if let Some(mut vel) = velocity {
            if !vel.0.is_finite() {
                vel.0 = Vec3::ZERO;
            }
            vel.0.z = 0.0;
        }
        if let Some(mut ang_vel) = angular_velocity {
            if !ang_vel.0.is_finite() {
                ang_vel.0 = Vec3::ZERO;
            }
            ang_vel.0.x = 0.0;
            ang_vel.0.y = 0.0;
        }
        if !transform.translation.is_finite() {
            transform.translation = Vec3::ZERO;
        }
        let mut heading = if let Some(rot) = rotation.as_ref() {
            if rot.0.is_finite() {
                rot.0.to_euler(EulerRot::ZYX).0
            } else {
                0.0
            }
        } else if transform.rotation.is_finite() {
            transform.rotation.to_euler(EulerRot::ZYX).0
        } else {
            0.0
        };
        if !heading.is_finite() {
            heading = 0.0;
        }
        let planar_rot = Quat::from_rotation_z(heading);
        if let Some(mut rot) = rotation {
            rot.0 = planar_rot;
        }
        transform.translation.z = 0.0;
        transform.rotation = planar_rot;
    }
}

fn toggle_debug_overlay_system(
    input: Res<'_, ButtonInput<KeyCode>>,
    mut debug_overlay: ResMut<'_, DebugOverlayEnabled>,
) {
    if input.just_pressed(KeyCode::F3) {
        debug_overlay.enabled = !debug_overlay.enabled;
    }
}

#[allow(clippy::type_complexity)]
fn draw_debug_overlay_system(
    debug_overlay: Res<'_, DebugOverlayEnabled>,
    session: Res<'_, ClientSession>,
    mut gizmos: Gizmos,
    entities: Query<
        '_,
        '_,
        (
            Entity,
            &'_ Transform,
            Option<&'_ SizeM>,
            Option<&'_ LinearVelocity>,
            Option<&'_ MountedOn>,
            Option<&'_ Hardpoint>,
            Option<&'_ ControlledEntity>,
            Option<&'_ ScannerRangeM>,
            Option<&'_ EntityGuid>,
            Option<&'_ lightyear::prelude::Confirmed<Position>>,
            Option<&'_ lightyear::prelude::Confirmed<Rotation>>,
            Has<lightyear::prelude::Predicted>,
            Has<lightyear::prelude::Replicated>,
            Has<lightyear::prelude::Interpolated>,
        ),
        With<WorldEntity>,
    >,
) {
    if !debug_overlay.enabled {
        return;
    }
    let local_player_entity_id = session.player_entity_id.as_deref();
    const VELOCITY_ARROW_SCALE: f32 = 0.5;
    const HARDPOINT_CROSS_HALF_SIZE: f32 = 2.0;
    let collision_color = Color::srgb(0.2, 0.8, 0.2);
    let velocity_color = Color::srgb(0.2, 0.5, 1.0);
    let hardpoint_color = Color::srgb(1.0, 0.8, 0.2);
    let controlled_predicted_color = Color::srgb(0.2, 1.0, 1.0);
    let controlled_confirmed_color = Color::srgb(1.0, 0.2, 1.0);
    let prediction_error_color = Color::srgb(1.0, 0.2, 0.2);
    let visibility_range_color = Color::srgb(0.9, 0.9, 0.15);
    let mut predicted_by_guid: HashMap<String, (Vec3, Quat, Vec3)> = HashMap::new();
    let mut authoritative_by_guid: HashMap<String, (Vec3, Quat, Vec3)> = HashMap::new();
    let mut controlled_root_guids: Vec<String> = Vec::new();
    let mut controlled_visibility_circle: Option<(Vec3, f32)> = None;

    for (
        _entity,
        transform,
        size_m,
        linear_velocity,
        mounted_on,
        hardpoint,
        controlled_marker,
        scanner_range,
        entity_guid,
        confirmed_position,
        confirmed_rotation,
        is_predicted,
        is_replicated,
        is_interpolated,
    ) in &entities
    {
        let pos = transform.translation;
        let rot = transform.rotation;
        let half_extents =
            size_m.map(|size| Vec3::new(size.width * 0.5, size.length * 0.5, size.height * 0.5));

        let is_local_controlled = controlled_marker.is_some_and(|controlled| {
            local_player_entity_id.is_some_and(|player_id| controlled.player_entity_id == player_id)
        });

        if let Some(half_extents) = half_extents {
            let aabb = bevy::math::bounding::Aabb3d::new(Vec3::ZERO, half_extents);
            let transform = Transform::from_translation(pos).with_rotation(rot);
            let draw_color = if is_local_controlled && mounted_on.is_none() {
                controlled_predicted_color
            } else {
                collision_color
            };
            gizmos.aabb_3d(aabb, transform, draw_color);

            // Fallback ghost path: use confirmed snapshot on the same controlled entity.
            if is_local_controlled
                && mounted_on.is_none()
                && let (Some(confirmed_position), Some(confirmed_rotation)) =
                    (confirmed_position, confirmed_rotation)
            {
                let confirmed_pos = confirmed_position.0.0;
                let confirmed_rot = confirmed_rotation.0.0;
                let confirmed_transform =
                    Transform::from_translation(confirmed_pos).with_rotation(confirmed_rot);
                gizmos.aabb_3d(aabb, confirmed_transform, controlled_confirmed_color);
                gizmos.line(pos, confirmed_pos, prediction_error_color);
            }
        }

        if mounted_on.is_none()
            && hardpoint.is_none()
            && let (Some(guid), Some(half_extents)) = (entity_guid, half_extents)
        {
            let guid = guid.0.to_string();
            if is_predicted {
                predicted_by_guid.insert(guid.clone(), (pos, rot, half_extents));
            }
            if is_replicated && !is_predicted && !is_interpolated {
                authoritative_by_guid.insert(guid.clone(), (pos, rot, half_extents));
            }
            if is_local_controlled {
                controlled_root_guids.push(guid);
                // Expected client visibility range circle.
                // Fallback to 300m when scanner range component is unavailable.
                let range_m = scanner_range
                    .map(|r| r.0.max(0.0))
                    .unwrap_or(300.0)
                    .max(1.0);
                controlled_visibility_circle = Some((pos, range_m));
            }
        }

        if mounted_on.is_none()
            && let Some(vel) = linear_velocity
        {
            let len = vel.0.length();
            if len > 0.01 {
                let end = pos + vel.0 * VELOCITY_ARROW_SCALE;
                gizmos.arrow(pos, end, velocity_color);
            }
        }

        if hardpoint.is_some() {
            let isometry = bevy::math::Isometry3d::new(pos, rot);
            gizmos.cross(isometry, HARDPOINT_CROSS_HALF_SIZE, hardpoint_color);
        }
    }

    for guid in controlled_root_guids {
        let Some((predicted_pos, _predicted_rot, predicted_half_extents)) =
            predicted_by_guid.get(&guid)
        else {
            continue;
        };
        let Some((authoritative_pos, authoritative_rot, authoritative_half_extents)) =
            authoritative_by_guid.get(&guid)
        else {
            continue;
        };
        let authoritative_aabb =
            bevy::math::bounding::Aabb3d::new(Vec3::ZERO, *authoritative_half_extents);
        let authoritative_transform =
            Transform::from_translation(*authoritative_pos).with_rotation(*authoritative_rot);
        gizmos.aabb_3d(
            authoritative_aabb,
            authoritative_transform,
            controlled_confirmed_color,
        );
        gizmos.line(*predicted_pos, *authoritative_pos, prediction_error_color);

        // Draw a second magenta box at predicted extents if dimensions mismatch.
        if (*authoritative_half_extents - *predicted_half_extents).length_squared() > 0.0001 {
            let predicted_aabb =
                bevy::math::bounding::Aabb3d::new(Vec3::ZERO, *predicted_half_extents);
            let predicted_transform = Transform::from_translation(*authoritative_pos);
            gizmos.aabb_3d(
                predicted_aabb,
                predicted_transform,
                controlled_confirmed_color,
            );
        }
    }

    if let Some((center, radius)) = controlled_visibility_circle {
        const CIRCLE_SEGMENTS: usize = 96;
        let mut prev = center + Vec3::new(radius, 0.0, 0.0);
        for i in 1..=CIRCLE_SEGMENTS {
            let t = (i as f32 / CIRCLE_SEGMENTS as f32) * std::f32::consts::TAU;
            let next = center + Vec3::new(radius * t.cos(), radius * t.sin(), 0.0);
            gizmos.line(prev, next, visibility_range_color);
            prev = next;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn logout_to_auth_system(
    input: Res<'_, ButtonInput<KeyCode>>,
    mut commands: Commands<'_, '_>,
    mut next_state: ResMut<'_, NextState<ClientAppState>>,
    mut session: ResMut<'_, ClientSession>,
    mut remote_registry: ResMut<'_, RemoteEntityRegistry>,
    mut entity_registry: ResMut<'_, RuntimeEntityHierarchy>,
    mut asset_manager: ResMut<'_, LocalAssetManager>,
    mut auth_state: ResMut<'_, ClientAuthSyncState>,
    mut focus_state: ResMut<'_, CameraFocusState>,
    mut player_view_state: ResMut<'_, LocalPlayerViewState>,
    mut character_selection: ResMut<'_, CharacterSelectionState>,
    mut free_camera: ResMut<'_, FreeCameraState>,
    mut watchdog: ResMut<'_, BootstrapWatchdogState>,
    mut ack_tracker: ResMut<'_, ClientInputAckTracker>,
    client_entities: Query<'_, '_, Entity, With<RawClient>>,
) {
    if !input.just_pressed(KeyCode::Escape) {
        return;
    }
    for entity in &client_entities {
        commands.entity(entity).despawn();
    }
    next_state.set(ClientAppState::Auth);
    session.account_id = None;
    session.player_entity_id = None;
    session.access_token = None;
    session.refresh_token = None;
    session.status = "Logged out. Back on auth screen.".to_string();
    session.ui_dirty = true;
    remote_registry.by_entity_id.clear();
    entity_registry.by_entity_id.clear();
    entity_registry.pending_children_by_parent_id.clear();
    asset_manager.pending_assets.clear();
    asset_manager.requested_asset_ids.clear();
    asset_manager.bootstrap_manifest_seen = false;
    asset_manager.bootstrap_phase_complete = false;
    asset_manager.bootstrap_total_bytes = 0;
    asset_manager.bootstrap_ready_bytes = 0;
    auth_state.sent_for_client_entities.clear();
    auth_state.last_sent_at_s_by_client_entity.clear();
    auth_state.last_player_entity_id = None;
    focus_state.set(None);
    *player_view_state = LocalPlayerViewState::default();
    *character_selection = CharacterSelectionState::default();
    *free_camera = FreeCameraState::default();
    *watchdog = BootstrapWatchdogState::default();
    *ack_tracker = ClientInputAckTracker::default();
}

fn update_starfield_material_system(
    time: Res<'_, Time>,
    camera_motion: Res<'_, CameraMotionState>,
    window_query: Query<'_, '_, &Window, With<bevy::window::PrimaryWindow>>,
    mut motion: ResMut<'_, StarfieldMotionState>,
    starfield_query: Query<'_, '_, &MeshMaterial2d<StarfieldMaterial>, With<StarfieldBackdrop>>,
    mut materials: ResMut<'_, Assets<StarfieldMaterial>>,
) {
    let Ok(window) = window_query.single() else {
        return;
    };
    if !camera_motion.initialized {
        return;
    }
    let dt = time.delta_secs().max(0.0);
    let velocity_xy = camera_motion.smoothed_velocity_xy;
    let speed = velocity_xy.length();

    if !motion.initialized {
        motion.initialized = true;
        motion.prev_speed = speed;
        motion.smoothed_warp = 0.0;
    }

    // Large same-frame camera jumps (scene/bootstrap/authority handoff) should not yank backdrop UVs.
    if camera_motion.frame_delta_xy.length() > 250.0 {
        motion.starfield_drift_uv = Vec2::ZERO;
        motion.background_drift_uv = Vec2::ZERO;
        motion.prev_speed = speed;
        motion.smoothed_warp = 0.0;
    }

    let _accel_raw = if dt > 0.0 {
        (speed - motion.prev_speed) / dt
    } else {
        0.0
    };
    motion.prev_speed = speed;

    // Integrate shared drift from velocity once; both starfield and space background consume this.
    let frame_background_step = velocity_xy * dt * 0.00003;
    let max_step = 0.03;
    motion.background_drift_uv += frame_background_step.clamp_length_max(max_step);
    motion.starfield_drift_uv = motion.background_drift_uv;
    let drift_xy = motion.background_drift_uv;

    // Keep streaking disabled in normal flight; only ramp in at high speed.
    let speed_warp_start = 500.0;
    let speed_warp_full = 2_000.0;
    let target_warp =
        ((speed - speed_warp_start) / (speed_warp_full - speed_warp_start)).clamp(0.0, 1.0);
    let warp_alpha = 1.0 - (-6.0 * dt).exp();
    motion.smoothed_warp = motion.smoothed_warp.lerp(target_warp, warp_alpha);
    let warp = motion.smoothed_warp;
    let intensity = 1.0;
    let alpha = 1.0;
    let velocity_dir = if speed > 0.001 {
        velocity_xy / speed
    } else {
        Vec2::Y
    };

    for material_handle in &starfield_query {
        if let Some(material) = materials.get_mut(&material_handle.0) {
            material.viewport_time = Vec4::new(
                window.resolution.width(),
                window.resolution.height(),
                time.elapsed_secs(),
                warp,
            );
            material.drift_intensity = Vec4::new(drift_xy.x, drift_xy.y, intensity, alpha);
            material.velocity_dir = Vec4::new(velocity_dir.x, velocity_dir.y, speed, 0.0);
        }
    }
}

fn update_space_background_material_system(
    time: Res<'_, Time>,
    camera_motion: Res<'_, CameraMotionState>,
    starfield_motion: Res<'_, StarfieldMotionState>,
    window_query: Query<'_, '_, &Window, With<bevy::window::PrimaryWindow>>,
    bg_query: Query<
        '_,
        '_,
        &MeshMaterial2d<SpaceBackgroundMaterial>,
        With<SpaceBackgroundBackdrop>,
    >,
    mut materials: ResMut<'_, Assets<SpaceBackgroundMaterial>>,
) {
    let Ok(window) = window_query.single() else {
        return;
    };
    if !camera_motion.initialized {
        return;
    }

    let drift_xy = starfield_motion.background_drift_uv;
    let velocity_xy = camera_motion.smoothed_velocity_xy;
    let speed = velocity_xy.length();
    let velocity_dir = if speed > 0.001 {
        velocity_xy / speed
    } else {
        Vec2::Y
    };

    for material_handle in &bg_query {
        if let Some(material) = materials.get_mut(&material_handle.0) {
            material.viewport_time = Vec4::new(
                window.resolution.width(),
                window.resolution.height(),
                time.elapsed_secs(),
                0.0,
            );
            material.motion = Vec4::new(drift_xy.x, drift_xy.y, velocity_dir.x, velocity_dir.y);
        }
    }
}

fn active_field_mut(session: &mut ClientSession) -> &mut String {
    match session.focus {
        FocusField::Email => &mut session.email,
        FocusField::Password => &mut session.password,
        FocusField::ResetToken => &mut session.reset_token,
        FocusField::NewPassword => &mut session.new_password,
    }
}

fn mask(value: &str) -> String {
    if value.is_empty() {
        return "".to_string();
    }
    "*".repeat(value.chars().count())
}

fn is_printable_char(chr: char) -> bool {
    let is_in_private_use_area = ('\u{e000}'..='\u{f8ff}').contains(&chr)
        || ('\u{f0000}'..='\u{ffffd}').contains(&chr)
        || ('\u{100000}'..='\u{10fffd}').contains(&chr);
    !is_in_private_use_area && !chr.is_ascii_control()
}

fn preferred_backends() -> Backends {
    Backends::from_env().unwrap_or(Backends::VULKAN | Backends::GL)
}

fn configured_wgpu_settings() -> WgpuSettings {
    let force_fallback_adapter = std::env::var("SIDEREAL_CLIENT_FORCE_SOFTWARE_ADAPTER")
        .is_ok_and(|v| v == "1" || v.eq_ignore_ascii_case("true"));
    let backends = preferred_backends();
    info!(
        "client render config backends={:?} force_fallback_adapter={}",
        backends, force_fallback_adapter
    );
    WgpuSettings {
        backends: Some(backends),
        force_fallback_adapter,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    #[test]
    fn remote_endpoint_registers_when_enabled() {
        let cfg = RemoteInspectConfig {
            enabled: true,
            bind_addr: IpAddr::V4(Ipv4Addr::LOCALHOST),
            port: 15714,
            auth_token: Some("0123456789abcdef".to_string()),
        };
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        configure_remote(&mut app, &cfg);

        assert!(
            app.world()
                .contains_resource::<bevy_remote::http::HostPort>()
        );
        assert!(app.world().contains_resource::<BrpAuthToken>());
    }

    #[test]
    fn predicted_controlled_adoption_defers_until_avian_motion_available() {
        assert!(should_defer_controlled_predicted_adoption(
            true, false, true, true
        ));
        assert!(should_defer_controlled_predicted_adoption(
            true, true, false, true
        ));
        assert!(should_defer_controlled_predicted_adoption(
            true, true, true, false
        ));
    }

    #[test]
    fn predicted_controlled_adoption_proceeds_when_requirements_met() {
        assert!(!should_defer_controlled_predicted_adoption(
            true, true, true, true
        ));
        assert!(!should_defer_controlled_predicted_adoption(
            false, false, false, false
        ));
    }
}
