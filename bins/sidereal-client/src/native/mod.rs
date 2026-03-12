mod auth_ui;
mod dev_console;
mod dialog_ui;
mod ecs_util;

mod app_state;
mod asset_loading_ui;
mod assets;
mod audio;
mod auth_net;
mod backdrop;
mod bootstrap;
mod camera;
mod components;
mod config;
mod control;
mod debug_overlay;
mod input;
mod lighting;
mod logout;
mod motion;
mod owner_manifest;
mod pause_menu;
mod platform;
#[cfg(not(target_arch = "wasm32"))]
mod platform_io;
mod plugins;
#[cfg(not(target_arch = "wasm32"))]
mod remote;
mod render_layers;
mod replication;
mod resources;
mod scene;
mod scene_world;
mod shaders;
mod tactical;
mod transforms;
mod transport;
mod ui;
mod visuals;
mod world_loading_ui;

pub(crate) use app_state::*;
pub(crate) use auth_net::submit_auth_request;
pub(crate) use backdrop::{
    AsteroidSpriteShaderMaterial, PlanetVisualMaterial, RuntimeEffectMaterial,
    SpaceBackgroundMaterial, SpaceBackgroundNebulaMaterial, StarfieldMaterial,
    StreamedSpriteShaderMaterial, TacticalMapOverlayMaterial,
};
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use config::*;
#[allow(unused_imports)]
pub(crate) use dev_console::build_log_capture_layer;
pub(crate) use platform::*;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use remote::*;
pub(crate) use resources::*;

use avian2d::prelude::*;
use bevy::app::PluginGroupBuilder;
#[cfg(not(target_arch = "wasm32"))]
use bevy::asset::{AssetApp, AssetPlugin};
use bevy::camera::visibility::RenderLayers;
use bevy::diagnostic::FrameTimeDiagnosticsPlugin;
use bevy::gizmos::config::GizmoConfigStore;
#[cfg(not(target_arch = "wasm32"))]
use bevy::log::LogPlugin;
use bevy::prelude::*;
#[cfg(not(target_arch = "wasm32"))]
use bevy::render::RenderPlugin;
#[cfg(not(target_arch = "wasm32"))]
use bevy::render::settings::RenderCreation;
#[cfg(not(target_arch = "wasm32"))]
use bevy::scene::ScenePlugin;
use bevy::sprite_render::Material2dPlugin;
#[cfg(not(target_arch = "wasm32"))]
use bevy::window::{PresentMode, Window, WindowPlugin, WindowResizeConstraints};
use bevy_svg::prelude::SvgPlugin;

use lightyear::avian2d::plugin::AvianReplicationMode;
use lightyear::avian2d::prelude::LightyearAvianPlugin;
use lightyear::frame_interpolation::FrameInterpolationPlugin;
use lightyear::input::native::prelude::NativeStateSequence;
use lightyear::input::plugin::InputPlugin as LightyearInputProtocolPlugin;
use lightyear::prelude::client::ClientPlugins;
use lightyear::prelude::client::{Client, Connected};
#[cfg(not(target_arch = "wasm32"))]
use lightyear::prelude::input::native::ClientInputPlugin as NativeClientInputPlugin;
use sidereal_core::SIM_TICK_HZ;
#[cfg(not(target_arch = "wasm32"))]
use sidereal_core::remote_inspect::RemoteInspectConfig;
use sidereal_game::{
    BallisticProjectileSpawnedEvent, CombatAuthorityEnabled, ShotFiredEvent, ShotHitEvent,
    ShotImpactResolvedEvent, apply_engine_thrust, bootstrap_weapon_cooldown_state,
    clamp_angular_velocity, process_character_movement_actions, process_flight_actions,
    process_weapon_fire_actions, stabilize_idle_motion, sync_mounted_hierarchy,
    tick_weapon_cooldowns, update_ballistic_projectiles,
};
use sidereal_net::register_lightyear_client_protocol;
use sidereal_runtime_sync::RuntimeEntityHierarchy;
#[cfg(not(target_arch = "wasm32"))]
use std::fs::OpenOptions;
#[cfg(not(target_arch = "wasm32"))]
use std::io::Write;
use std::time::Duration;
#[cfg(not(target_arch = "wasm32"))]
use tracing_subscriber::FmtSubscriber;

