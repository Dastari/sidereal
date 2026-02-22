#[cfg(not(target_arch = "wasm32"))]
mod auth_ui;

#[cfg(not(target_arch = "wasm32"))]
mod dialog_ui;

#[cfg(not(target_arch = "wasm32"))]
mod prediction;

#[cfg(not(target_arch = "wasm32"))]
use avian3d::prelude::*;
#[cfg(not(target_arch = "wasm32"))]
use bevy::asset::{AssetApp, AssetPlugin};
#[cfg(not(target_arch = "wasm32"))]
use bevy::camera::visibility::RenderLayers;
#[cfg(not(target_arch = "wasm32"))]
use bevy::ecs::reflect::AppTypeRegistry;
#[cfg(not(target_arch = "wasm32"))]
use bevy::input::mouse::MouseWheel;
#[cfg(not(target_arch = "wasm32"))]
use bevy::log::{info, warn};
use bevy::prelude::*;
#[cfg(not(target_arch = "wasm32"))]
use bevy::reflect::TypePath;
use bevy::render::RenderPlugin;
#[cfg(not(target_arch = "wasm32"))]
use bevy::render::render_resource::AsBindGroup;
use bevy::render::settings::{Backends, RenderCreation, WgpuSettings};
#[cfg(not(target_arch = "wasm32"))]
use bevy::scene::ScenePlugin;
#[cfg(not(target_arch = "wasm32"))]
use bevy::shader::ShaderRef;
#[cfg(not(target_arch = "wasm32"))]
use bevy::sprite_render::{
    AlphaMode2d, ColorMaterial, Material2d, Material2dPlugin, MeshMaterial2d,
};
#[cfg(not(target_arch = "wasm32"))]
use bevy::state::state_scoped::DespawnOnExit;
#[cfg(not(target_arch = "wasm32"))]
use bevy::window::{PresentMode, Window, WindowPlugin};

#[cfg(not(target_arch = "wasm32"))]
use crate::prediction::{
    EntitySnapshot, InputHistory, InputHistoryEntry, ReconciliationState, RemoteEntity,
    SnapshotBuffer, interpolate_remote_entities, replay_predicted_state_from_authoritative,
};
#[cfg(not(target_arch = "wasm32"))]
use bevy_remote::RemotePlugin;
#[cfg(not(target_arch = "wasm32"))]
use bevy_remote::http::RemoteHttpPlugin;
#[cfg(not(target_arch = "wasm32"))]
use lightyear::prelude::client::ClientPlugins;
#[cfg(not(target_arch = "wasm32"))]
use lightyear::prelude::client::{Client, Connect, Connected, RawClient};
#[cfg(not(target_arch = "wasm32"))]
use lightyear::prelude::{
    ChannelRegistry, LocalAddr, MessageManager, MessageReceiver, MessageSender, PeerAddr,
    Transport, UdpIo,
};
#[cfg(not(target_arch = "wasm32"))]
use sidereal_asset_runtime::{
    AssetCacheIndex, AssetCacheIndexRecord, cache_index_path, load_cache_index, save_cache_index,
    sha256_hex,
};
#[cfg(not(target_arch = "wasm32"))]
use sidereal_core::remote_inspect::RemoteInspectConfig;
#[cfg(not(target_arch = "wasm32"))]
use sidereal_game::{
    ActionQueue, FullscreenLayer, GeneratedComponentRegistry, HealthPool, SiderealGameCorePlugin,
    SiderealGamePlugin, default_corvette_asset_id, default_flight_action_capabilities,
    default_space_background_shader_asset_id, default_starfield_shader_asset_id,
    generated::components::{FlightTuning, TotalMassKg},
};
#[cfg(not(target_arch = "wasm32"))]
use sidereal_net::{
    AssetAckMessage, AssetRequestMessage, AssetStreamChunkMessage, AssetStreamManifestMessage,
    ClientAuthMessage, ClientInputMessage, ClientViewUpdateMessage, ControlChannel, InputChannel,
    ReplicationStateMessage, RequestedAsset, StateChannel, WorldComponentDelta,
    register_lightyear_protocol,
};
#[cfg(not(target_arch = "wasm32"))]
use sidereal_runtime_sync::{
    RuntimeEntityHierarchy, component_type_path_map, extract_f32_from_world_delta,
    extract_vec3_from_world_delta, insert_registered_components_from_world_deltas,
    register_runtime_entity, remove_runtime_entity, update_parent_link_from_properties,
};
#[cfg(not(target_arch = "wasm32"))]
use sidereal_sim_core::EntityKinematics;
#[cfg(not(target_arch = "wasm32"))]
use std::collections::{HashMap, HashSet, VecDeque};
#[cfg(not(target_arch = "wasm32"))]
use std::net::SocketAddr;
#[cfg(not(target_arch = "wasm32"))]
use std::time::{Duration, Instant};

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Resource, Clone)]
#[allow(dead_code)]
struct BrpAuthToken(String);

#[cfg(not(target_arch = "wasm32"))]
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[states(scoped_entities)]
enum ClientAppState {
    #[default]
    Auth,
    InWorld,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AuthAction {
    Login,
    Register,
    ForgotRequest,
    ForgotConfirm,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FocusField {
    Email,
    Password,
    ResetToken,
    NewPassword,
}

#[cfg(not(target_arch = "wasm32"))]
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
    player_entity_id: Option<String>,
    status: String,
    ui_dirty: bool,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Resource, Default)]
struct ClientNetworkTick(u64);

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Resource, Default)]
struct ClientInputAckTracker {
    pending_ticks: VecDeque<u64>,
    last_acked_tick: u64,
    last_server_tick: u64,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Resource, Default)]
struct ClientAuthSyncState {
    sent_for_client_entities: std::collections::HashSet<Entity>,
    last_player_entity_id: Option<String>,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Resource, Default)]
struct ClientViewUpdateTick(u64);

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Resource, Default)]
struct CameraFocusState {
    focused_entity_id: Option<String>,
}

#[cfg(not(target_arch = "wasm32"))]
impl CameraFocusState {
    fn set(&mut self, entity_id: Option<String>) {
        self.focused_entity_id = entity_id;
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, Default)]
struct PendingAssetChunks {
    relative_cache_path: String,
    byte_len: u64,
    chunk_count: u32,
    chunks: Vec<Option<Vec<u8>>>,
    counts_toward_bootstrap: bool,
}

#[cfg(not(target_arch = "wasm32"))]
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

#[cfg(not(target_arch = "wasm32"))]
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

#[cfg(not(target_arch = "wasm32"))]
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

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Resource, Default)]
struct RuntimeAssetStreamIndicatorState {
    blinking_phase_s: f32,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Resource, Default)]
struct CriticalAssetRequestState {
    last_request_at_s: f64,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Resource, Default)]
struct DebugBlueOverlayEnabled(bool);

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Resource, Default)]
struct StarfieldMotionState {
    prev_velocity_xy: Vec2,
    drift_xy: Vec2,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Resource)]
struct CameraMotionState {
    prev_position_xy: Vec2,
    velocity_xy: Vec2,
    smoothed_velocity_xy: Vec2,
}

