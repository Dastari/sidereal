#[cfg(not(target_arch = "wasm32"))]
mod auth_ui;

#[cfg(not(target_arch = "wasm32"))]
mod dialog_ui;

#[cfg(not(target_arch = "wasm32"))]
mod prediction;

#[cfg(not(target_arch = "wasm32"))]
use avian3d::prelude::*;
#[cfg(not(target_arch = "wasm32"))]
use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
#[cfg(not(target_arch = "wasm32"))]
use bevy::reflect::TypePath;
use bevy::render::RenderPlugin;
#[cfg(not(target_arch = "wasm32"))]
use bevy::render::render_resource::AsBindGroup;
use bevy::render::settings::{Backends, RenderCreation, WgpuSettings};
#[cfg(not(target_arch = "wasm32"))]
use bevy::window::{PresentMode, Window, WindowPlugin};
#[cfg(not(target_arch = "wasm32"))]
use bevy::shader::ShaderRef;
#[cfg(not(target_arch = "wasm32"))]
use bevy::sprite_render::{AlphaMode2d, Material2d, Material2dPlugin, MeshMaterial2d};
#[cfg(not(target_arch = "wasm32"))]
use bevy::state::state_scoped::DespawnOnExit;

#[cfg(not(target_arch = "wasm32"))]
use crate::prediction::{
    EntitySnapshot, RemoteEntity, SnapshotBuffer, interpolate_remote_entities,
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
use sidereal_core::remote_inspect::RemoteInspectConfig;
#[cfg(not(target_arch = "wasm32"))]
use sidereal_game::{
    ActionCapabilities, ActionQueue, Engine, EntityAction, EntityGuid, FlightComputer, FuelTank,
    HealthPool, MountedOn, OwnerId, PositionM, SiderealGamePlugin, VelocityMps,
};
#[cfg(not(target_arch = "wasm32"))]
use sidereal_net::{
    ClientAuthMessage, ClientInputMessage, ControlChannel, InputChannel, ReplicationStateMessage,
    StateChannel, register_lightyear_protocol,
};
#[cfg(not(target_arch = "wasm32"))]
use std::collections::HashMap;
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
    status: String,
    world_snapshot: Option<WorldMeResponse>,
    ui_dirty: bool,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Resource, Default)]
struct ClientNetworkTick(u64);

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Resource, Default)]
struct ClientAuthSyncState {
    sent_for_client_entities: std::collections::HashSet<Entity>,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Resource, Default)]