fn init_transport_resources(
    app: &mut App,
    headless_transport: bool,
    gateway_http_adapter: GatewayHttpAdapter,
    asset_cache_adapter: AssetCacheAdapter,
) {
    app.insert_resource(ClientSession::default());
    app.insert_resource(PendingDisconnectNotify::default());
    app.insert_resource(PendingDisconnectNotifySent::default());
    app.insert_resource(LogoutCleanupRequested::default());
    app.insert_resource(DisconnectRequest::default());
    app.insert_resource(PauseMenuState::default());
    app.insert_resource(ClientNetworkTick::default());
    app.insert_resource(ClientInputAckTracker::default());
    app.insert_resource(ClientInputSendState::default());
    app.insert_resource(ClientAuthSyncState::default());
    app.insert_resource(SessionReadyWatchdogConfig::from_env());
    app.insert_resource(SessionReadyWatchdogState::default());
    app.insert_resource(gateway_http_adapter);
    app.insert_resource(asset_cache_adapter);
    app.insert_resource(HeadlessTransportMode(headless_transport));
}

fn init_asset_runtime_resources(app: &mut App, asset_root: String) {
    app.insert_resource(AssetRootPath(asset_root));
    app.insert_resource(assets::LocalAssetManager::default());
    app.insert_resource(assets::AssetCatalogHotReloadState::default());
    app.insert_resource(assets::RuntimeAssetDependencyState::default());
    app.insert_resource(assets::RuntimeAssetDependencyDirtyState::default());
    app.insert_resource(assets::RuntimeAssetNetIndicatorState::default());
    app.insert_resource(assets::RuntimeAssetHttpFetchState::default());
    app.insert_resource(RuntimeAssetPerfCounters::default());
    app.insert_resource(OwnedAssetManifestCache::default());
}

fn init_control_and_prediction_resources(app: &mut App) {
    app.insert_resource(CombatAuthorityEnabled(false));
    app.insert_resource(MotionOwnershipReconcileState {
        dirty: true,
        ..default()
    });
    app.insert_resource(ClientControlRequestState::default());
    app.insert_resource(ClientViewModeState::default());
    app.insert_resource(LocalPlayerViewState::default());
    app.insert_resource(RuntimeEntityHierarchy::default());
    app.insert_resource(BootstrapWatchdogState::default());
    app.insert_resource(DeferredPredictedAdoptionState::default());
    app.insert_resource(PredictionBootstrapTuning::from_env());
    app.insert_resource(PredictionCorrectionTuning::from_env());
    app.insert_resource(NearbyCollisionProxyTuning::from_env());
    app.insert_resource(RemoteEntityRegistry::default());
}

fn init_debug_and_diagnostics_resources(app: &mut App, headless_transport: bool) {
    app.insert_resource(DebugOverlayState::default());
    app.insert_resource(DebugOverlaySnapshot::default());
    app.insert_resource(shaders::RuntimeShaderAssignments::default());
    app.insert_resource(shaders::RuntimeShaderAssignmentSyncState::default());
    if headless_transport {
        app.init_resource::<dialog_ui::DialogQueue>();
    }
}

fn init_tactical_resources(app: &mut App) {
    app.insert_resource(NameplateUiState { enabled: false });
    app.insert_resource(HudPerfCounters::default());
    app.insert_resource(CharacterSelectionState::default());
    app.insert_resource(FreeCameraState::default());
    app.insert_resource(OwnedEntitiesPanelState::default());
    app.insert_resource(TacticalFogCache::default());
    app.insert_resource(TacticalContactsCache::default());
    app.insert_resource(TacticalResnapshotRequestState::default());
    app.insert_resource(TacticalMapUiState::default());
    app.insert_resource(SessionReadyState::default());
}

fn init_scene_and_render_resources(app: &mut App) {
    app.insert_resource(FullscreenExternalWorldData::default());
    app.insert_resource(StarfieldMotionState::default());
    app.insert_resource(CameraMotionState::default());
}