#[cfg(not(target_arch = "wasm32"))]
impl Default for CameraMotionState {
    fn default() -> Self {
        Self {
            prev_position_xy: Vec2::ZERO,
            velocity_xy: Vec2::ZERO,
            smoothed_velocity_xy: Vec2::ZERO,
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
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

#[cfg(not(target_arch = "wasm32"))]
#[derive(Resource, Debug, Clone, Copy)]
struct HeadlessTransportMode(bool);

#[cfg(not(target_arch = "wasm32"))]
#[derive(Resource, Debug)]
struct HeadlessAccountSwitchPlan {
    switch_after_s: f64,
    switched: bool,
    next_player_entity_id: String,
    next_access_token: String,
}

#[cfg(not(target_arch = "wasm32"))]
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
            player_entity_id: None,
            status: "Ready. F1 Login, F2 Register, F3 Forgot Request, F4 Forgot Confirm."
                .to_string(),
            ui_dirty: true,
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct RegisterRequest {
    email: String,
    password: String,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct LoginRequest {
    email: String,
    password: String,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct ForgotRequest {
    email: String,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct ForgotConfirmRequest {
    reset_token: String,
    new_password: String,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct AuthTokens {
    access_token: String,
    refresh_token: String,
    token_type: String,
    expires_in_s: u64,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct ForgotResponse {
    accepted: bool,
    reset_token: Option<String>,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct ForgotConfirmResponse {
    accepted: bool,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct AuthMeResponse {
    account_id: String,
    email: String,
    player_entity_id: String,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Resource, Clone)]
struct AssetRootPath(String);

#[cfg(not(target_arch = "wasm32"))]
#[derive(Resource, Clone)]
struct EmbeddedFonts {
    bold: Handle<Font>,
    regular: Handle<Font>,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Component)]
struct WorldEntity;
#[cfg(not(target_arch = "wasm32"))]
#[derive(Component)]
struct HudText;
#[cfg(not(target_arch = "wasm32"))]
#[derive(Component)]
struct LoadingOverlayText;
#[cfg(not(target_arch = "wasm32"))]
#[derive(Component)]
struct LoadingProgressBarFill;
#[cfg(not(target_arch = "wasm32"))]
#[derive(Component)]
struct LoadingOverlayRoot;
#[cfg(not(target_arch = "wasm32"))]
#[derive(Component)]
struct RuntimeStreamingIconText;
#[cfg(not(target_arch = "wasm32"))]
#[derive(Component)]
struct GameplayCamera;
#[cfg(not(target_arch = "wasm32"))]
#[derive(Component)]
struct GameplayHud;
#[cfg(not(target_arch = "wasm32"))]
#[derive(Component)]
struct UiOverlayCamera;

#[cfg(not(target_arch = "wasm32"))]
#[derive(Component)]
struct ControlledEntity {
    entity_id: String,
    #[allow(dead_code)]
    player_entity_id: String,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Component, Default, Debug, Clone, Copy)]
struct DisplayVelocity(Vec3);

#[cfg(not(target_arch = "wasm32"))]
#[derive(Component, Debug, Clone, Copy)]
struct InterpolationState {
    prev_position: Vec3,
    prev_rotation: Quat,
    current_position: Vec3,
    current_rotation: Quat,
}

#[cfg(not(target_arch = "wasm32"))]
impl Default for InterpolationState {
    fn default() -> Self {
        Self {
            prev_position: Vec3::ZERO,
            prev_rotation: Quat::IDENTITY,
            current_position: Vec3::ZERO,
            current_rotation: Quat::IDENTITY,
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Component)]
struct RemoteVisibleEntity {
    #[allow(dead_code)]
    entity_id: String,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Component, Clone)]
struct StreamedModelAssetId(String);

#[cfg(not(target_arch = "wasm32"))]
#[derive(Component)]
struct StreamedModelVisualAttached;

#[cfg(not(target_arch = "wasm32"))]
#[derive(Resource, Default)]
struct RemoteEntityRegistry {
    by_entity_id: HashMap<String, Entity>,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone)]
struct PendingControlledState {
    message_tick: u64,
    acked_input_tick: u64,
    position_m: Vec3,
    velocity_mps: Vec3,
    heading_rad: f32,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Resource, Default)]
struct PendingControlledReconciliation {
    by_entity_id: HashMap<String, PendingControlledState>,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Resource)]
enum ClientPhysicsMode {
    /// Default: server-authoritative with client extrapolation via Avian Kinematic body.
    Predicted,
    /// Full local simulation; ignore server reconciliation entirely.
    /// Tests whether the client-side flight feel is smooth in isolation.
    Local,
    /// No client physics at all; snap to server state every tick.
    /// Tests whether raw server state is smooth without client interference.
    ServerOnly,
}

#[cfg(not(target_arch = "wasm32"))]
impl ClientPhysicsMode {
    fn from_env() -> Self {
        match std::env::var("SIDEREAL_CLIENT_PHYSICS_MODE")
            .unwrap_or_default()
            .to_lowercase()
            .as_str()
        {
            "local" => {
                eprintln!(
                    "[sidereal-client] PHYSICS MODE: local (full client simulation, no reconciliation)"
                );
                Self::Local
            }
            "server" | "server_only" | "serveronly" => {
                eprintln!(
                    "[sidereal-client] PHYSICS MODE: server-only (no client physics, snap to server)"
                );
                Self::ServerOnly
            }
            _ => Self::Predicted,
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
const HARD_SNAP_THRESHOLD_M: f32 = 10.0;
#[cfg(not(target_arch = "wasm32"))]
const SMOOTH_CORRECTION_RATE: f32 = 8.0;
#[cfg(not(target_arch = "wasm32"))]
const REPLICATION_TICK_HZ_F64: f64 = 30.0;
#[cfg(not(target_arch = "wasm32"))]
const STALE_REPLICATION_WINDOW_TICKS: u64 = 2;
#[cfg(not(target_arch = "wasm32"))]
const CAMERA_LOOK_AHEAD_DEADZONE_MPS: f32 = 10.0;
#[cfg(not(target_arch = "wasm32"))]
const CAMERA_LOOK_AHEAD_REVERSAL_DAMPING: f32 = 0.08;
#[cfg(not(target_arch = "wasm32"))]
const CAMERA_LOOK_AHEAD_MAX_OFFSET_DELTA_PER_S: f32 = 80.0;
#[cfg(not(target_arch = "wasm32"))]
const BACKDROP_RENDER_LAYER: usize = 1;

#[cfg(not(target_arch = "wasm32"))]
fn default_flight_tuning() -> FlightTuning {
    FlightTuning {
        max_linear_speed_mps: 600.0,
        max_linear_accel_mps2: 60.0,
        passive_brake_accel_mps2: 3.0,
        active_brake_accel_mps2: 12.0,
        drag_per_s: 0.0,
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn flight_tuning_from_component_deltas(components: &[WorldComponentDelta]) -> Option<FlightTuning> {
    components
        .iter()
        .find(|component| component.component_kind == "flight_tuning")
        .and_then(|component| {
            serde_json::from_value::<FlightTuning>(component.properties.clone()).ok()
        })
}

#[cfg(not(target_arch = "wasm32"))]
fn smooth_look_ahead_offset(
    current: Vec2,
    desired: Vec2,
    alpha: f32,
    max_offset_delta_per_s: f32,
    dt: f32,
) -> Vec2 {
    let mut next = current.lerp(desired, alpha.clamp(0.0, 1.0));
    let delta = next - current;
    let max_step = (max_offset_delta_per_s * dt.max(0.0)).max(0.0);
    let delta_len = delta.length();
    if max_step > 0.0 && delta_len > max_step {
        next = current + delta / delta_len * max_step;
    }
    next
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Component)]
struct StarfieldBackdrop;

#[cfg(not(target_arch = "wasm32"))]
#[derive(Component)]
struct SpaceBackgroundBackdrop;

#[cfg(not(target_arch = "wasm32"))]
#[derive(Component)]
struct DebugBlueBackdrop;

#[cfg(not(target_arch = "wasm32"))]
#[derive(Component)]
struct SpaceBackdropFallback;

#[cfg(not(target_arch = "wasm32"))]
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
struct StarfieldMaterial {
    #[uniform(0)]
    viewport_time: Vec4,
    #[uniform(1)]
    drift_intensity: Vec4,
    #[uniform(2)]
    velocity_dir: Vec4,
}

#[cfg(not(target_arch = "wasm32"))]
impl Default for StarfieldMaterial {
    fn default() -> Self {
        Self {
            viewport_time: Vec4::new(1920.0, 1080.0, 0.0, 0.0),
            drift_intensity: Vec4::new(0.0, 0.0, 1.0, 1.0),
            velocity_dir: Vec4::new(0.0, 1.0, 0.0, 0.0),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Material2d for StarfieldMaterial {
    fn fragment_shader() -> ShaderRef {
        "data/cache_stream/shaders/starfield.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
struct SpaceBackgroundMaterial {
    #[uniform(0)]
    viewport_time: Vec4,
    #[uniform(1)]
    colors: Vec4,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Component)]
struct FullscreenLayerRenderable {
    layer_kind: String,
    layer_order: i32,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Component)]
struct FallbackFullscreenLayer;

#[cfg(not(target_arch = "wasm32"))]
impl Default for SpaceBackgroundMaterial {
    fn default() -> Self {
        Self {
            viewport_time: Vec4::new(1920.0, 1080.0, 0.0, 1.0),
            colors: Vec4::new(0.05, 0.08, 0.15, 1.0),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Material2d for SpaceBackgroundMaterial {
    fn fragment_shader() -> ShaderRef {
        "data/cache_stream/shaders/simple_space_background.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Opaque
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Component)]
struct TopDownCamera {
    distance: f32,
    target_distance: f32,
    min_distance: f32,
    max_distance: f32,
    zoom_units_per_wheel: f32,
    zoom_smoothness: f32,
    look_ahead_fraction: f32,
    look_ahead_max_speed: f32,
    look_ahead_smoothness: f32,
    look_ahead_offset: Vec2,
    filtered_velocity_xy: Vec2,
    focus_smoothness: f32,
    filtered_focus_xy: Vec2,
    focus_initialized: bool,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Resource, Debug)]
struct FrameRateCap {
    frame_duration: Duration,
    last_frame_end: Instant,
}

#[cfg(not(target_arch = "wasm32"))]
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

#[cfg(not(target_arch = "wasm32"))]
fn main() {
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
    app.insert_resource(Time::<Fixed>::from_hz(30.0));
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
                    render_creation: RenderCreation::Automatic(WgpuSettings {
                        backends: Some(preferred_backends()),
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
        );
        ensure_shader_placeholders(&asset_root);
        app.add_plugins(Material2dPlugin::<StarfieldMaterial>::default());
        app.add_plugins(Material2dPlugin::<SpaceBackgroundMaterial>::default());
        if let Some(frame_cap) = FrameRateCap::from_env(120) {
            app.insert_resource(frame_cap);
            app.add_systems(Last, enforce_frame_rate_cap_system);
        }
    }

    let physics_mode = ClientPhysicsMode::from_env();
    app.add_plugins(PhysicsPlugins::default().with_length_unit(1.0));
    app.insert_resource(Gravity(Vec3::ZERO));
    if physics_mode == ClientPhysicsMode::Local {
        app.add_plugins(SiderealGamePlugin);
    } else {
        app.add_plugins(SiderealGameCorePlugin);
    }
    app.add_plugins(ClientPlugins::default());
    register_lightyear_protocol(&mut app);
    configure_remote(&mut app, &remote_cfg);
    app.insert_resource(AssetRootPath(asset_root));
    app.insert_resource(physics_mode);
    app.insert_resource(ClientSession::default());
    app.insert_resource(ClientNetworkTick::default());
    app.insert_resource(ClientInputAckTracker::default());
    app.insert_resource(ClientAuthSyncState::default());
    app.insert_resource(ClientViewUpdateTick::default());
    app.insert_resource(LocalAssetManager::default());
    app.insert_resource(RuntimeAssetStreamIndicatorState::default());
    app.insert_resource(CriticalAssetRequestState::default());
    let debug_blue_overlay = std::env::var("SIDEREAL_DEBUG_BLUE_FULLSCREEN")
        .is_ok_and(|v| v == "1" || v.eq_ignore_ascii_case("true"));
    app.insert_resource(DebugBlueOverlayEnabled(debug_blue_overlay));
    app.insert_resource(CameraFocusState::default());
    app.insert_resource(RuntimeEntityHierarchy::default());
    app.insert_resource(StarfieldMotionState::default());
    app.insert_resource(CameraMotionState::default());
    app.insert_resource(BootstrapWatchdogState::default());
    app.insert_resource(RemoteEntityRegistry::default());
    app.insert_resource(PendingControlledReconciliation::default());
    app.insert_resource(HeadlessTransportMode(headless_transport));
    if headless_transport {
        app.init_resource::<dialog_ui::DialogQueue>();
    }
    app.add_observer(log_native_client_connected);
    app.add_systems(Startup, start_lightyear_client_transport);
    if !headless_transport {
        app.add_systems(Startup, spawn_ui_overlay_camera);
    }

    if headless_transport {
        app.add_systems(Startup, configure_headless_session_from_env);
        app.add_systems(
            FixedUpdate,
            (
                send_lightyear_input_messages,
                apply_controlled_reconciliation_fixed_step,
                refresh_predicted_input_history_state,
            )
                .chain(),
        );
        app.add_systems(
            Update,
            (
                apply_headless_account_switch_system,
                ensure_client_transport_channels,
                send_lightyear_auth_messages,
                send_lightyear_view_updates,
                receive_lightyear_asset_stream_messages,
                ensure_critical_assets_available_system
                    .after(receive_lightyear_asset_stream_messages),
                receive_lightyear_replication_messages,
                update_focus_target_system,
            ),
        );
        app.add_systems(Startup, || {
            info!("sidereal-client headless transport mode");
        });
    } else {
        insert_embedded_fonts(&mut app);
        app.init_state::<ClientAppState>();
        auth_ui::register_auth_ui(&mut app);
        dialog_ui::register_dialog_ui(&mut app);
        app.add_systems(
            OnEnter(ClientAppState::InWorld),
            (
                spawn_world_scene,
                reset_bootstrap_watchdog_on_enter_in_world,
            ),
        );
        app.add_systems(
            Update,
            (
                ensure_client_transport_channels,
                send_lightyear_auth_messages,
                send_lightyear_view_updates,
                receive_lightyear_asset_stream_messages,
                ensure_critical_assets_available_system
                    .after(receive_lightyear_asset_stream_messages),
                receive_lightyear_replication_messages,
                update_focus_target_system,
            ),
        );
        app.add_systems(
            Update,
            (
                interpolate_remote_entities.after(receive_lightyear_replication_messages),
                ensure_fullscreen_layer_fallback_system
                    .after(receive_lightyear_replication_messages),
                attach_streamed_model_visuals_system.after(receive_lightyear_asset_stream_messages),
                sync_fullscreen_layer_renderables_system
                    .after(receive_lightyear_replication_messages),
                sync_backdrop_fullscreen_system
                    .after(sync_fullscreen_layer_renderables_system),
                gate_gameplay_camera_system,
                update_loading_overlay_system,
                update_runtime_stream_icon_system,
                watch_in_world_bootstrap_failures,
                interpolate_controlled_transform,
                update_topdown_camera_system.after(interpolate_controlled_transform),
                update_camera_motion_state.after(update_topdown_camera_system),
                update_hud_system,
                logout_to_auth_system,
                update_starfield_material_system.after(update_camera_motion_state),
                update_space_background_material_system.after(update_camera_motion_state),
            )
                .run_if(in_state(ClientAppState::InWorld)),
        );
        app.add_systems(
            FixedUpdate,
            (
                send_lightyear_input_messages,
                apply_controlled_reconciliation_fixed_step,
                enforce_controlled_planar_motion,
                refresh_predicted_input_history_state,
            )
                .chain()
                .run_if(in_state(ClientAppState::InWorld)),
        );
    }
    app.run();
}

#[cfg(not(target_arch = "wasm32"))]
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

#[cfg(not(target_arch = "wasm32"))]
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

#[cfg(not(target_arch = "wasm32"))]
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

#[cfg(target_arch = "wasm32")]
fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(RenderPlugin {
        render_creation: RenderCreation::Automatic(WgpuSettings {
            backends: Some(preferred_backends()),
            ..Default::default()
        }),
        ..Default::default()
    }));
    app.add_systems(Startup, || {
        info!("sidereal-client wasm scaffold booted (WebGPU-capable)");
    });
    app.run();
}

#[cfg(not(target_arch = "wasm32"))]
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

#[cfg(not(target_arch = "wasm32"))]
const STREAMED_SHADER_PATHS: &[&str] = &[
    "data/cache_stream/shaders/starfield.wgsl",
    "data/cache_stream/shaders/simple_space_background.wgsl",
];

#[cfg(not(target_arch = "wasm32"))]
const LOCAL_SHADER_FALLBACK_PATHS: &[&str] = &[
    "data/shaders/starfield.wgsl",
    "data/shaders/simple_space_background.wgsl",
];

#[cfg(not(target_arch = "wasm32"))]
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

#[cfg(not(target_arch = "wasm32"))]
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

#[cfg(not(target_arch = "wasm32"))]
fn streamed_shader_path_for_asset_id(shader_asset_id: &str) -> Option<&'static str> {
    match shader_asset_id {
        "starfield_wgsl" => Some(STREAMED_SHADER_PATHS[0]),
        "space_background_wgsl" => Some(STREAMED_SHADER_PATHS[1]),
        _ => None,
    }
}

#[cfg(not(target_arch = "wasm32"))]
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

#[cfg(not(target_arch = "wasm32"))]
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

#[cfg(not(target_arch = "wasm32"))]
fn start_lightyear_client_transport(mut commands: Commands<'_, '_>) {
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

#[cfg(not(target_arch = "wasm32"))]
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

#[cfg(not(target_arch = "wasm32"))]
fn submit_auth_request(
    session: &mut ClientSession,
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
                    session.player_entity_id = Some(me.player_entity_id);
                    session.status =
                        "Authenticated. Waiting for replication world bootstrap...".to_string();
                    next_state.set(ClientAppState::InWorld);
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

#[cfg(not(target_arch = "wasm32"))]
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

#[cfg(not(target_arch = "wasm32"))]
#[allow(clippy::too_many_arguments)]
fn spawn_world_scene(
    mut commands: Commands<'_, '_>,
    asset_server: Res<'_, AssetServer>,
    fonts: Res<'_, EmbeddedFonts>,
    mut session: ResMut<'_, ClientSession>,
    mut shaders: ResMut<'_, Assets<bevy::shader::Shader>>,
    mut meshes: ResMut<'_, Assets<Mesh>>,
    mut color_materials: ResMut<'_, Assets<ColorMaterial>>,
    asset_root: Res<'_, AssetRootPath>,
    debug_blue_overlay: Res<'_, DebugBlueOverlayEnabled>,
) {
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
            distance: 220.0,
            target_distance: 220.0,
            min_distance: 180.0,
            max_distance: 420.0,
            zoom_units_per_wheel: 16.0,
            zoom_smoothness: 8.0,
            look_ahead_fraction: 0.12,
            look_ahead_max_speed: 400.0,
            look_ahead_smoothness: 1.2,
            look_ahead_offset: Vec2::ZERO,
            filtered_velocity_xy: Vec2::ZERO,
            focus_smoothness: 12.0,
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

#[cfg(not(target_arch = "wasm32"))]
#[allow(clippy::type_complexity)]
fn update_topdown_camera_system(
    time: Res<'_, Time>,
    mut mouse_wheel_events: MessageReader<'_, '_, MouseWheel>,
    controlled_query: Query<
        '_,
        '_,
        (&Transform, &DisplayVelocity),
        (With<ControlledEntity>, Without<Camera3d>),
    >,
    mut camera_query: Query<
        '_,
        '_,
        (&mut Transform, &mut TopDownCamera),
        (With<Camera3d>, Without<ControlledEntity>),
    >,
    window_query: Query<'_, '_, &Window, With<bevy::window::PrimaryWindow>>,
) {
    let Ok((controlled_transform, display_velocity)) = controlled_query.single() else {
        return;
    };
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

    let raw_vel_xy = display_velocity.0.truncate();
    let mut velocity_alpha = 1.0 - (-(camera.look_ahead_smoothness * 0.6) * dt).exp();
    if camera.filtered_velocity_xy.dot(raw_vel_xy) < 0.0 {
        // Damp rapid sign flips so look-ahead doesn't snap to the opposite side.
        velocity_alpha *= CAMERA_LOOK_AHEAD_REVERSAL_DAMPING;
    }
    camera.filtered_velocity_xy = camera.filtered_velocity_xy.lerp(raw_vel_xy, velocity_alpha);
    let speed = camera.filtered_velocity_xy.length();
    let speed_factor = if speed <= CAMERA_LOOK_AHEAD_DEADZONE_MPS {
        0.0
    } else {
        ((speed - CAMERA_LOOK_AHEAD_DEADZONE_MPS)
            / (camera.look_ahead_max_speed - CAMERA_LOOK_AHEAD_DEADZONE_MPS).max(1.0))
        .clamp(0.0, 1.0)
    };

    let fov_y = std::f32::consts::FRAC_PI_4;
    let half_height = camera.distance * (fov_y / 2.0).tan();
    let aspect = if let Ok(window) = window_query.single() {
        let w = window.resolution.physical_width() as f32;
        let h = window.resolution.physical_height() as f32;
        if h > 0.0 { w / h } else { 16.0 / 9.0 }
    } else {
        16.0 / 9.0
    };
    let half_width = half_height * aspect;

    let desired_offset = if speed_factor > 0.0 {
        let dir = camera.filtered_velocity_xy / speed;
        Vec2::new(
            dir.x * speed_factor * half_width * camera.look_ahead_fraction,
            dir.y * speed_factor * half_height * camera.look_ahead_fraction,
        )
    } else {
        Vec2::ZERO
    };

    let alpha = 1.0 - (-camera.look_ahead_smoothness * dt).exp();
    camera.look_ahead_offset = smooth_look_ahead_offset(
        camera.look_ahead_offset,
        desired_offset,
        alpha,
        CAMERA_LOOK_AHEAD_MAX_OFFSET_DELTA_PER_S,
        dt,
    );

    let focus_xy = controlled_transform.translation.truncate();
    if !camera.focus_initialized {
        camera.filtered_focus_xy = focus_xy;
        camera.focus_initialized = true;
    } else {
        if (focus_xy - camera.filtered_focus_xy).length() > camera.distance * 0.75 {
            // Reconciliation/teleport catch-up: avoid losing the controlled ship off-screen.
            camera.filtered_focus_xy = focus_xy;
        }
        let focus_alpha = 1.0 - (-camera.focus_smoothness * dt).exp();
        camera.filtered_focus_xy = camera.filtered_focus_xy.lerp(focus_xy, focus_alpha);
    }
    camera_transform.translation.x = camera.filtered_focus_xy.x + camera.look_ahead_offset.x;
    camera_transform.translation.y = camera.filtered_focus_xy.y + camera.look_ahead_offset.y;
    camera_transform.translation.z = camera.distance;
    camera_transform.rotation = Quat::IDENTITY;
}

#[cfg(not(target_arch = "wasm32"))]
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

    if dt > 0.0 {
        motion.velocity_xy = (current_xy - motion.prev_position_xy) / dt;
    }
    motion.prev_position_xy = current_xy;

    let smooth_alpha = (8.0 * dt).min(1.0);
    motion.smoothed_velocity_xy = motion.smoothed_velocity_xy.lerp(motion.velocity_xy, smooth_alpha);
}

#[cfg(not(target_arch = "wasm32"))]
fn update_focus_target_system(
    input: Option<Res<'_, ButtonInput<KeyCode>>>,
    mut focus_state: ResMut<'_, CameraFocusState>,
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
        && let Some((controlled, _)) = controlled_query.iter().next()
    {
        focus_state.set(Some(controlled.entity_id.clone()));
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
            focus_state.set(nearest_remote);
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn gate_gameplay_camera_system(
    asset_manager: Res<'_, LocalAssetManager>,
    mut camera_query: Query<'_, '_, &mut Camera, With<GameplayCamera>>,
    mut hud_query: Query<'_, '_, &mut Visibility, With<GameplayHud>>,
) {
    let ready = asset_manager.bootstrap_complete();
    for mut camera in &mut camera_query {
        camera.is_active = ready;
    }
    for mut visibility in &mut hud_query {
        *visibility = if ready {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

#[cfg(not(target_arch = "wasm32"))]
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

#[cfg(not(target_arch = "wasm32"))]
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

#[cfg(not(target_arch = "wasm32"))]
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

#[cfg(not(target_arch = "wasm32"))]
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

#[cfg(not(target_arch = "wasm32"))]
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

#[cfg(not(target_arch = "wasm32"))]
fn enforce_frame_rate_cap_system(mut frame_cap: ResMut<'_, FrameRateCap>) {
    let elapsed = frame_cap.last_frame_end.elapsed();
    if elapsed < frame_cap.frame_duration {
        std::thread::sleep(frame_cap.frame_duration - elapsed);
    }
    frame_cap.last_frame_end = Instant::now();
}

#[cfg(not(target_arch = "wasm32"))]
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn send_lightyear_input_messages(
    input: Option<Res<'_, ButtonInput<KeyCode>>>,
    app_state: Option<Res<'_, State<ClientAppState>>>,
    headless_mode: Res<'_, HeadlessTransportMode>,
    watchdog: Res<'_, BootstrapWatchdogState>,
    mut commands: Commands<'_, '_>,
    session: Res<'_, ClientSession>,
    mut tick: ResMut<'_, ClientNetworkTick>,
    mut ack_tracker: ResMut<'_, ClientInputAckTracker>,
    mut controlled_input_history: Query<
        '_,
        '_,
        (
            Entity,
            &ControlledEntity,
            Option<&Position>,
            Option<&Rotation>,
            &Transform,
            &DisplayVelocity,
            Option<&mut ActionQueue>,
            &mut InputHistory,
        ),
    >,
    mut senders: Query<
        '_,
        '_,
        &mut MessageSender<ClientInputMessage>,
        (With<Client>, With<Connected>),
    >,
    physics_mode: Res<'_, ClientPhysicsMode>,
) {
    tick.0 = tick.0.saturating_add(1);
    if senders.is_empty() {
        if tick.0.is_multiple_of(120) {
            warn!("native client waiting for connected Lightyear transport");
        }
        return;
    }
    if !watchdog.replication_state_seen && !headless_mode.0 {
        return;
    }

    let in_world_state = app_state
        .as_ref()
        .is_some_and(|state| **state == ClientAppState::InWorld)
        || headless_mode.0;

    let (player_entity_id, thrust, turn, brake) = if in_world_state {
        let Some(player_entity_id) = session.player_entity_id.clone() else {
            return;
        };
        let brake = input
            .as_ref()
            .is_some_and(|keys| keys.pressed(KeyCode::Space));
        let thrust = if brake {
            0.0
        } else if input
            .as_ref()
            .is_some_and(|keys| keys.pressed(KeyCode::KeyW))
        {
            1.0
        } else if input
            .as_ref()
            .is_some_and(|keys| keys.pressed(KeyCode::KeyS))
        {
            -0.7
        } else {
            0.0
        };
        let turn = if input
            .as_ref()
            .is_some_and(|keys| keys.pressed(KeyCode::KeyA))
        {
            1.0
        } else if input
            .as_ref()
            .is_some_and(|keys| keys.pressed(KeyCode::KeyD))
        {
            -1.0
        } else {
            0.0
        };
        (player_entity_id, thrust, turn, brake)
    } else {
        return;
    };

    let message =
        ClientInputMessage::from_axis_inputs(player_entity_id.clone(), tick.0, thrust, turn, brake);
    for mut sender in &mut senders {
        sender.send::<InputChannel>(message.clone());
    }
    ack_tracker.pending_ticks.push_back(tick.0);
    while ack_tracker.pending_ticks.len() > 512 {
        ack_tracker.pending_ticks.pop_front();
    }
    for (entity, controlled, position, rotation, transform, display_velocity, maybe_actions, mut history) in
        &mut controlled_input_history
    {
        if controlled.player_entity_id != player_entity_id {
            continue;
        }
        let apply_local = headless_mode.0 || *physics_mode == ClientPhysicsMode::Local;
        if apply_local {
            if let Some(mut queue) = maybe_actions {
                for action in &message.actions {
                    queue.push(*action);
                }
            } else {
                commands.entity(entity).insert((
                    ActionQueue {
                        pending: message.actions.clone(),
                    },
                    default_flight_action_capabilities(),
                ));
            }
        }
        let current_pos = position.map_or(transform.translation, |p| p.0);
        let current_rot = rotation.map_or(transform.rotation, |r| r.0);
        let predicted_state = EntityKinematics {
            position_m: current_pos.to_array(),
            velocity_mps: display_velocity.0.to_array(),
            heading_rad: current_rot.to_euler(EulerRot::ZYX).0,
        };
        history.push(InputHistoryEntry {
            tick: tick.0,
            thrust,
            turn,
            brake,
            predicted_state,
        });
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[allow(clippy::type_complexity)]
fn send_lightyear_auth_messages(
    app_state: Option<Res<'_, State<ClientAppState>>>,
    headless_mode: Res<'_, HeadlessTransportMode>,
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
        auth_state.last_player_entity_id = Some(player_entity_id.clone());
    }

    for (client_entity, mut sender) in &mut senders {
        if auth_state.sent_for_client_entities.contains(&client_entity) {
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
    }
}

#[cfg(not(target_arch = "wasm32"))]
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
    controlled_query: Query<'_, '_, (&ControlledEntity, &Transform)>,
    focus_state: Res<'_, CameraFocusState>,
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
    if !view_tick.0.is_multiple_of(10) {
        return;
    }

    let controlled = controlled_query.iter().next();
    let camera_position = camera_query
        .single()
        .ok()
        .map(|t| t.translation)
        .or_else(|| controlled.map(|(_, t)| t.translation))
        .unwrap_or(Vec3::ZERO);
    let controlled_entity_id = controlled.map(|(c, _)| c.entity_id.clone());
    let focused_entity_id = focus_state.focused_entity_id.clone();
    let message = ClientViewUpdateMessage {
        player_entity_id: player_entity_id.clone(),
        focused_entity_id,
        controlled_entity_id,
        camera_position_m: [camera_position.x, camera_position.y, camera_position.z],
    };

    for mut sender in &mut senders {
        sender.send::<ControlChannel>(message.clone());
    }
}

#[cfg(not(target_arch = "wasm32"))]
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

#[cfg(not(target_arch = "wasm32"))]
fn log_native_client_connected(
    trigger: On<Add, Connected>,
    clients: Query<'_, '_, (), With<Client>>,
) {
    if clients.get(trigger.entity).is_ok() {
        info!("native client lightyear transport connected");
    }
}

/// Receives and applies server state updates:
/// - Controlled entity: reconciliation (smooth correction toward server position)
/// - Remote entities: spawn new or update snapshot buffer for interpolation
#[cfg(not(target_arch = "wasm32"))]
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn receive_lightyear_replication_messages(
    mut commands: Commands<'_, '_>,
    mut receivers: Query<
        '_,
        '_,
        &mut MessageReceiver<ReplicationStateMessage>,
        (With<Client>, With<Connected>),
    >,
    mut session: ResMut<'_, ClientSession>,
    mut dialog_queue: ResMut<'_, dialog_ui::DialogQueue>,
    controlled_query: Query<'_, '_, (Entity, &ControlledEntity)>,
    mut remote_registry: ResMut<'_, RemoteEntityRegistry>,
    mut pending_reconciliation: ResMut<'_, PendingControlledReconciliation>,
    mut entity_registry: ResMut<'_, RuntimeEntityHierarchy>,
    mut focus_state: ResMut<'_, CameraFocusState>,
    mut remote_query: Query<'_, '_, &mut SnapshotBuffer, With<RemoteVisibleEntity>>,
    component_registry: Res<'_, GeneratedComponentRegistry>,
    app_type_registry: Res<'_, AppTypeRegistry>,
    mut asset_manager: ResMut<'_, LocalAssetManager>,
    mut watchdog: ResMut<'_, BootstrapWatchdogState>,
    mut ack_tracker: ResMut<'_, ClientInputAckTracker>,
    physics_mode: Res<'_, ClientPhysicsMode>,
) {
    let Some(local_player_entity_id) = session.player_entity_id.clone() else {
        return;
    };
    let type_paths = component_type_path_map(&component_registry);
    let mut despawned_entities = HashSet::<Entity>::new();

    for mut receiver in &mut receivers {
        for message in receiver.receive() {
            if !watchdog.replication_state_seen {
                info!(
                    "client received first replication state tick={}",
                    message.tick
                );
            }
            watchdog.replication_state_seen = true;
            if message.tick.saturating_add(STALE_REPLICATION_WINDOW_TICKS)
                < ack_tracker.last_server_tick
            {
                debug!(
                    "dropping stale replication tick={} last_seen_tick={}",
                    message.tick, ack_tracker.last_server_tick
                );
                continue;
            }
            ack_tracker.last_server_tick = ack_tracker.last_server_tick.max(message.tick);
            ack_tracker.last_acked_tick = ack_tracker.last_acked_tick.max(message.acked_input_tick);
            while ack_tracker
                .pending_ticks
                .front()
                .is_some_and(|tick| *tick <= ack_tracker.last_acked_tick)
            {
                ack_tracker.pending_ticks.pop_front();
            }
            let world = match message.decode_world() {
                Ok(w) => w,
                Err(err) => {
                    let error_msg = format!(
                        "Failed to decode replication state at tick {}.\n\n\
                         Details: {err}\n\n\
                         This usually means:\n\
                         • Backend server needs to be restarted/recompiled\n\
                         • Protocol version mismatch between client and server\n\
                         • Corrupted network packet",
                        message.tick
                    );
                    eprintln!(
                        "native client failed decoding replication state tick={} from Lightyear: {err}",
                        message.tick
                    );
                    dialog_queue.push_error("Replication Protocol Error", error_msg);
                    continue;
                }
            };

            for update in &world.updates {
                if update.removed {
                    if focus_state.focused_entity_id.as_deref() == Some(update.entity_id.as_str()) {
                        focus_state.set(None);
                    }
                    if let Some(entity) =
                        entity_registry.by_entity_id.get(&update.entity_id).copied()
                    {
                        queue_despawn_once(&mut commands, &mut despawned_entities, entity);
                    }
                    if let Some((entity, ..)) = controlled_query
                        .iter()
                        .find(|(_, controlled)| controlled.entity_id == update.entity_id)
                    {
                        queue_despawn_once(&mut commands, &mut despawned_entities, entity);
                        pending_reconciliation
                            .by_entity_id
                            .remove(&update.entity_id);
                    }
                    if let Some(entity) = remote_registry.by_entity_id.remove(&update.entity_id) {
                        queue_despawn_once(&mut commands, &mut despawned_entities, entity);
                    }
                    remove_runtime_entity(&mut entity_registry, &update.entity_id);
                    continue;
                }

                let position = extract_vec3_from_world_delta(update, "position_m");
                let velocity = extract_vec3_from_world_delta(update, "velocity_mps");
                let has_spatial_state = position.is_some();
                let heading = extract_f32_from_world_delta(update, "heading_rad").unwrap_or(0.0);
                let server_pos = position.unwrap_or(Vec3::ZERO);
                let server_vel = velocity.unwrap_or(Vec3::ZERO);
                let server_rot = Quat::from_rotation_z(heading);
                let update_player_entity_id = update
                    .properties
                    .get("player_entity_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                let accepts_player_input = update
                    .properties
                    .get("accepts_player_input")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let is_controlled_entity =
                    accepts_player_input && update_player_entity_id == local_player_entity_id;
                if has_spatial_state {
                    debug!(
                        entity_id = %update.entity_id,
                        local_player = %local_player_entity_id,
                        update_player = %update_player_entity_id,
                        accepts_input = accepts_player_input,
                        is_controlled = is_controlled_entity,
                        "client evaluating entity for controlled spawn"
                    );
                }
                let update_flight_tuning = flight_tuning_from_component_deltas(&update.components);
                let asset_id = update
                    .properties
                    .get("asset_id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                let existing_controlled = controlled_query
                    .iter()
                    .find(|(_, controlled)| controlled.entity_id == update.entity_id)
                    .map(|(entity, _)| entity);

                if !has_spatial_state {
                    if let Some(entity) =
                        entity_registry.by_entity_id.get(&update.entity_id).copied()
                    {
                        insert_registered_components_from_world_deltas(
                            &mut commands,
                            entity,
                            &update.components,
                            &type_paths,
                            &app_type_registry,
                        );
                        update_parent_link_from_properties(
                            &mut commands,
                            &mut entity_registry,
                            entity,
                            &update.entity_id,
                            &update.properties,
                        );
                        update_streamed_model_asset_tag(&mut commands, entity, asset_id.as_deref());
                    } else {
                        let entity = commands
                            .spawn((
                                Name::new(update.entity_id.clone()),
                                Transform::default(),
                                GlobalTransform::default(),
                                Visibility::Visible,
                                InheritedVisibility::default(),
                                ViewVisibility::default(),
                                WorldEntity,
                                DespawnOnExit(ClientAppState::InWorld),
                            ))
                            .id();
                        register_runtime_entity(
                            &mut entity_registry,
                            update.entity_id.clone(),
                            entity,
                        );
                        insert_registered_components_from_world_deltas(
                            &mut commands,
                            entity,
                            &update.components,
                            &type_paths,
                            &app_type_registry,
                        );
                        update_parent_link_from_properties(
                            &mut commands,
                            &mut entity_registry,
                            entity,
                            &update.entity_id,
                            &update.properties,
                        );
                        update_streamed_model_asset_tag(&mut commands, entity, asset_id.as_deref());
                    }
                    continue;
                }

                if is_controlled_entity {
                    // Ensure we treat this entity as controlled and not remote.
                    if let Some(entity) = remote_registry.by_entity_id.remove(&update.entity_id) {
                        queue_despawn_once(&mut commands, &mut despawned_entities, entity);
                    }

                    if existing_controlled.is_none() {
                        info!(
                            entity_id = %update.entity_id,
                            player_entity_id = %update_player_entity_id,
                            "client spawning controlled entity"
                        );
                        let owner_id = update_player_entity_id.to_string();
                        let ft = update_flight_tuning.unwrap_or_else(default_flight_tuning);
                        let controlled_entity = match *physics_mode {
                            ClientPhysicsMode::Local => {
                                let e = commands
                                    .spawn((
                                        Name::new(update.entity_id.clone()),
                                        Transform::from_translation(server_pos)
                                            .with_rotation(server_rot),
                                        GlobalTransform::default(),
                                        ControlledEntity {
                                            entity_id: update.entity_id.clone(),
                                            player_entity_id: owner_id.clone(),
                                        },
                                        Position(server_pos),
                                        Rotation(server_rot),
                                        LinearVelocity(server_vel),
                                        AngularVelocity(Vec3::ZERO),
                                        RigidBody::Dynamic,
                                        ft,
                                        InputHistory::default(),
                                        ReconciliationState::default(),
                                        WorldEntity,
                                    ))
                                    .id();
                                commands.entity(e).insert((
                                    Collider::cuboid(6.0, 3.0, 2.0),
                                    LockedAxes::new()
                                        .lock_translation_z()
                                        .lock_rotation_x()
                                        .lock_rotation_y(),
                                    LinearDamping(0.0),
                                    AngularDamping(0.0),
                                    ActionQueue::default(),
                                    DespawnOnExit(ClientAppState::InWorld),
                                ));
                                e
                            }
                            ClientPhysicsMode::ServerOnly => commands
                                .spawn((
                                    Name::new(update.entity_id.clone()),
                                    Transform::from_translation(server_pos)
                                        .with_rotation(server_rot),
                                    GlobalTransform::default(),
                                    ControlledEntity {
                                        entity_id: update.entity_id.clone(),
                                        player_entity_id: owner_id.clone(),
                                    },
                                    LinearVelocity(server_vel),
                                    AngularVelocity(Vec3::ZERO),
                                    ft,
                                    InputHistory::default(),
                                    ReconciliationState::default(),
                                    WorldEntity,
                                    DespawnOnExit(ClientAppState::InWorld),
                                ))
                                .id(),
                            ClientPhysicsMode::Predicted => commands
                                .spawn((
                                    Name::new(update.entity_id.clone()),
                                    Transform::from_translation(server_pos)
                                        .with_rotation(server_rot),
                                    GlobalTransform::default(),
                                    ControlledEntity {
                                        entity_id: update.entity_id.clone(),
                                        player_entity_id: owner_id.clone(),
                                    },
                                    Position(server_pos),
                                    Rotation(server_rot),
                                    LinearVelocity(server_vel),
                                    AngularVelocity(Vec3::ZERO),
                                    ft,
                                    InputHistory::default(),
                                    ReconciliationState::default(),
                                    WorldEntity,
                                    DespawnOnExit(ClientAppState::InWorld),
                                ))
                                .id(),
                        };
                        commands.entity(controlled_entity).insert((
                            Visibility::Visible,
                            InheritedVisibility::default(),
                            ViewVisibility::default(),
                            DisplayVelocity(server_vel),
                            InterpolationState {
                                prev_position: server_pos,
                                prev_rotation: server_rot,
                                current_position: server_pos,
                                current_rotation: server_rot,
                            },
                        ));
                        pending_reconciliation.by_entity_id.insert(
                            update.entity_id.clone(),
                            PendingControlledState {
                                message_tick: message.tick,
                                acked_input_tick: message.acked_input_tick,
                                position_m: server_pos,
                                velocity_mps: server_vel,
                                heading_rad: heading,
                            },
                        );
                        register_runtime_entity(
                            &mut entity_registry,
                            update.entity_id.clone(),
                            controlled_entity,
                        );
                        insert_registered_components_from_world_deltas(
                            &mut commands,
                            controlled_entity,
                            &update.components,
                            &type_paths,
                            &app_type_registry,
                        );
                        update_parent_link_from_properties(
                            &mut commands,
                            &mut entity_registry,
                            controlled_entity,
                            &update.entity_id,
                            &update.properties,
                        );
                        update_streamed_model_asset_tag(
                            &mut commands,
                            controlled_entity,
                            asset_id.as_deref(),
                        );
                        if focus_state.focused_entity_id.is_none() {
                            focus_state.set(Some(update.entity_id.clone()));
                        }
                        continue;
                    }

                    if let Some(entity) = existing_controlled {
                        let entry = pending_reconciliation
                            .by_entity_id
                            .entry(update.entity_id.clone())
                            .or_insert(PendingControlledState {
                                message_tick: message.tick,
                                acked_input_tick: message.acked_input_tick,
                                position_m: server_pos,
                                velocity_mps: server_vel,
                                heading_rad: heading,
                            });
                        if message.tick >= entry.message_tick {
                            *entry = PendingControlledState {
                                message_tick: message.tick,
                                acked_input_tick: message.acked_input_tick,
                                position_m: server_pos,
                                velocity_mps: server_vel,
                                heading_rad: heading,
                            };
                        }
                        insert_registered_components_from_world_deltas(
                            &mut commands,
                            entity,
                            &update.components,
                            &type_paths,
                            &app_type_registry,
                        );
                        update_parent_link_from_properties(
                            &mut commands,
                            &mut entity_registry,
                            entity,
                            &update.entity_id,
                            &update.properties,
                        );
                        update_streamed_model_asset_tag(&mut commands, entity, asset_id.as_deref());
                    }
                } else {
                    // Remote entity: spawn or update
                    let snapshot = EntitySnapshot {
                        server_time: message.tick as f64 / REPLICATION_TICK_HZ_F64,
                        position_m: [server_pos.x, server_pos.y, server_pos.z],
                        rotation: [server_rot.x, server_rot.y, server_rot.z, server_rot.w],
                    };

                    if let Some(entity) = remote_registry.by_entity_id.get(&update.entity_id) {
                        // Update existing remote entity snapshot buffer
                        if let Ok(mut buffer) = remote_query.get_mut(*entity) {
                            buffer.push(snapshot);
                        }
                        insert_registered_components_from_world_deltas(
                            &mut commands,
                            *entity,
                            &update.components,
                            &type_paths,
                            &app_type_registry,
                        );
                        update_parent_link_from_properties(
                            &mut commands,
                            &mut entity_registry,
                            *entity,
                            &update.entity_id,
                            &update.properties,
                        );
                        update_streamed_model_asset_tag(
                            &mut commands,
                            *entity,
                            asset_id.as_deref(),
                        );
                    } else {
                        // Spawn new remote entity
                        let mut snapshot_buffer = SnapshotBuffer::default();
                        snapshot_buffer.push(snapshot);
                        let entity = commands
                            .spawn((
                                Name::new(format!("Remote:{}", update.entity_id)),
                                Transform::from_translation(server_pos).with_rotation(server_rot),
                                GlobalTransform::default(),
                                Visibility::Visible,
                                InheritedVisibility::default(),
                                ViewVisibility::default(),
                                RemoteVisibleEntity {
                                    entity_id: update.entity_id.clone(),
                                },
                                RemoteEntity,
                                snapshot_buffer,
                                WorldEntity,
                                DespawnOnExit(ClientAppState::InWorld),
                            ))
                            .id();
                        register_runtime_entity(
                            &mut entity_registry,
                            update.entity_id.clone(),
                            entity,
                        );
                        insert_registered_components_from_world_deltas(
                            &mut commands,
                            entity,
                            &update.components,
                            &type_paths,
                            &app_type_registry,
                        );
                        update_parent_link_from_properties(
                            &mut commands,
                            &mut entity_registry,
                            entity,
                            &update.entity_id,
                            &update.properties,
                        );
                        update_streamed_model_asset_tag(&mut commands, entity, asset_id.as_deref());
                        remote_registry
                            .by_entity_id
                            .insert(update.entity_id.clone(), entity);
                    }
                }
            }

            session.status = format!(
                "Replication stream active. tick={} ack={} pending_inputs={} updates={}",
                message.tick,
                ack_tracker.last_acked_tick,
                ack_tracker.pending_ticks.len(),
                world.updates.len()
            );
            if !asset_manager.bootstrap_phase_complete
                && !asset_manager.bootstrap_manifest_seen
                && asset_manager.pending_assets.is_empty()
                && !world.updates.is_empty()
            {
                warn!(
                    "client bootstrap manifest missing; unlocking gameplay camera on replication state fallback"
                );
                asset_manager.bootstrap_phase_complete = true;
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[allow(clippy::type_complexity)]
fn apply_controlled_reconciliation_fixed_step(
    fixed_time: Res<'_, Time<Fixed>>,
    mut pending_reconciliation: ResMut<'_, PendingControlledReconciliation>,
    mut controlled_query: Query<
        '_,
        '_,
        (
            &ControlledEntity,
            &mut Transform,
            Option<&mut Position>,
            Option<&mut Rotation>,
            Option<&mut LinearVelocity>,
            Option<&mut AngularVelocity>,
            &mut InputHistory,
            &mut ReconciliationState,
            &mut DisplayVelocity,
            &mut InterpolationState,
            Option<&TotalMassKg>,
            Option<&FlightTuning>,
        ),
    >,
    physics_mode: Res<'_, ClientPhysicsMode>,
) {
    if *physics_mode == ClientPhysicsMode::Local {
        pending_reconciliation.by_entity_id.clear();
        return;
    }

    let dt = fixed_time.delta_secs();
    for (
        controlled,
        _transform,
        mut position,
        mut rotation,
        velocity,
        angular_velocity,
        mut history,
        mut reconciliation,
        mut display_velocity,
        mut interpolation,
        total_mass,
        flight_tuning,
    ) in &mut controlled_query
    {
        if let Some(pending) = pending_reconciliation.by_entity_id.remove(&controlled.entity_id) {
            reconciliation.last_server_tick = pending.message_tick;
            reconciliation.last_acked_input_tick = pending.acked_input_tick;
            reconciliation.last_authoritative_state = Some(EntityKinematics {
                position_m: pending.position_m.to_array(),
                velocity_mps: pending.velocity_mps.to_array(),
                heading_rad: pending.heading_rad,
            });
            history.prune_before_tick(pending.acked_input_tick);
        }

        let Some(authoritative) = reconciliation.last_authoritative_state else {
            continue;
        };

        if *physics_mode == ClientPhysicsMode::ServerOnly {
            let server_pos = Vec3::from_array(authoritative.position_m);
            let server_rot = Quat::from_rotation_z(authoritative.heading_rad);
            display_velocity.0 = Vec3::from_array(authoritative.velocity_mps);
            if let Some(mut vel) = velocity {
                vel.0 = Vec3::ZERO;
            }
            if let Some(mut ang_vel) = angular_velocity {
                ang_vel.0 = Vec3::ZERO;
            }
            if let Some(mut pos) = position {
                pos.0 = server_pos;
            }
            if let Some(mut rot) = rotation {
                rot.0 = server_rot;
            }
            // Update interpolation state
            interpolation.prev_position = interpolation.current_position;
            interpolation.prev_rotation = interpolation.current_rotation;
            interpolation.current_position = server_pos;
            interpolation.current_rotation = server_rot;
            reconciliation.correction_error_m = 0.0;
            continue;
        }

        let mass_kg = total_mass.map(|m| m.0.max(1.0)).unwrap_or(15_000.0);
        let available_thrust = 15_000.0 * 60.0; // Approximation of typical engine budget for starter ship
        let brake_available_thrust = 15_000.0 * 12.0;

        let (replayed, _) = replay_predicted_state_from_authoritative(
            authoritative,
            &history,
            reconciliation.last_acked_input_tick,
            mass_kg,
            flight_tuning,
            available_thrust,
            brake_available_thrust,
        );
        let replayed_position = Vec3::from_array(replayed.position_m);
        let replayed_velocity = Vec3::from_array(replayed.velocity_mps);
        let replayed_heading = replayed.heading_rad;
        let replayed_rotation = Quat::from_rotation_z(replayed_heading);
        let current_pos = position.as_ref().map_or(interpolation.current_position, |p| p.0);
        let error = replayed_position.distance(current_pos);
        reconciliation.correction_error_m = error;
        reconciliation.correction_timer = 0.0;

        display_velocity.0 = replayed_velocity;
        if let Some(mut vel) = velocity {
            vel.0 = Vec3::ZERO;
        }
        if let Some(mut ang_vel) = angular_velocity {
            ang_vel.0 = Vec3::ZERO;
        }

        if error > HARD_SNAP_THRESHOLD_M {
            if let Some(ref mut pos) = position {
                pos.0 = replayed_position;
            }
            if let Some(ref mut rot) = rotation {
                rot.0 = replayed_rotation;
            }
        } else {
            let blend = (SMOOTH_CORRECTION_RATE * dt).min(1.0);
            if let Some(ref mut pos) = position {
                pos.0 = pos.0.lerp(replayed_position, blend);
            }
            if let Some(ref mut rot) = rotation {
                rot.0 = rot.0.slerp(replayed_rotation, blend);
            }
        }
        let final_pos = position.as_ref().map_or(replayed_position, |p| p.0);
        let final_rot = rotation.as_ref().map_or(replayed_rotation, |r| r.0);

        // Update interpolation state: copy current to prev, set new current
        interpolation.prev_position = interpolation.current_position;
        interpolation.prev_rotation = interpolation.current_rotation;
        interpolation.current_position = final_pos;
        interpolation.current_rotation = final_rot;

        // Don't update Transform here - let the interpolation system in Update handle it
        // This ensures smooth visuals between 30Hz physics steps

        // Force Avian's Position and Rotation to match our manual integration
        if let Some(mut pos) = position {
            pos.0 = final_pos;
        }
        if let Some(mut rot) = rotation {
            rot.0 = final_rot;
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn interpolate_controlled_transform(
    fixed_time: Res<'_, Time<Fixed>>,
    mut controlled_query: Query<'_, '_, (&InterpolationState, &mut Transform), With<ControlledEntity>>,
) {
    let t = fixed_time.overstep_fraction();
    for (interpolation, mut transform) in &mut controlled_query {
        transform.translation = interpolation
            .prev_position
            .lerp(interpolation.current_position, t);
        transform.rotation = interpolation
            .prev_rotation
            .slerp(interpolation.current_rotation, t);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn refresh_predicted_input_history_state(
    tick: Res<'_, ClientNetworkTick>,
    session: Res<'_, ClientSession>,
    mut controlled_query: Query<
        '_,
        '_,
        (
            &ControlledEntity,
            Option<&Position>,
            Option<&Rotation>,
            &Transform,
            &DisplayVelocity,
            &mut InputHistory,
        ),
    >,
) {
    let Some(player_entity_id) = session.player_entity_id.as_deref() else {
        return;
    };
    for (controlled, position, rotation, transform, display_velocity, mut history) in &mut controlled_query {
        if controlled.player_entity_id != player_entity_id {
            continue;
        }
        let Some(entry) = history
            .entries
            .iter_mut()
            .rev()
            .find(|entry| entry.tick == tick.0)
        else {
            continue;
        };
        let current_pos = position.map_or(transform.translation, |p| p.0);
        let current_rot = rotation.map_or(transform.rotation, |r| r.0);
        entry.predicted_state = EntityKinematics {
            position_m: current_pos.to_array(),
            velocity_mps: display_velocity.0.to_array(),
            heading_rad: current_rot.to_euler(EulerRot::ZYX).0,
        };
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn streamed_model_scene_path(asset_id: &str, asset_manager: &LocalAssetManager) -> Option<String> {
    let relative = asset_manager.cached_relative_path(asset_id)?;
    if !(relative.ends_with(".gltf") || relative.ends_with(".glb")) {
        return None;
    }
    Some(format!("data/cache_stream/{relative}"))
}

#[cfg(not(target_arch = "wasm32"))]
fn queue_despawn_once(
    commands: &mut Commands<'_, '_>,
    despawned_entities: &mut HashSet<Entity>,
    entity: Entity,
) {
    if despawned_entities.insert(entity) {
        commands.entity(entity).despawn();
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn update_streamed_model_asset_tag(
    commands: &mut Commands<'_, '_>,
    entity: Entity,
    asset_id: Option<&str>,
) {
    let Ok(mut entity_commands) = commands.get_entity(entity) else {
        return;
    };
    if let Some(asset_id) = asset_id {
        entity_commands.try_insert(StreamedModelAssetId(asset_id.to_string()));
    } else {
        entity_commands.remove::<(StreamedModelAssetId, StreamedModelVisualAttached)>();
    }
}

#[cfg(not(target_arch = "wasm32"))]
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

#[cfg(not(target_arch = "wasm32"))]
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

#[cfg(not(target_arch = "wasm32"))]
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

#[cfg(not(target_arch = "wasm32"))]
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

#[cfg(not(target_arch = "wasm32"))]
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

#[cfg(not(target_arch = "wasm32"))]
fn watch_in_world_bootstrap_failures(
    time: Res<'_, Time>,
    auth_state: Res<'_, ClientAuthSyncState>,
    mut session: ResMut<'_, ClientSession>,
    mut asset_manager: ResMut<'_, LocalAssetManager>,
    mut watchdog: ResMut<'_, BootstrapWatchdogState>,
    mut dialog_queue: ResMut<'_, dialog_ui::DialogQueue>,
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
        && now - entered_at > 3.0
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

#[cfg(not(target_arch = "wasm32"))]
fn ensure_client_transport_channels(
    mut transports: Query<'_, '_, &mut Transport, With<Client>>,
    registry: Res<'_, ChannelRegistry>,
) {
    for mut transport in &mut transports {
        if !transport.has_sender::<ControlChannel>() {
            transport.add_sender_from_registry::<ControlChannel>(&registry);
        }
        if !transport.has_sender::<InputChannel>() {
            transport.add_sender_from_registry::<InputChannel>(&registry);
        }
        if !transport.has_receiver::<StateChannel>() {
            transport.add_receiver_from_registry::<StateChannel>(&registry);
        }
        if !transport.has_receiver::<ControlChannel>() {
            transport.add_receiver_from_registry::<ControlChannel>(&registry);
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn update_hud_system(
    controlled_query: Query<
        '_,
        '_,
        (&Transform, &DisplayVelocity, &HealthPool),
        With<ControlledEntity>,
    >,
    mut hud_query: Query<'_, '_, &mut Text, With<HudText>>,
    focus_state: Res<'_, CameraFocusState>,
) {
    let Ok((transform, display_velocity, health)) = controlled_query.single() else {
        return;
    };
    let Ok(mut text) = hud_query.single_mut() else {
        return;
    };

    let pos = transform.translation;
    let vel = display_velocity.0;
    let heading_rad = transform.rotation.to_euler(EulerRot::ZYX).0;
    // Convert math convention (CCW from +Y) to compass convention (CW from north).
    let heading_deg = {
        let raw = (-heading_rad.to_degrees()).rem_euclid(360.0);
        if raw == 0.0 { 0.0_f32 } else { raw }
    };
    let speed = Vec2::new(vel.x, vel.y).length();
    let content = format!(
        "SIDEREAL FLIGHT\nPos: ({:.0}, {:.0})\nSpeed: {:.1} m/s\nVel: ({:.1}, {:.1})\nHeading: {:.0}\u{00b0}\nHealth: {:.0}/{:.0}\nFocus: {}\nControls: W/S thrust, A/D turn, SPACE brake, F focus nearest, C focus controlled, ESC logout",
        pos.x,
        pos.y,
        speed,
        vel.x,
        vel.y,
        heading_deg,
        health.current,
        health.maximum,
        focus_state.focused_entity_id.as_deref().unwrap_or("<none>")
    );
    content.clone_into(&mut **text);
}

#[cfg(not(target_arch = "wasm32"))]
#[allow(clippy::type_complexity)]
fn enforce_controlled_planar_motion(
    mut controlled_query: Query<
        '_,
        '_,
        (
            &mut Transform,
            Option<&mut Position>,
            Option<&mut Rotation>,
            &mut LinearVelocity,
            &mut AngularVelocity,
        ),
        With<ControlledEntity>,
    >,
) {
    for (mut transform, position, rotation, mut velocity, mut angular_velocity) in
        &mut controlled_query
    {
        if let Some(mut pos) = position {
            pos.0.z = 0.0;
        }
        velocity.0.z = 0.0;
        angular_velocity.0.x = 0.0;
        angular_velocity.0.y = 0.0;
        let heading = transform.rotation.to_euler(EulerRot::ZYX).0;
        let planar_rot = Quat::from_rotation_z(heading);
        if let Some(mut rot) = rotation {
            rot.0 = planar_rot;
        }
        transform.translation.z = 0.0;
        transform.rotation = planar_rot;
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[allow(clippy::too_many_arguments)]
fn logout_to_auth_system(
    input: Res<'_, ButtonInput<KeyCode>>,
    mut next_state: ResMut<'_, NextState<ClientAppState>>,
    mut session: ResMut<'_, ClientSession>,
    mut remote_registry: ResMut<'_, RemoteEntityRegistry>,
    mut entity_registry: ResMut<'_, RuntimeEntityHierarchy>,
    mut asset_manager: ResMut<'_, LocalAssetManager>,
    mut auth_state: ResMut<'_, ClientAuthSyncState>,
    mut focus_state: ResMut<'_, CameraFocusState>,
    mut watchdog: ResMut<'_, BootstrapWatchdogState>,
    mut pending_reconciliation: ResMut<'_, PendingControlledReconciliation>,
    mut ack_tracker: ResMut<'_, ClientInputAckTracker>,
) {
    if !input.just_pressed(KeyCode::Escape) {
        return;
    }
    next_state.set(ClientAppState::Auth);
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
    auth_state.last_player_entity_id = None;
    focus_state.set(None);
    *watchdog = BootstrapWatchdogState::default();
    pending_reconciliation.by_entity_id.clear();
    *ack_tracker = ClientInputAckTracker::default();
}

#[cfg(not(target_arch = "wasm32"))]
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
    let dt = time.delta_secs();
    let velocity_xy = camera_motion.smoothed_velocity_xy;
    let acceleration_xy = if dt > 0.0 {
        (velocity_xy - motion.prev_velocity_xy) / dt
    } else {
        Vec2::ZERO
    };
    motion.prev_velocity_xy = velocity_xy;

    motion.drift_xy += velocity_xy * (0.00014 * dt);
    motion.drift_xy.x = motion.drift_xy.x.rem_euclid(1.0);
    motion.drift_xy.y = motion.drift_xy.y.rem_euclid(1.0);

    let speed = velocity_xy.length();
    let speed_warp_start = 70.0;
    let speed_warp_full = 320.0;
    let accel_warp_full = 120.0;
    let speed_norm =
        ((speed - speed_warp_start) / (speed_warp_full - speed_warp_start)).clamp(0.0, 1.0);
    let accel_norm = (acceleration_xy.length() / accel_warp_full).clamp(0.0, 1.0);
    let warp = (speed_norm * 0.8 + accel_norm * 0.2).clamp(0.0, 1.0);
    let intensity = 1.45 + warp * 1.05;
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
            material.drift_intensity =
                Vec4::new(motion.drift_xy.x, motion.drift_xy.y, intensity, alpha);
            material.velocity_dir = Vec4::new(velocity_dir.x, velocity_dir.y, speed, accel_norm);
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn update_space_background_material_system(
    time: Res<'_, Time>,
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

    for material_handle in &bg_query {
        if let Some(material) = materials.get_mut(&material_handle.0) {
            material.viewport_time = Vec4::new(
                window.resolution.width(),
                window.resolution.height(),
                time.elapsed_secs(),
                0.0,
            );
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn active_field_mut(session: &mut ClientSession) -> &mut String {
    match session.focus {
        FocusField::Email => &mut session.email,
        FocusField::Password => &mut session.password,
        FocusField::ResetToken => &mut session.reset_token,
        FocusField::NewPassword => &mut session.new_password,
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn mask(value: &str) -> String {
    if value.is_empty() {
        return "".to_string();
    }
    "*".repeat(value.chars().count())
}

#[cfg(not(target_arch = "wasm32"))]
fn is_printable_char(chr: char) -> bool {
    let is_in_private_use_area = ('\u{e000}'..='\u{f8ff}').contains(&chr)
        || ('\u{f0000}'..='\u{ffffd}').contains(&chr)
        || ('\u{100000}'..='\u{10fffd}').contains(&chr);
    !is_in_private_use_area && !chr.is_ascii_control()
}

#[cfg(target_arch = "wasm32")]
fn preferred_backends() -> Backends {
    Backends::BROWSER_WEBGPU | Backends::GL
}

#[cfg(not(target_arch = "wasm32"))]
fn preferred_backends() -> Backends {
    Backends::VULKAN | Backends::GL
}

#[cfg(all(test, not(target_arch = "wasm32")))]
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
    fn smooth_look_ahead_offset_caps_per_frame_delta() {
        let current = Vec2::ZERO;
        let desired = Vec2::new(1000.0, 0.0);
        let dt = 1.0 / 60.0;
        let next = smooth_look_ahead_offset(current, desired, 1.0, 120.0, dt);
        let max_step = 120.0 * dt;
        assert!((next.length() - max_step).abs() < 1e-4);
    }
}