struct StarfieldMotionState {
    prev_velocity_xy: Vec2,
    drift_xy: Vec2,
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
            status: "Ready. F1 Login, F2 Register, F3 Forgot Request, F4 Forgot Confirm."
                .to_string(),
            world_snapshot: None,
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
struct StreamAssetDescriptor {
    asset_id: String,
    relative_cache_path: String,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct WorldMeResponse {
    player_entity_id: String,
    ship_entity_id: String,
    ship_name: String,
    position_m: [f32; 3],
    velocity_mps: [f32; 3],
    heading_rad: f32,
    health: f32,
    max_health: f32,
    engine_max_accel_mps2: f32,
    engine_ramp_to_max_s: f32,
    model_asset_id: String,
    starfield_shader_asset_id: String,
    assets: Vec<StreamAssetDescriptor>,
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
struct ControlledShip {
    entity_id: String,
    #[allow(dead_code)]
    player_entity_id: String,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Component)]
struct RemoteShip {
    #[allow(dead_code)]
    entity_id: String,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Resource, Default)]
struct RemoteShipRegistry {
    by_entity_id: HashMap<String, Entity>,
}

#[cfg(not(target_arch = "wasm32"))]
const HARD_SNAP_THRESHOLD_M: f32 = 10.0;
#[cfg(not(target_arch = "wasm32"))]
const SMOOTH_CORRECTION_RATE: f32 = 8.0;

#[cfg(not(target_arch = "wasm32"))]
#[derive(Component)]
struct StarfieldBackdrop;

#[cfg(not(target_arch = "wasm32"))]
#[derive(Component)]
struct SpaceBackgroundBackdrop;

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
    if headless_transport {
        app.add_plugins(MinimalPlugins);
        app.add_plugins(bevy::log::LogPlugin::default());
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

    app.add_plugins(PhysicsPlugins::default().with_length_unit(1.0));
    app.insert_resource(Gravity(Vec3::ZERO));
    app.add_plugins(SiderealGamePlugin);
    app.add_plugins(ClientPlugins::default());
    register_lightyear_protocol(&mut app);
    configure_remote(&mut app, &remote_cfg);
    app.insert_resource(AssetRootPath(asset_root));
    app.insert_resource(ClientSession::default());
    app.insert_resource(ClientNetworkTick::default());
    app.insert_resource(ClientAuthSyncState::default());
    app.insert_resource(StarfieldMotionState::default());
    app.insert_resource(RemoteShipRegistry::default());
    app.add_observer(log_native_client_connected);
    app.add_systems(Startup, start_lightyear_client_transport);

    // Input-to-action runs in FixedUpdate before game systems
    app.add_systems(
        FixedUpdate,
        client_input_to_actions
            .before(sidereal_game::validate_action_capabilities)
            .run_if(in_state(ClientAppState::InWorld)),
    );

    if headless_transport {
        app.add_systems(
            Update,
            (
                ensure_client_transport_channels,
                send_lightyear_auth_messages,
                send_lightyear_input_messages,
                receive_lightyear_replication_messages,
            ),
        );
        app.add_systems(Startup, || {
            println!("sidereal-client headless transport mode");
        });
    } else {
        insert_embedded_fonts(&mut app);
        app.init_state::<ClientAppState>();
        auth_ui::register_auth_ui(&mut app);
        dialog_ui::register_dialog_ui(&mut app);
        app.add_systems(OnEnter(ClientAppState::InWorld), spawn_world_scene);
        app.add_systems(
            Update,
            (
                ensure_client_transport_channels,
                send_lightyear_auth_messages,
                send_lightyear_input_messages,
                receive_lightyear_replication_messages,
            ),
        );
        app.add_systems(
            Update,
            (
                sync_controlled_ship_from_avian,
                interpolate_remote_entities.after(receive_lightyear_replication_messages),
                sync_backdrop_fullscreen_system,
                update_topdown_camera_system,
                update_hud_system,
                logout_to_auth_system,
                update_starfield_material_system,
                update_space_background_material_system,
            )
                .run_if(in_state(ClientAppState::InWorld)),
        );
    }
    app.run();
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
    println!("native client lightyear UDP connecting {local_addr} -> {remote_addr}");
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
    asset_root: &AssetRootPath,
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
        AuthAction::ForgotRequest => (|| -> Result<(Option<AuthTokens>, Option<String>), String> {
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
            session.status = "Password reset token requested. Use F4 to confirm reset.".to_string();
            Ok((None, None::<String>))
        })(),
        AuthAction::ForgotConfirm => (|| -> Result<(Option<AuthTokens>, Option<String>), String> {
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
        })(),
    };