pub(crate) fn configure_client_runtime(
    app: &mut App,
    asset_root: String,
    headless_transport: bool,
    gateway_http_adapter: GatewayHttpAdapter,
    asset_cache_adapter: AssetCacheAdapter,
) {
    app.add_plugins(
        PhysicsPlugins::default()
            .with_length_unit(1.0)
            .build()
            .disable::<PhysicsTransformPlugin>()
            .disable::<PhysicsInterpolationPlugin>(),
    );
    app.insert_resource(Gravity(Vec2::ZERO));
    crate::client_core::configure_shared_client_core(app);
    app.add_plugins(ClientPlugins {
        tick_duration: Duration::from_secs_f64(1.0 / f64::from(SIM_TICK_HZ)),
    });
    app.add_plugins(LightyearAvianPlugin {
        replication_mode: AvianReplicationMode::PositionButInterpolateTransform,
        update_syncs_manually: false,
        rollback_resources: false,
        rollback_islands: false,
    });
    app.add_plugins(FrameInterpolationPlugin::<Transform>::default());
    app.add_plugins(LightyearInputProtocolPlugin::<
        NativeStateSequence<sidereal_net::PlayerInput>,
    >::default());
    #[cfg(not(target_arch = "wasm32"))]
    app.add_plugins(NativeClientInputPlugin::<sidereal_net::PlayerInput>::default());
    register_lightyear_client_protocol(app);
    app.add_message::<ShotFiredEvent>();
    app.add_message::<ShotImpactResolvedEvent>();
    app.add_message::<ShotHitEvent>();
    app.add_message::<BallisticProjectileSpawnedEvent>();

    app.insert_resource(Time::<Fixed>::from_hz(f64::from(SIM_TICK_HZ)));
    init_transport_resources(
        app,
        headless_transport,
        gateway_http_adapter,
        asset_cache_adapter,
    );
    init_asset_runtime_resources(app, asset_root);
    init_control_and_prediction_resources(app);
    init_debug_and_diagnostics_resources(app, headless_transport);
    init_tactical_resources(app);
    init_scene_and_render_resources(app);
    app.add_systems(
        FixedUpdate,
        (
            motion::enforce_motion_ownership_for_world_entities,
            motion::sync_controlled_mass_from_total_mass,
            process_character_movement_actions,
            process_flight_actions,
            bootstrap_weapon_cooldown_state,
            tick_weapon_cooldowns,
            process_weapon_fire_actions,
            replication::mark_new_ballistic_projectiles_prespawned,
            update_ballistic_projectiles,
            apply_engine_thrust,
        )
            .chain()
            .before(avian2d::prelude::PhysicsSystems::StepSimulation),
    );
    app.add_systems(FixedPreUpdate, motion::mark_motion_ownership_dirty_signals);
    app.add_systems(
        FixedUpdate,
        (stabilize_idle_motion, clamp_angular_velocity)
            .chain()
            .after(avian2d::prelude::PhysicsSystems::StepSimulation),
    );
    app.add_systems(
        PostUpdate,
        sync_mounted_hierarchy.before(bevy::transform::TransformSystems::Propagate),
    );
    app.add_observer(log_client_transport_connected);
    app.add_observer(transport::ensure_raw_client_connected_after_linked);
    app.add_plugins(plugins::ClientBootstrapPlugin {
        headless: headless_transport,
    });
    app.add_plugins(plugins::ClientTransportPlugin {
        headless: headless_transport,
    });
    app.add_plugins(plugins::ClientReplicationPlugin {
        headless: headless_transport,
    });
    app.add_plugins(plugins::ClientPredictionPlugin {
        headless: headless_transport,
    });
    if !headless_transport {
        app.add_plugins(plugins::ClientVisualsPlugin);
        app.add_plugins(plugins::ClientLightingPlugin);
        app.add_plugins(plugins::ClientUiPlugin);
        app.add_plugins(plugins::ClientDiagnosticsPlugin);
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn build_headless_client_app(
    asset_root: String,
    gateway_http_adapter: GatewayHttpAdapter,
    asset_cache_adapter: AssetCacheAdapter,
) -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy::log::LogPlugin::default());
    app.add_plugins(bevy::transform::TransformPlugin);
    app.add_plugins(AssetPlugin::default());
    app.add_plugins(ScenePlugin);
    // Avian's collider cache reads mesh asset events even in headless mode.
    app.add_message::<bevy::asset::AssetEvent<Mesh>>();
    app.init_asset::<Mesh>();
    app.init_asset::<Image>();
    app.init_asset::<bevy::shader::Shader>();
    configure_client_runtime(
        &mut app,
        asset_root,
        true,
        gateway_http_adapter,
        asset_cache_adapter,
    );
    app
}

