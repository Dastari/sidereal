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
mod control;
mod debug_overlay;
mod input;
mod lighting;
mod logout;
mod motion;
mod owner_manifest;
mod pause_menu;
mod platform;
mod plugins;
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

pub(crate) use app_state::*;
pub(crate) use auth_net::submit_auth_request;
pub(crate) use backdrop::{
    AsteroidSpriteShaderMaterial, PlanetVisualMaterial, RuntimeEffectMaterial,
    SpaceBackgroundMaterial, SpaceBackgroundNebulaMaterial, StarfieldMaterial,
    StreamedSpriteShaderMaterial, TacticalMapOverlayMaterial,
};
pub(crate) use platform::*;
pub(crate) use remote::*;
pub(crate) use resources::*;

use avian2d::prelude::*;
use bevy::asset::{AssetApp, AssetPlugin};
use bevy::diagnostic::FrameTimeDiagnosticsPlugin;
use bevy::log::LogPlugin;
use bevy::prelude::*;
use bevy::render::RenderPlugin;
use bevy::render::settings::RenderCreation;
use bevy::scene::ScenePlugin;
use bevy::sprite_render::Material2dPlugin;
use bevy::window::{PresentMode, Window, WindowPlugin, WindowResizeConstraints};
use bevy_svg::prelude::SvgPlugin;

use lightyear::avian2d::plugin::AvianReplicationMode;
use lightyear::avian2d::prelude::LightyearAvianPlugin;
use lightyear::prelude::client::ClientPlugins;
use lightyear::prelude::client::{Client, Connected};
use sidereal_core::remote_inspect::RemoteInspectConfig;
use sidereal_game::{
    apply_engine_thrust, clamp_angular_velocity, process_character_movement_actions,
    process_flight_actions, stabilize_idle_motion, sync_mounted_hierarchy,
};
use sidereal_net::register_lightyear_client_protocol;
use sidereal_runtime_sync::RuntimeEntityHierarchy;
use std::fs::OpenOptions;
use std::io::Write;
use std::time::Duration;