    match result {
        Ok((Some(tokens), _)) => {
            session.access_token = Some(tokens.access_token.clone());
            session.refresh_token = Some(tokens.refresh_token);
            match fetch_world_and_stream_assets(
                &client,
                &gateway_url,
                &tokens.access_token,
                &asset_root.0,
            ) {
                Ok(world) => {
                    session.world_snapshot = Some(world);
                    session.status =
                        "World loaded. WASD thrust/turn, SPACE brake, ESC logout.".to_string();
                    next_state.set(ClientAppState::InWorld);
                }
                Err(err) => {
                    session.status = format!("Auth OK but world load failed: {err}");
                    dialog_queue.push_error(
                        "World Load Failed",
                        format!(
                            "Authentication succeeded, but failed to load world data.\n\n\
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
fn fetch_world_and_stream_assets(
    client: &reqwest::blocking::Client,
    gateway_url: &str,
    access_token: &str,
    asset_root: &str,
) -> Result<WorldMeResponse, String> {
    let world = client
        .get(format!("{gateway_url}/world/me"))
        .bearer_auth(access_token)
        .send()
        .map_err(|err| err.to_string())?
        .error_for_status()
        .map_err(|err| err.to_string())?
        .json::<WorldMeResponse>()
        .map_err(|err| err.to_string())?;

    for asset in &world.assets {
        let bytes = client
            .get(format!("{gateway_url}/assets/stream/{}", asset.asset_id))
            .bearer_auth(access_token)
            .send()
            .map_err(|err| err.to_string())?
            .error_for_status()
            .map_err(|err| err.to_string())?
            .bytes()
            .map_err(|err| err.to_string())?;

        let target = std::path::PathBuf::from(asset_root)
            .join("data/cache_stream")
            .join(&asset.relative_cache_path);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }
        std::fs::write(&target, &bytes).map_err(|err| err.to_string())?;
    }

    Ok(world)
}

#[cfg(not(target_arch = "wasm32"))]
#[allow(clippy::too_many_arguments)]
fn spawn_world_scene(
    mut commands: Commands<'_, '_>,
    asset_server: Res<'_, AssetServer>,
    mut meshes: ResMut<'_, Assets<Mesh>>,
    mut materials: ResMut<'_, Assets<StandardMaterial>>,
    mut starfield_materials: ResMut<'_, Assets<StarfieldMaterial>>,
    mut space_bg_materials: ResMut<'_, Assets<SpaceBackgroundMaterial>>,
    mut session: ResMut<'_, ClientSession>,
    mut shaders: ResMut<'_, Assets<bevy::shader::Shader>>,
    asset_root: Res<'_, AssetRootPath>,
) {
    let Some(world) = session.world_snapshot.clone() else {
        session.status = "No world snapshot available for scene spawn.".to_string();
        return;
    };

    reload_streamed_shaders(&asset_server, &mut shaders, &asset_root.0);

    let starfield_camera = commands
        .spawn((
            Camera2d,
            Camera {
                order: -1,
                clear_color: ClearColorConfig::Custom(Color::BLACK),
                ..default()
            },
            WorldEntity,
            DespawnOnExit(ClientAppState::InWorld),
        ))
        .id();

    commands.spawn((
        Camera3d::default(),
        Camera {
            order: 0,
            clear_color: ClearColorConfig::None,
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 80.0).looking_at(Vec3::ZERO, Vec3::Y),
        TopDownCamera {
            distance: 220.0,
            target_distance: 220.0,
            min_distance: 180.0,
            max_distance: 420.0,
            zoom_units_per_wheel: 16.0,
            zoom_smoothness: 8.0,
            look_ahead_fraction: 0.4,
            look_ahead_max_speed: 250.0,
            look_ahead_smoothness: 3.0,
            look_ahead_offset: Vec2::ZERO,
        },
        WorldEntity,
        DespawnOnExit(ClientAppState::InWorld),
    ));

    let starfield_material = starfield_materials.add(StarfieldMaterial::default());
    let space_bg_material = space_bg_materials.add(SpaceBackgroundMaterial::default());

    commands.entity(starfield_camera).with_children(|children| {
        children.spawn((
            Mesh2d(meshes.add(Rectangle::new(1.0, 1.0))),
            MeshMaterial2d(space_bg_material.clone()),
            Transform::from_xyz(0.0, 0.0, -2.0),
            SpaceBackgroundBackdrop,
        ));

        children.spawn((
            Mesh2d(meshes.add(Rectangle::new(1.0, 1.0))),
            MeshMaterial2d(starfield_material.clone()),
            Transform::from_xyz(0.0, 0.0, -1.0),
            StarfieldBackdrop,
        ));
    });

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
        WorldEntity,
        DespawnOnExit(ClientAppState::InWorld),
    ));

    let ship_position = Vec3::new(
        world.position_m[0],
        world.position_m[1],
        world.position_m[2],
    );
    let ship_velocity = Vec3::new(
        world.velocity_mps[0],
        world.velocity_mps[1],
        world.velocity_mps[2],
    );

    let ship_guid = uuid::Uuid::new_v4();

    // Spawn controlled ship with Avian physics (matches server's physics setup)
    let ship = commands
        .spawn((
            Name::new(world.ship_name.clone()),
            Transform::from_translation(ship_position)
                .with_rotation(Quat::from_rotation_z(-world.heading_rad)),
            GlobalTransform::default(),
            Visibility::Visible,
            InheritedVisibility::default(),
            ViewVisibility::default(),
            ControlledShip {
                entity_id: world.ship_entity_id.clone(),
                player_entity_id: world.player_entity_id.clone(),
            },
            EntityGuid(ship_guid),
            OwnerId(world.player_entity_id.clone()),
            ActionQueue::default(),
        ))
        .insert((
            ActionCapabilities {
                supported: vec![
                    EntityAction::ThrustForward,
                    EntityAction::ThrustReverse,
                    EntityAction::ThrustNeutral,
                    EntityAction::Brake,
                    EntityAction::YawLeft,
                    EntityAction::YawRight,
                    EntityAction::YawNeutral,
                ],
            },
            FlightComputer {
                profile: "basic_fly_by_wire".to_string(),
                throttle: 0.0,
                yaw_input: 0.0,
                turn_rate_deg_s: 45.0,
            },
            HealthPool {
                current: world.health,
                maximum: world.max_health,
            },
            PositionM(ship_position),
            VelocityMps(ship_velocity),
        ))
        .insert((
            RigidBody::Dynamic,
            Collider::cuboid(6.0, 3.0, 2.0),
            Position(ship_position),
            Rotation(Quat::from_rotation_z(-world.heading_rad)),
            LinearVelocity(ship_velocity),
            AngularVelocity::default(),
            LinearDamping(0.12),
            AngularDamping(0.35),
            WorldEntity,
            DespawnOnExit(ClientAppState::InWorld),
        ))
        .id();

    // Engine as separate entity (linked by EntityGuid, same as server)
    let engine_guid = uuid::Uuid::new_v4();
    commands.spawn((
        Name::new("ControlledShip:engine"),
        EntityGuid(engine_guid),
        MountedOn {
            parent_entity_id: ship_guid,
            hardpoint_id: "engine_main".to_string(),
        },
        Engine {
            thrust_n: 140_000.0,
            burn_rate_kg_s: 0.4,
            thrust_dir: Vec3::Y,
        },
        FuelTank { fuel_kg: 1000.0 },
        OwnerId(world.player_entity_id.clone()),
        WorldEntity,
        DespawnOnExit(ClientAppState::InWorld),
    ));

    // Visual model as child of ship entity
    let streamed_scene_path = "data/cache_stream/models/corvette_01/corvette_01.gltf";
    let scene_handle =
        asset_server.load(bevy::gltf::GltfAssetLabel::Scene(0).from_asset(streamed_scene_path));
    commands.entity(ship).with_children(|child| {
        child.spawn((
            SceneRoot(scene_handle.clone()),
            Transform::from_scale(Vec3::splat(2.5)),
        ));
        child.spawn((
            Mesh3d(meshes.add(Capsule3d::new(0.8, 2.0))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgb(0.3, 0.6, 0.95),
                emissive: LinearRgba::rgb(0.03, 0.05, 0.08),
                ..default()
            })),
            Transform::from_xyz(0.0, 0.0, 0.0),
        ));
    });
}

/// Converts keyboard input into EntityActions for the local controlled ship.
/// Runs in FixedUpdate before SiderealGamePlugin's action processing systems.
#[cfg(not(target_arch = "wasm32"))]
fn client_input_to_actions(
    input: Res<'_, ButtonInput<KeyCode>>,
    mut ship_query: Query<'_, '_, &mut ActionQueue, With<ControlledShip>>,
) {
    let Ok(mut queue) = ship_query.single_mut() else {
        return;
    };

    if input.pressed(KeyCode::Space) {
        queue.push(EntityAction::Brake);
    } else if input.pressed(KeyCode::KeyW) {
        queue.push(EntityAction::ThrustForward);
    } else if input.pressed(KeyCode::KeyS) {
        queue.push(EntityAction::ThrustReverse);
    } else {
        queue.push(EntityAction::ThrustNeutral);
    }

    if input.pressed(KeyCode::KeyA) {
        queue.push(EntityAction::YawLeft);
    } else if input.pressed(KeyCode::KeyD) {
        queue.push(EntityAction::YawRight);
    } else {
        queue.push(EntityAction::YawNeutral);
    }
}

/// Syncs Avian physics state to Transform and gameplay components for the controlled ship
#[cfg(not(target_arch = "wasm32"))]
#[allow(clippy::type_complexity)]
fn sync_controlled_ship_from_avian(
    mut ships: Query<
        '_,
        '_,
        (
            &Position,
            &avian3d::prelude::Rotation,
            &LinearVelocity,
            &mut Transform,
            &mut PositionM,
            &mut VelocityMps,
        ),
        With<ControlledShip>,
    >,
) {
    for (position, rotation, velocity, mut transform, mut position_m, mut velocity_mps) in
        &mut ships
    {
        transform.translation = position.0;
        transform.rotation = rotation.0;
        position_m.0 = position.0;
        velocity_mps.0 = velocity.0;
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[allow(clippy::type_complexity)]
fn update_topdown_camera_system(
    time: Res<'_, Time>,
    mut mouse_wheel_events: MessageReader<'_, '_, MouseWheel>,
    ship_query: Query<
        '_,
        '_,
        (&Transform, &LinearVelocity),
        (With<ControlledShip>, Without<Camera3d>),
    >,
    mut camera_query: Query<
        '_,
        '_,
        (&mut Transform, &mut TopDownCamera),
        (With<Camera3d>, Without<ControlledShip>),
    >,
    window_query: Query<'_, '_, &Window, With<bevy::window::PrimaryWindow>>,
) {
    let Ok((ship_transform, ship_vel)) = ship_query.single() else {
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

    let vel_xy = ship_vel.0.truncate();
    let speed = vel_xy.length();
    let speed_factor = (speed / camera.look_ahead_max_speed).clamp(0.0, 1.0);

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

    let desired_offset = if speed > 0.01 {
        let dir = vel_xy / speed;
        Vec2::new(
            dir.x * speed_factor * half_width * camera.look_ahead_fraction,
            dir.y * speed_factor * half_height * camera.look_ahead_fraction,
        )
    } else {
        Vec2::ZERO
    };

    let alpha = 1.0 - (-camera.look_ahead_smoothness * dt).exp();
    camera.look_ahead_offset = camera.look_ahead_offset.lerp(desired_offset, alpha);

    let focus = ship_transform.translation;
    camera_transform.translation.x = focus.x + camera.look_ahead_offset.x;
    camera_transform.translation.y = focus.y + camera.look_ahead_offset.y;
    camera_transform.translation.z = camera.distance;
    camera_transform.rotation = Quat::IDENTITY;
}

#[cfg(not(target_arch = "wasm32"))]
fn sync_backdrop_fullscreen_system(
    window_query: Query<'_, '_, &Window, With<bevy::window::PrimaryWindow>>,
    mut backdrop_query: Query<
        '_,
        '_,
        &mut Transform,
        Or<(With<StarfieldBackdrop>, With<SpaceBackgroundBackdrop>)>,
    >,
) {
    let Ok(window) = window_query.single() else {
        return;
    };
    let width = window.resolution.physical_width() as f32;
    let height = window.resolution.physical_height() as f32;
    if width <= 0.0 || height <= 0.0 {
        return;
    }
    for mut transform in &mut backdrop_query {
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
fn send_lightyear_input_messages(
    input: Option<Res<'_, ButtonInput<KeyCode>>>,
    app_state: Option<Res<'_, State<ClientAppState>>>,
    session: Res<'_, ClientSession>,
    mut tick: ResMut<'_, ClientNetworkTick>,
    mut senders: Query<
        '_,
        '_,
        &mut MessageSender<ClientInputMessage>,
        (With<Client>, With<Connected>),
    >,
) {
    tick.0 = tick.0.saturating_add(1);
    if senders.is_empty() {
        if tick.0.is_multiple_of(120) {
            println!("native client waiting for connected Lightyear transport");
        }
        return;
    }

    let in_world_state = app_state
        .as_ref()
        .is_some_and(|state| **state == ClientAppState::InWorld);

    let (player_entity_id, thrust, turn, brake) = if in_world_state {
        let Some(world) = &session.world_snapshot else {
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
        (world.player_entity_id.clone(), thrust, turn, brake)
    } else {
        if !tick.0.is_multiple_of(30) {
            return;
        }
        ("transport:probe".to_string(), 0.0, 0.0, false)
    };

    let message =
        ClientInputMessage::from_axis_inputs(player_entity_id, tick.0, thrust, turn, brake);
    for mut sender in &mut senders {
        sender.send::<InputChannel>(message.clone());
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[allow(clippy::type_complexity)]
fn send_lightyear_auth_messages(
    app_state: Option<Res<'_, State<ClientAppState>>>,
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
        .is_some_and(|state| **state == ClientAppState::InWorld);
    if !in_world_state {
        return;
    }
    let Some(access_token) = session.access_token.as_ref() else {
        return;
    };
    let Some(world) = session.world_snapshot.as_ref() else {
        return;
    };

    for (client_entity, mut sender) in &mut senders {
        if auth_state.sent_for_client_entities.contains(&client_entity) {
            continue;
        }
        let auth_message = ClientAuthMessage {
            player_entity_id: world.player_entity_id.clone(),
            access_token: access_token.clone(),
        };
        sender.send::<ControlChannel>(auth_message);
        auth_state.sent_for_client_entities.insert(client_entity);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn log_native_client_connected(
    trigger: On<Add, Connected>,
    clients: Query<'_, '_, (), With<Client>>,
) {
    if clients.get(trigger.entity).is_ok() {
        println!("native client lightyear transport connected");
    }
}

/// Receives and applies server state updates:
/// - Controlled ship: reconciliation (smooth correction toward server position)
/// - Remote ships: spawn new or update snapshot buffer for interpolation
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
    mut controlled_query: Query<
        '_,
        '_,
        (
            &ControlledShip,
            &mut Position,
            &mut LinearVelocity,
            &mut avian3d::prelude::Rotation,
            &mut HealthPool,
        ),
    >,
    mut remote_registry: ResMut<'_, RemoteShipRegistry>,
    mut remote_query: Query<'_, '_, &mut SnapshotBuffer, With<RemoteShip>>,
    time: Res<'_, Time>,
    mut meshes: ResMut<'_, Assets<Mesh>>,
    mut materials: ResMut<'_, Assets<StandardMaterial>>,
) {
    for mut receiver in &mut receivers {
        for message in receiver.receive() {
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

            let dt = time.delta_secs();

            for update in &world.updates {
                if update.removed {
                    if let Some(entity) = remote_registry.by_entity_id.remove(&update.entity_id) {
                        commands.entity(entity).despawn();
                    }
                    continue;
                }

                let position = extract_vec3(&update.properties, "position_m");
                let velocity = extract_vec3(&update.properties, "velocity_mps");
                let heading = update
                    .properties
                    .get("heading_rad")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0) as f32;

                // Check if this is our controlled ship
                let is_controlled = controlled_query
                    .iter()
                    .any(|(ship, ..)| ship.entity_id == update.entity_id);

                if is_controlled {
                    // Reconciliation: smooth-correct toward server state
                    if let Ok((_, mut pos, mut vel, mut rot, mut hp)) =
                        controlled_query.single_mut()
                    {
                        if let Some(server_pos) = position {
                            let error = (server_pos - pos.0).length();
                            if error > HARD_SNAP_THRESHOLD_M {
                                pos.0 = server_pos;
                            } else if error > 0.01 {
                                let blend = (SMOOTH_CORRECTION_RATE * dt).min(1.0);
                                pos.0 = pos.0.lerp(server_pos, blend);
                            }
                        }
                        if let Some(server_vel) = velocity {
                            let blend = (SMOOTH_CORRECTION_RATE * dt).min(1.0);
                            vel.0 = vel.0.lerp(server_vel, blend);
                        }
                        let server_rot = Quat::from_rotation_z(-heading);
                        let angle_diff = rot.0.angle_between(server_rot);
                        if angle_diff > 0.01 {
                            let blend = (SMOOTH_CORRECTION_RATE * dt).min(1.0);
                            rot.0 = rot.0.slerp(server_rot, blend);
                        }

                        if let Some(hp_val) = update.properties.get("health")
                            && let Some(h) = hp_val.as_f64()
                        {
                            hp.current = h as f32;
                        }
                        if let Some(max_hp_val) = update.properties.get("max_health")
                            && let Some(mh) = max_hp_val.as_f64()
                        {
                            hp.maximum = mh as f32;
                        }
                    }
                } else {
                    // Remote ship: spawn or update
                    let server_pos = position.unwrap_or(Vec3::ZERO);
                    let server_rot = Quat::from_rotation_z(-heading);
                    let snapshot = EntitySnapshot {
                        server_time: time.elapsed_secs_f64(),
                        position_m: [server_pos.x, server_pos.y, server_pos.z],
                        rotation: [server_rot.x, server_rot.y, server_rot.z, server_rot.w],
                    };

                    if let Some(entity) = remote_registry.by_entity_id.get(&update.entity_id) {
                        // Update existing remote ship snapshot buffer
                        if let Ok(mut buffer) = remote_query.get_mut(*entity) {
                            buffer.push(snapshot);
                        }
                    } else {
                        // Spawn new remote ship
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
                                RemoteShip {
                                    entity_id: update.entity_id.clone(),
                                },
                                RemoteEntity,
                                snapshot_buffer,
                                WorldEntity,
                                DespawnOnExit(ClientAppState::InWorld),
                            ))
                            .with_children(|child| {
                                child.spawn((
                                    Mesh3d(meshes.add(Capsule3d::new(1.5, 4.0))),
                                    MeshMaterial3d(materials.add(StandardMaterial {
                                        base_color: Color::srgb(0.9, 0.3, 0.3),
                                        emissive: LinearRgba::rgb(0.1, 0.02, 0.02),
                                        ..default()
                                    })),
                                    Transform::from_xyz(0.0, 0.0, 0.0),
                                ));
                            })
                            .id();
                        remote_registry
                            .by_entity_id
                            .insert(update.entity_id.clone(), entity);
                    }
                }
            }

            session.status = format!(
                "Replication stream active. tick={} updates={}",
                message.tick,
                world.updates.len()
            );
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn extract_vec3(props: &serde_json::Value, key: &str) -> Option<Vec3> {
    let arr = props.get(key)?.as_array()?;
    if arr.len() == 3 {
        Some(Vec3::new(
            arr[0].as_f64()? as f32,
            arr[1].as_f64()? as f32,
            arr[2].as_f64()? as f32,
        ))
    } else {
        None
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
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn update_hud_system(
    ship_query: Query<
        '_,
        '_,
        (&Transform, &LinearVelocity, &HealthPool, &FlightComputer),
        With<ControlledShip>,
    >,
    mut hud_query: Query<'_, '_, &mut Text, With<HudText>>,
) {
    let Ok((transform, velocity, health, fc)) = ship_query.single() else {
        return;
    };
    let Ok(mut text) = hud_query.single_mut() else {
        return;
    };

    let pos = transform.translation;
    let vel = velocity.0;
    let heading_rad = transform.rotation.to_euler(EulerRot::ZYX).0;
    let content = format!(
        "SIDEREAL FLIGHT\nCoords: [{:.2}, {:.2}, {:.2}]\nVelocity m/s: [{:.2}, {:.2}, {:.2}] | speed {:.2}\nHeading(rad): {:.2} | throttle: {:.2}\nHealth: {:.1}/{:.1}\nControls: W/S thrust, A/D turn, SPACE brake, ESC logout",
        pos.x,
        pos.y,
        pos.z,
        vel.x,
        vel.y,
        vel.z,
        vel.length(),
        heading_rad,
        fc.throttle,
        health.current,
        health.maximum
    );
    content.clone_into(&mut **text);
}

#[cfg(not(target_arch = "wasm32"))]
fn logout_to_auth_system(
    input: Res<'_, ButtonInput<KeyCode>>,
    mut next_state: ResMut<'_, NextState<ClientAppState>>,
    mut session: ResMut<'_, ClientSession>,
    mut remote_registry: ResMut<'_, RemoteShipRegistry>,
    mut auth_state: ResMut<'_, ClientAuthSyncState>,
) {
    if !input.just_pressed(KeyCode::Escape) {
        return;
    }
    next_state.set(ClientAppState::Auth);
    session.world_snapshot = None;
    session.access_token = None;
    session.refresh_token = None;
    session.status = "Logged out. Back on auth screen.".to_string();
    session.ui_dirty = true;
    remote_registry.by_entity_id.clear();
    auth_state.sent_for_client_entities.clear();
}

#[cfg(not(target_arch = "wasm32"))]
fn update_starfield_material_system(
    time: Res<'_, Time>,
    ship_query: Query<'_, '_, &LinearVelocity, With<ControlledShip>>,
    window_query: Query<'_, '_, &Window, With<bevy::window::PrimaryWindow>>,
    mut motion: ResMut<'_, StarfieldMotionState>,
    starfield_query: Query<'_, '_, &MeshMaterial2d<StarfieldMaterial>, With<StarfieldBackdrop>>,
    mut materials: ResMut<'_, Assets<StarfieldMaterial>>,
) {
    let Ok(ship_vel) = ship_query.single() else {
        return;
    };
    let Ok(window) = window_query.single() else {
        return;
    };
    let dt = time.delta_secs();
    let velocity_xy = ship_vel.0.truncate();
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
    let speed_norm = ((speed - speed_warp_start) / (speed_warp_full - speed_warp_start))
        .clamp(0.0, 1.0);
    let accel_norm = (acceleration_xy.length() / accel_warp_full).clamp(0.0, 1.0);
    let warp = (speed_norm * 0.8 + accel_norm * 0.2).clamp(0.0, 1.0);
    let intensity = 1.15 + warp * 0.9;
    let alpha = 0.94;
    let velocity_dir = if speed > 0.001 {
        velocity_xy / speed
    } else {
        Vec2::Y
    };

    for material_handle in &starfield_query {
        if let Some(material) = materials.get_mut(&material_handle.0) {
            material.viewport_time = Vec4::new(
                window.resolution.physical_width() as f32,
                window.resolution.physical_height() as f32,
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
                window.resolution.physical_width() as f32,
                window.resolution.physical_height() as f32,
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
}
