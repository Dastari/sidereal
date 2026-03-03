mod auth_ui;
mod dialog_ui;

mod app_state;
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
mod logout;
mod motion;
mod platform;
mod plugins;
mod remote;
mod replication;
mod resources;
mod scene;
mod scene_world;
mod shaders;
mod transforms;
mod transport;
mod ui;
mod visuals;

pub(crate) use app_state::*;
pub(crate) use auth_net::submit_auth_request;
pub(crate) use backdrop::{
    SpaceBackgroundMaterial, StarfieldMaterial, StreamedSpriteShaderMaterial, ThrusterPlumeMaterial,
};
pub(crate) use platform::*;
pub(crate) use remote::*;
pub(crate) use resources::*;

use avian2d::prelude::*;
use bevy::asset::{AssetApp, AssetPlugin};
use bevy::diagnostic::FrameTimeDiagnosticsPlugin;
use bevy::prelude::*;
use bevy::render::RenderPlugin;
use bevy::render::settings::RenderCreation;
use bevy::scene::ScenePlugin;
use bevy::sprite_render::Material2dPlugin;
use bevy::window::{PresentMode, Window, WindowPlugin, WindowResizeConstraints};

use lightyear::avian2d::plugin::AvianReplicationMode;
use lightyear::avian2d::prelude::LightyearAvianPlugin;
use lightyear::prelude::client::ClientPlugins;
use lightyear::prelude::client::{Client, Connected};
use sidereal_core::remote_inspect::RemoteInspectConfig;
use sidereal_game::{
    SiderealGameCorePlugin, apply_engine_thrust, process_character_movement_actions,
    process_flight_actions, sync_mounted_hierarchy,
};
use sidereal_net::register_lightyear_protocol;
use sidereal_runtime_sync::RuntimeEntityHierarchy;
use std::time::Duration;

pub(crate) fn run() {
    let headless_transport = std::env::var("SIDEREAL_CLIENT_HEADLESS")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let _multi_instance_guard = (!headless_transport)
        .then(platform::acquire_multi_instance_guard)
        .flatten();
    let _is_secondary_instance = !headless_transport && _multi_instance_guard.is_none();
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
                .set(RenderPlugin {
                    render_creation: RenderCreation::Automatic(platform::configured_wgpu_settings()),
                    ..Default::default()
                }),
        );
        shaders::ensure_shader_placeholders(&asset_root);
        app.add_plugins(Material2dPlugin::<StarfieldMaterial>::default());
        app.add_plugins(Material2dPlugin::<SpaceBackgroundMaterial>::default());
        app.add_plugins(Material2dPlugin::<StreamedSpriteShaderMaterial>::default());
        app.add_plugins(Material2dPlugin::<ThrusterPlumeMaterial>::default());
        app.add_plugins(FrameTimeDiagnosticsPlugin::default());
        audio::insert_embedded_menu_loop_audio(&mut app);
        // FPS cap: SIDEREAL_CLIENT_MAX_FPS (default 60). Set to 0 to disable (uncapped).
        if let Some(frame_cap) = FrameRateCap::from_env(60) {
            app.insert_resource(frame_cap);
            app.add_systems(Last, platform::enforce_frame_rate_cap_system);
        }
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
    app.add_plugins(SiderealGameCorePlugin);
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
    app.insert_resource(MotionOwnershipAuditEnabled::from_env());
    app.insert_resource(MotionOwnershipAuditState::default());
    app.insert_resource(ClientSession::default());
    app.insert_resource(PendingDisconnectNotify::default());
    app.insert_resource(LogoutCleanupRequested::default());
    app.insert_resource(ClientNetworkTick::default());
    app.insert_resource(ClientInputAckTracker::default());
    app.insert_resource(ClientInputLogState::default());
    app.insert_resource(ClientInputSendState::default());
    app.insert_resource(ClientAuthSyncState::default());
    app.insert_resource(ClientControlRequestState::default());
    app.insert_resource(ClientControlDebugState::default());
    app.insert_resource(SessionReadyState::default());
    app.insert_resource(assets::LocalAssetManager::default());
    app.insert_resource(assets::RuntimeAssetStreamIndicatorState::default());
    app.insert_resource(assets::CriticalAssetRequestState::default());
    let debug_blue_overlay = std::env::var("SIDEREAL_DEBUG_BLUE_FULLSCREEN")
        .is_ok_and(|v| v == "1" || v.eq_ignore_ascii_case("true"));
    app.insert_resource(DebugBlueOverlayEnabled(debug_blue_overlay));
    app.insert_resource(DebugOverlayEnabled { enabled: false });
    app.insert_resource(LocalPlayerViewState::default());
    app.insert_resource(CharacterSelectionState::default());
    app.insert_resource(FreeCameraState::default());
    app.insert_resource(OwnedEntitiesPanelState::default());
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
    app.add_systems(
        FixedUpdate,
        (
            motion::enforce_motion_ownership_for_world_entities,
            motion::audit_motion_ownership_system
                .after(motion::enforce_motion_ownership_for_world_entities),
            process_character_movement_actions,
            motion::sync_controlled_mass_from_total_mass,
            process_flight_actions,
            apply_engine_thrust,
        )
            .chain()
            .before(avian2d::prelude::PhysicsSystems::StepSimulation),
    );
    app.add_systems(
        FixedUpdate,
        (
            motion::reconcile_controlled_prediction_with_confirmed,
            motion::stabilize_controlled_idle_motion,
            motion::clamp_controlled_angular_velocity,
        )
            .chain()
            .after(avian2d::prelude::PhysicsSystems::StepSimulation),
    );
    if headless_transport {
        app.init_resource::<dialog_ui::DialogQueue>();
    }
    app.add_systems(
        PostUpdate,
        sync_mounted_hierarchy.before(bevy::transform::TransformSystems::Propagate),
    );
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
        app.add_plugins(plugins::ClientVisualsPlugin);
        app.add_plugins(plugins::ClientUiPlugin);
        app.add_plugins(plugins::ClientDiagnosticsPlugin);
    }
    app.run();
}

fn log_native_client_connected(
    trigger: On<Add, Connected>,
    clients: Query<'_, '_, (), With<Client>>,
) {
    if clients.get(trigger.entity).is_ok() {
        info!("native client lightyear transport connected");
    }
}