pub(crate) fn build_windowed_client_app(
    default_plugins: PluginGroupBuilder,
    asset_root: String,
    gateway_http_adapter: GatewayHttpAdapter,
    asset_cache_adapter: AssetCacheAdapter,
) -> App {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::BLACK));
    app.add_plugins(default_plugins);
    app.add_plugins(Material2dPlugin::<StarfieldMaterial>::default());
    app.add_plugins(Material2dPlugin::<SpaceBackgroundMaterial>::default());
    app.add_plugins(Material2dPlugin::<SpaceBackgroundNebulaMaterial>::default());
    app.add_plugins(Material2dPlugin::<StreamedSpriteShaderMaterial>::default());
    app.add_plugins(Material2dPlugin::<AsteroidSpriteShaderMaterial>::default());
    app.add_plugins(Material2dPlugin::<PlanetVisualMaterial>::default());
    app.add_plugins(Material2dPlugin::<RuntimeEffectMaterial>::default());
    app.add_plugins(Material2dPlugin::<TacticalMapOverlayMaterial>::default());
    app.add_plugins(SvgPlugin);
    app.add_plugins(FrameTimeDiagnosticsPlugin::default());
    audio::insert_embedded_menu_loop_audio(&mut app);
    if let Some(mut gizmo_config_store) = app.world_mut().get_resource_mut::<GizmoConfigStore>() {
        let (config, _) =
            gizmo_config_store.config_mut::<bevy::gizmos::config::DefaultGizmoConfigGroup>();
        config.render_layers = RenderLayers::layer(DEBUG_OVERLAY_RENDER_LAYER);
    }
    configure_client_runtime(
        &mut app,
        asset_root,
        false,
        gateway_http_adapter,
        asset_cache_adapter,
    );
    app
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn run() {
    match apply_process_cli() {
        Ok(CliAction::Run) => {}
        Ok(CliAction::Help(help)) => {
            println!("{help}");
            return;
        }
        Err(err) => {
            emit_startup_tracing_error(&err.to_string());
            std::process::exit(2);
        }
    }
    dev_console::install_panic_file_hook();

    let headless_transport = std::env::var("SIDEREAL_CLIENT_HEADLESS")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let remote_cfg = match RemoteInspectConfig::from_env("CLIENT", 15714) {
        Ok(cfg) => cfg,
        Err(err) => {
            emit_startup_tracing_error(&format!("invalid CLIENT BRP config: {err}"));
            std::process::exit(2);
        }
    };

    let asset_root = std::env::var("SIDEREAL_ASSET_ROOT").unwrap_or_else(|_| ".".to_string());
    let mut app = if headless_transport {
        build_headless_client_app(
            asset_root.clone(),
            platform_io::native_gateway_http_adapter(),
            platform_io::native_asset_cache_adapter(),
        )
    } else {
        build_windowed_client_app(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        present_mode: PresentMode::AutoVsync,
                        resizable: true,
                        resize_constraints: WindowResizeConstraints {
                            min_width: MIN_WINDOW_WIDTH,
                            min_height: MIN_WINDOW_HEIGHT,
                            ..default()
                        },
                        ..default()
                    }),
                    ..default()
                })
                .set(bevy::asset::AssetPlugin {
                    file_path: asset_root.clone(),
                    ..Default::default()
                })
                .set(LogPlugin {
                    custom_layer: dev_console::build_log_capture_layer,
                    fmt_layer: dev_console::build_file_fmt_layer,
                    ..default()
                })
                .set(RenderPlugin {
                    render_creation: RenderCreation::Automatic(platform::configured_wgpu_settings()),
                    ..Default::default()
                }),
            asset_root.clone(),
            platform_io::native_gateway_http_adapter(),
            platform_io::native_asset_cache_adapter(),
        )
    };

    configure_remote(&mut app, &remote_cfg);
    app.run();
}

#[cfg(not(target_arch = "wasm32"))]
fn log_startup_error_line(message: &str) {
    let path = dev_console::log_file_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{message}");
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn emit_startup_tracing_error(message: &str) {
    log_startup_error_line(message);
    let subscriber = FmtSubscriber::builder()
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .without_time()
        .finish();
    tracing::subscriber::with_default(subscriber, || {
        tracing::error!("{message}");
    });
}

fn log_client_transport_connected(
    trigger: On<Add, Connected>,
    clients: Query<'_, '_, (), With<Client>>,
) {
    if clients.get(trigger.entity).is_ok() {
        info!("client lightyear transport connected");
    }
}