pub(crate) fn run() {
    dev_console::install_panic_file_hook();
    let env_flag = |name: &str| {
        std::env::var(name)
            .ok()
            .map(|v| v.trim().to_string())
            .is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"))
    };
    eprintln!(
        "client startup env flags: disable_runtime_asset_fetch={} disable_repl_adoption={} disable_hierarchy_rebuild={} disable_world_visuals={} disable_motion_ownership={} shader_materials_enabled={} streamed_shader_overrides={}",
        env_flag("SIDEREAL_CLIENT_DISABLE_RUNTIME_ASSET_FETCH"),
        env_flag("SIDEREAL_CLIENT_DISABLE_REPLICATION_ADOPTION"),
        env_flag("SIDEREAL_CLIENT_DISABLE_HIERARCHY_REBUILD"),
        env_flag("SIDEREAL_CLIENT_DISABLE_WORLD_VISUALS"),
        env_flag("SIDEREAL_CLIENT_DISABLE_MOTION_OWNERSHIP"),
        shaders::shader_materials_enabled(),
        shaders::streamed_shader_overrides_enabled(),
    );

    let headless_transport = std::env::var("SIDEREAL_CLIENT_HEADLESS")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let remote_cfg = match RemoteInspectConfig::from_env("CLIENT", 15714) {
        Ok(cfg) => cfg,
        Err(err) => {
            log_startup_error_line(&format!("invalid CLIENT BRP config: {err}"));
            eprintln!("invalid CLIENT BRP config: {err}");
            std::process::exit(2);
        }
    };

    let asset_root = std::env::var("SIDEREAL_ASSET_ROOT").unwrap_or_else(|_| ".".to_string());
    let mut app = App::new();
    if headless_transport {
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
    } else {
        app.insert_resource(ClearColor(Color::BLACK));
        app.add_plugins(
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
        );
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
    }

    app.add_plugins(
        PhysicsPlugins::default()
            .with_length_unit(1.0)
            .build()
            .disable::<PhysicsTransformPlugin>()
            .disable::<PhysicsInterpolationPlugin>(),
    );
    app.insert_resource(Gravity(Vec2::ZERO));
    // Client prediction needs shared flight/mass gameplay systems, but not player observer
    // anchoring/movement writers from full server plugin.
    crate::client_core::configure_shared_client_core(&mut app);
    app.add_plugins(ClientPlugins {
        tick_duration: Duration::from_secs_f64(1.0 / 60.0),
    });
    app.add_plugins(LightyearAvianPlugin {
        replication_mode: AvianReplicationMode::PositionButInterpolateTransform,
        update_syncs_manually: false,
        rollback_resources: false,
        rollback_islands: false,
    });
    register_lightyear_client_protocol(&mut app);
    configure_remote(&mut app, &remote_cfg);
    // Lightyear/Bevy plugins can initialize Fixed time; reset project-authoritative 60 Hz after plugin wiring.
    app.insert_resource(Time::<Fixed>::from_hz(60.0));
    app.insert_resource(AssetRootPath(asset_root));
    app.insert_resource(LocalSimulationDebugMode::from_env());
    app.insert_resource(MotionOwnershipAuditEnabled::from_env());
    app.insert_resource(MotionOwnershipAuditState::default());
    app.insert_resource(MotionOwnershipReconcileState {
        dirty: true,
        ..default()
    });
    app.insert_resource(ClientSession::default());
    app.insert_resource(PendingDisconnectNotify::default());
    app.insert_resource(PendingDisconnectNotifySent::default());
    app.insert_resource(LogoutCleanupRequested::default());
    app.insert_resource(DisconnectRequest::default());
    app.insert_resource(PauseMenuState::default());
    app.insert_resource(ClientNetworkTick::default());
    app.insert_resource(ClientInputAckTracker::default());
    app.insert_resource(ClientInputLogState::default());
    app.insert_resource(ClientInputSendState::default());
    app.insert_resource(ClientAuthSyncState::default());
    app.insert_resource(SessionReadyWatchdogConfig::from_env());
    app.insert_resource(SessionReadyWatchdogState::default());
    app.insert_resource(ClientControlRequestState::default());
    app.insert_resource(ClientControlDebugState::default());
    app.insert_resource(ClientViewModeState::default());
    app.insert_resource(SessionReadyState::default());
    app.insert_resource(assets::LocalAssetManager::default());
    app.insert_resource(assets::RuntimeAssetNetIndicatorState::default());
    app.insert_resource(assets::RuntimeAssetHttpFetchState::default());
    let debug_blue_overlay = std::env::var("SIDEREAL_DEBUG_BLUE_FULLSCREEN")
        .is_ok_and(|v| v == "1" || v.eq_ignore_ascii_case("true"));
    app.insert_resource(DebugBlueOverlayEnabled(debug_blue_overlay));
    app.insert_resource(DebugOverlayEnabled { enabled: false });
    app.insert_resource(NameplateUiState { enabled: false });
    app.insert_resource(LocalPlayerViewState::default());
    app.insert_resource(CharacterSelectionState::default());
    app.insert_resource(FreeCameraState::default());
    app.insert_resource(OwnedEntitiesPanelState::default());
    app.insert_resource(OwnedAssetManifestCache::default());
    app.insert_resource(TacticalFogCache::default());
    app.insert_resource(TacticalContactsCache::default());
    app.insert_resource(TacticalResnapshotRequestState::default());
    app.insert_resource(TacticalMapUiState::default());
    app.insert_resource(RuntimeEntityHierarchy::default());
    app.insert_resource(FullscreenExternalWorldData::default());
    app.insert_resource(StarfieldMotionState::default());
    app.insert_resource(CameraMotionState::default());
    app.insert_resource(BootstrapWatchdogState::default());
    app.insert_resource(DeferredPredictedAdoptionState::default());
    app.insert_resource(PredictionBootstrapTuning::from_env());
    app.insert_resource(PredictionCorrectionTuning::from_env());
    app.insert_resource(NearbyCollisionProxyTuning::from_env());
    app.insert_resource(RemoteEntityRegistry::default());
    app.insert_resource(HeadlessTransportMode(headless_transport));
    let disable_motion_ownership = env_flag("SIDEREAL_CLIENT_DISABLE_MOTION_OWNERSHIP");
    if disable_motion_ownership {
        eprintln!(
            "WARN sidereal_client::native: client motion-ownership systems disabled via SIDEREAL_CLIENT_DISABLE_MOTION_OWNERSHIP"
        );
        app.add_systems(
            FixedUpdate,
            (
                motion::sync_controlled_mass_from_total_mass,
                process_character_movement_actions,
                process_flight_actions,
                apply_engine_thrust,
            )
                .chain()
                .before(avian2d::prelude::PhysicsSystems::StepSimulation),
        );
    } else {
        app.add_systems(
            FixedUpdate,
            (
                motion::enforce_motion_ownership_for_world_entities,
                motion::audit_motion_ownership_system
                    .after(motion::enforce_motion_ownership_for_world_entities)
                    .run_if(bevy::ecs::schedule::common_conditions::not(
                        lightyear::prelude::is_in_rollback,
                    )),
                motion::sync_controlled_mass_from_total_mass,
                process_character_movement_actions,
                process_flight_actions,
                apply_engine_thrust,
            )
                .chain()
                .before(avian2d::prelude::PhysicsSystems::StepSimulation),
        );
        app.add_systems(FixedPreUpdate, motion::mark_motion_ownership_dirty_signals);
    }
    app.add_systems(
        FixedUpdate,
        (stabilize_idle_motion, clamp_angular_velocity)
            .chain()
            .after(avian2d::prelude::PhysicsSystems::StepSimulation),
    );
    if headless_transport {
        app.init_resource::<dialog_ui::DialogQueue>();
    }
    let disable_hierarchy_rebuild = env_flag("SIDEREAL_CLIENT_DISABLE_HIERARCHY_REBUILD");
    if disable_hierarchy_rebuild {
        eprintln!(
            "WARN sidereal_client::native: client hierarchy rebuild disabled via SIDEREAL_CLIENT_DISABLE_HIERARCHY_REBUILD"
        );
    } else {
        app.add_systems(
            PostUpdate,
            sync_mounted_hierarchy.before(bevy::transform::TransformSystems::Propagate),
        );
    }
    app.add_observer(log_native_client_connected);
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
        let disable_world_visuals = env_flag("SIDEREAL_CLIENT_DISABLE_WORLD_VISUALS");
        if disable_world_visuals {
            eprintln!(
                "WARN sidereal_client::native: client world visuals disabled via SIDEREAL_CLIENT_DISABLE_WORLD_VISUALS"
            );
        } else {
            app.add_plugins(plugins::ClientVisualsPlugin);
        }
        app.add_plugins(plugins::ClientLightingPlugin);
        app.add_plugins(plugins::ClientUiPlugin);
        app.add_plugins(plugins::ClientDiagnosticsPlugin);
    }
    app.run();
}

fn log_startup_error_line(message: &str) {
    let path = std::env::var("SIDEREAL_CLIENT_LOG_FILE")
        .unwrap_or_else(|_| "logs/sidereal-client.log".to_string());
    let path = std::path::PathBuf::from(path);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{message}");
    }
}

fn log_native_client_connected(
    trigger: On<Add, Connected>,
    clients: Query<'_, '_, (), With<Client>>,
) {
    if clients.get(trigger.entity).is_ok() {
        info!("native client lightyear transport connected");
    }
}
