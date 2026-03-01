mod auth_ui;
mod dialog_ui;

mod app_state;
mod assets;
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
    SpaceBackgroundMaterial, StarfieldMaterial, StreamedSpriteShaderMaterial,
};
pub(crate) use platform::*;
pub(crate) use remote::*;
pub(crate) use resources::*;

use avian2d::prelude::*;
use bevy::asset::{AssetApp, AssetPlugin};
use bevy::diagnostic::FrameTimeDiagnosticsPlugin;
use bevy::log::info;
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
    SiderealGameCorePlugin, apply_engine_thrust, clamp_angular_velocity,
    process_character_movement_actions, process_flight_actions, recompute_total_mass,
    stabilize_idle_motion, sync_mounted_hierarchy,
    validate_action_capabilities,
};
use sidereal_net::register_lightyear_protocol;
use sidereal_runtime_sync::RuntimeEntityHierarchy;
use std::time::Duration;

pub(crate) use camera::{
    audit_active_world_cameras_system, gate_gameplay_camera_system,
    sync_ui_overlay_camera_to_gameplay_camera_system, update_camera_motion_state,
    update_topdown_camera_system,
};
pub(crate) use debug_overlay::{
    draw_debug_overlay_system, log_prediction_runtime_state, toggle_debug_overlay_system,
};
pub(crate) use motion::{
    apply_predicted_input_to_action_queue, audit_motion_ownership_system,
    enforce_controlled_planar_motion, enforce_motion_ownership_for_world_entities,
    reconcile_controlled_prediction_with_confirmed,
};
pub(crate) use replication::{
    adopt_native_lightyear_replicated_entities, configure_prediction_manager_tuning,
    ensure_replicated_entity_spatial_components, sync_controlled_entity_tags_system,
    sync_local_player_view_state_system, transition_world_loading_to_in_world,
};
pub(crate) use transforms::{
    apply_interpolated_visual_smoothing_system, lock_camera_to_player_entity_end_of_frame,
    lock_player_entity_to_controlled_entity_end_of_frame,
    refresh_interpolated_visual_targets_system, sync_world_entity_transforms_from_physics,
};

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
        app.add_plugins(FrameTimeDiagnosticsPlugin::default());
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
            enforce_motion_ownership_for_world_entities,
            audit_motion_ownership_system.after(enforce_motion_ownership_for_world_entities),
            validate_action_capabilities,
            process_character_movement_actions,
            process_flight_actions,
            recompute_total_mass,
            apply_engine_thrust,
        )
            .chain()
            .before(avian2d::prelude::PhysicsSystems::StepSimulation),
    );
    app.add_systems(
        FixedUpdate,
        (
            reconcile_controlled_prediction_with_confirmed,
            stabilize_idle_motion,
            clamp_angular_velocity,
        )
            .chain()
            .after(avian2d::prelude::PhysicsSystems::StepSimulation),
    );
    if headless_transport {
        app.init_resource::<dialog_ui::DialogQueue>();
    }
    app.add_systems(PreUpdate, ensure_replicated_entity_spatial_components);
    app.add_systems(
        PostUpdate,
        sync_mounted_hierarchy.before(bevy::transform::TransformSystems::Propagate),
    );
    app.add_observer(log_native_client_connected);
    if headless_transport {
        app.add_systems(Startup, transport::start_lightyear_client_transport);
    }
    if !headless_transport {
        app.add_systems(Startup, scene::spawn_ui_overlay_camera);
    }

    if headless_transport {
        app.add_systems(Startup, auth_net::configure_headless_session_from_env);
        app.add_systems(
            FixedPreUpdate,
            (
                input::enforce_single_input_marker_owner
                    .before(input::send_lightyear_input_messages),
                input::send_lightyear_input_messages,
                bevy::ecs::schedule::ApplyDeferred,
            )
                .chain()
                .in_set(lightyear::prelude::client::input::InputSystems::WriteClientInputs),
        );
        app.add_systems(
            Update,
            (
                auth_net::apply_headless_account_switch_system,
                configure_prediction_manager_tuning,
                transport::ensure_client_transport_channels,
                auth_net::send_lightyear_auth_messages,
                assets::receive_lightyear_asset_stream_messages,
                assets::ensure_critical_assets_available_system
                    .after(assets::receive_lightyear_asset_stream_messages),
                adopt_native_lightyear_replicated_entities,
                sync_world_entity_transforms_from_physics
                    .after(adopt_native_lightyear_replicated_entities),
                sync_local_player_view_state_system
                    .after(adopt_native_lightyear_replicated_entities),
                sync_controlled_entity_tags_system.after(sync_local_player_view_state_system),
                control::send_lightyear_control_requests.after(sync_controlled_entity_tags_system),
                control::receive_lightyear_control_results
                    .after(control::send_lightyear_control_requests),
                control::log_client_control_state_changes
                    .after(control::receive_lightyear_control_results),
                log_prediction_runtime_state,
            ),
        );
        app.add_systems(Startup, || {
            info!("sidereal-client headless transport mode");
        });
    } else {
        scene::insert_embedded_fonts(&mut app);
        app.init_state::<ClientAppState>();
        app.add_systems(
            OnEnter(ClientAppState::CharacterSelect),
            scene::setup_character_select_screen,
        );
        app.add_systems(
            OnEnter(ClientAppState::WorldLoading),
            (
                transport::ensure_lightyear_client_system,
                bootstrap::reset_bootstrap_watchdog_on_enter_in_world,
            )
                .chain(),
        );
        auth_ui::register_auth_ui(&mut app);
        dialog_ui::register_dialog_ui(&mut app);
        app.add_systems(
            OnEnter(ClientAppState::InWorld),
            (
                transport::ensure_lightyear_client_system,
                scene_world::spawn_world_scene,
                bootstrap::reset_bootstrap_watchdog_on_enter_in_world,
            )
                .chain(),
        );
        app.add_systems(
            Update,
            (
                scene::handle_character_select_buttons,
                transport::ensure_client_transport_channels,
                configure_prediction_manager_tuning,
                auth_net::send_lightyear_auth_messages,
                auth_net::receive_lightyear_session_ready_messages,
                auth_net::receive_lightyear_session_denied_messages,
                assets::receive_lightyear_asset_stream_messages,
                assets::ensure_critical_assets_available_system
                    .after(assets::receive_lightyear_asset_stream_messages),
                adopt_native_lightyear_replicated_entities,
                sync_world_entity_transforms_from_physics
                    .after(adopt_native_lightyear_replicated_entities),
                transition_world_loading_to_in_world
                    .after(adopt_native_lightyear_replicated_entities),
                sync_local_player_view_state_system
                    .after(adopt_native_lightyear_replicated_entities),
                sync_controlled_entity_tags_system.after(sync_local_player_view_state_system),
                control::send_lightyear_control_requests.after(sync_controlled_entity_tags_system),
                control::receive_lightyear_control_results
                    .after(control::send_lightyear_control_requests),
                control::log_client_control_state_changes
                    .after(control::receive_lightyear_control_results),
                log_prediction_runtime_state,
            ),
        );
        app.add_systems(
            Update,
            (
                visuals::ensure_fullscreen_layer_fallback_system
                    .after(adopt_native_lightyear_replicated_entities),
                visuals::suppress_duplicate_predicted_interpolated_visuals_system
                    .after(adopt_native_lightyear_replicated_entities),
                visuals::cleanup_streamed_visual_children_system
                    .after(visuals::suppress_duplicate_predicted_interpolated_visuals_system),
                visuals::attach_streamed_visual_assets_system
                    .after(assets::receive_lightyear_asset_stream_messages),
                visuals::sync_fullscreen_layer_renderables_system
                    .after(adopt_native_lightyear_replicated_entities),
                visuals::sync_backdrop_fullscreen_system
                    .after(visuals::sync_fullscreen_layer_renderables_system),
                gate_gameplay_camera_system,
                ui::update_owned_entities_panel_system,
                ui::handle_owned_entities_panel_buttons,
                ui::update_loading_overlay_system,
                ui::update_runtime_stream_icon_system,
                bootstrap::watch_in_world_bootstrap_failures,
                update_topdown_camera_system
                    .after(lock_player_entity_to_controlled_entity_end_of_frame),
                sync_ui_overlay_camera_to_gameplay_camera_system
                    .after(update_topdown_camera_system),
                update_camera_motion_state.after(update_topdown_camera_system),
                ui::propagate_ui_overlay_layer_system,
                ui::update_hud_system,
                ui::sync_ship_nameplates_system,
                ui::update_segmented_bars_system.after(ui::update_hud_system),
                toggle_debug_overlay_system,
            )
                .run_if(in_state(ClientAppState::InWorld)),
        );
        app.add_systems(
            Update,
            (
                refresh_interpolated_visual_targets_system
                    .after(sync_world_entity_transforms_from_physics),
                apply_interpolated_visual_smoothing_system
                    .after(refresh_interpolated_visual_targets_system),
                lock_player_entity_to_controlled_entity_end_of_frame
                    .after(apply_interpolated_visual_smoothing_system),
            )
                .run_if(in_state(ClientAppState::InWorld)),
        );
        app.add_systems(
            Update,
            audit_active_world_cameras_system.run_if(in_state(ClientAppState::InWorld)),
        );
        app.add_systems(
            Last,
            (
                lock_player_entity_to_controlled_entity_end_of_frame,
                lock_camera_to_player_entity_end_of_frame
                    .after(lock_player_entity_to_controlled_entity_end_of_frame),
                backdrop::compute_fullscreen_external_world_system
                    .after(lock_camera_to_player_entity_end_of_frame),
                backdrop::update_starfield_material_system
                    .after(backdrop::compute_fullscreen_external_world_system),
                backdrop::update_space_background_material_system
                    .after(backdrop::update_starfield_material_system),
                ui::update_ship_nameplate_positions_system
                    .after(backdrop::update_space_background_material_system),
                ui::update_segmented_bars_system
                    .after(ui::update_ship_nameplate_positions_system),
                draw_debug_overlay_system.after(ui::update_segmented_bars_system),
            )
                .chain()
                .run_if(in_state(ClientAppState::InWorld)),
        );
        app.add_systems(
            FixedPreUpdate,
            (
                input::enforce_single_input_marker_owner
                    .before(input::send_lightyear_input_messages),
                input::send_lightyear_input_messages,
                bevy::ecs::schedule::ApplyDeferred,
            )
                .chain()
                .in_set(lightyear::prelude::client::input::InputSystems::WriteClientInputs)
                .run_if(in_state(ClientAppState::InWorld)),
        );
        {
            #[cfg(not(target_arch = "wasm32"))]
            let logout_chain = (
                logout::request_logout_system.run_if(in_state(ClientAppState::InWorld)),
                logout::request_logout_system.run_if(in_state(ClientAppState::WorldLoading)),
                logout::request_logout_system.run_if(in_state(ClientAppState::CharacterSelect)),
                logout::request_logout_on_window_close_system
                    .run_if(in_state(ClientAppState::InWorld)),
                logout::request_logout_on_window_close_system
                    .run_if(in_state(ClientAppState::WorldLoading)),
                logout::request_logout_on_window_close_system
                    .run_if(in_state(ClientAppState::CharacterSelect)),
                logout::send_disconnect_notify_and_trigger_system,
                logout::logout_cleanup_system.run_if(resource_equals(LogoutCleanupRequested(true))),
            );
            #[cfg(target_arch = "wasm32")]
            let logout_chain = (
                logout::request_logout_system.run_if(in_state(ClientAppState::InWorld)),
                logout::request_logout_system.run_if(in_state(ClientAppState::WorldLoading)),
                logout::request_logout_system.run_if(in_state(ClientAppState::CharacterSelect)),
                logout::send_disconnect_notify_and_trigger_system,
                logout::logout_cleanup_system.run_if(resource_equals(LogoutCleanupRequested(true))),
            );
            app.add_systems(PreUpdate, logout_chain.chain());
        }
        // Run predicted input before movement so the same tick's input is applied (player entity and ship).
        app.add_systems(
            FixedUpdate,
            (
                apply_predicted_input_to_action_queue,
                enforce_controlled_planar_motion,
            )
                .chain()
                .before(process_character_movement_actions)
                .run_if(in_state(ClientAppState::InWorld)),
        );
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
