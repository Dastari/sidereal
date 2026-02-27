#[path = "../auth_ui.rs"]
mod auth_ui;
#[path = "../dialog_ui.rs"]
mod dialog_ui;

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
mod shaders;
mod state;
mod transforms;
mod transport;

pub(crate) use auth_net::submit_auth_request;
pub(crate) use backdrop::{
    SpaceBackgroundMaterial, StarfieldMaterial, StreamedSpriteShaderMaterial,
};
pub(crate) use components::*;
pub(crate) use platform::*;
pub(crate) use remote::*;
pub(crate) use resources::*;
pub(crate) use state::*;

use avian2d::prelude::*;
use bevy::asset::{AssetApp, AssetPlugin};
use bevy::camera::visibility::RenderLayers;
use bevy::log::{info, warn};
use bevy::prelude::*;
use bevy::render::RenderPlugin;
use bevy::render::settings::RenderCreation;
use bevy::scene::ScenePlugin;
use bevy::sprite_render::{ColorMaterial, Material2dPlugin, MeshMaterial2d};
use bevy::state::state_scoped::DespawnOnExit;
use bevy::window::{PresentMode, Window, WindowPlugin, WindowResizeConstraints};

use lightyear::avian2d::plugin::AvianReplicationMode;
use lightyear::avian2d::prelude::LightyearAvianPlugin;
use lightyear::prelude::client::ClientPlugins;
use lightyear::prelude::client::{Client, Connected};
use lightyear::prelude::{MessageReceiver, MessageSender};
use sidereal_asset_runtime::{
    AssetCacheIndexRecord, cache_index_path, load_cache_index, save_cache_index, sha256_hex,
};
use sidereal_core::remote_inspect::RemoteInspectConfig;
use sidereal_game::{
    EntityGuid, FullscreenLayer, Hardpoint, HealthPool, MountedOn, OwnerId, PlayerTag,
    SiderealGameCorePlugin, SizeM, apply_engine_thrust, clamp_angular_velocity,
    default_corvette_asset_id, default_space_background_shader_asset_id,
    default_starfield_shader_asset_id, process_flight_actions, recompute_total_mass,
    stabilize_idle_motion, validate_action_capabilities,
};
use sidereal_net::{
    AssetAckMessage, AssetRequestMessage, AssetStreamChunkMessage, AssetStreamManifestMessage,
    ControlChannel, RequestedAsset, register_lightyear_protocol,
};
use sidereal_runtime_sync::RuntimeEntityHierarchy;
use std::collections::HashMap;
use std::time::Duration;

pub(crate) use camera::{
    audit_active_world_cameras_system, gate_gameplay_camera_system,
    sync_ui_overlay_camera_to_gameplay_camera_system, update_camera_motion_state,
    update_topdown_camera_system,
};
pub(crate) use debug_overlay::{draw_debug_overlay_system, toggle_debug_overlay_system};
pub(crate) use motion::{
    apply_predicted_input_to_action_queue, audit_motion_ownership_system,
    enforce_controlled_planar_motion, enforce_motion_ownership_for_world_entities,
    reconcile_controlled_prediction_with_confirmed,
};
pub(crate) use replication::{
    adopt_native_lightyear_replicated_entities, configure_prediction_manager_tuning,
    log_prediction_runtime_state, sync_controlled_entity_tags_system,
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
    app.insert_resource(LocalAssetManager::default());
    app.insert_resource(RuntimeAssetStreamIndicatorState::default());
    app.insert_resource(CriticalAssetRequestState::default());
    let debug_blue_overlay = std::env::var("SIDEREAL_DEBUG_BLUE_FULLSCREEN")
        .is_ok_and(|v| v == "1" || v.eq_ignore_ascii_case("true"));
    app.insert_resource(DebugBlueOverlayEnabled(debug_blue_overlay));
    app.insert_resource(DebugOverlayEnabled { enabled: false });
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
    app.insert_resource(NearbyCollisionProxyTuning::from_env());
    app.insert_resource(RemoteEntityRegistry::default());
    app.insert_resource(HeadlessTransportMode(headless_transport));
    app.add_systems(
        FixedUpdate,
        (
            enforce_motion_ownership_for_world_entities,
            audit_motion_ownership_system.after(enforce_motion_ownership_for_world_entities),
            validate_action_capabilities,
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
    app.add_observer(log_native_client_connected);
    if headless_transport {
        app.add_systems(Startup, transport::start_lightyear_client_transport);
    }
    if !headless_transport {
        app.add_systems(Startup, spawn_ui_overlay_camera);
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
                receive_lightyear_asset_stream_messages,
                ensure_critical_assets_available_system
                    .after(receive_lightyear_asset_stream_messages),
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
        insert_embedded_fonts(&mut app);
        app.init_state::<ClientAppState>();
        app.add_systems(
            OnEnter(ClientAppState::CharacterSelect),
            setup_character_select_screen,
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
                spawn_world_scene,
                bootstrap::reset_bootstrap_watchdog_on_enter_in_world,
            )
                .chain(),
        );
        app.add_systems(
            Update,
            (
                handle_character_select_buttons,
                transport::ensure_client_transport_channels,
                configure_prediction_manager_tuning,
                auth_net::send_lightyear_auth_messages,
                auth_net::receive_lightyear_session_ready_messages,
                receive_lightyear_asset_stream_messages,
                ensure_critical_assets_available_system
                    .after(receive_lightyear_asset_stream_messages),
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
                ensure_fullscreen_layer_fallback_system
                    .after(adopt_native_lightyear_replicated_entities),
                suppress_duplicate_predicted_interpolated_visuals_system
                    .after(adopt_native_lightyear_replicated_entities),
                cleanup_streamed_visual_children_system
                    .after(suppress_duplicate_predicted_interpolated_visuals_system),
                attach_streamed_visual_assets_system.after(receive_lightyear_asset_stream_messages),
                sync_fullscreen_layer_renderables_system
                    .after(adopt_native_lightyear_replicated_entities),
                sync_backdrop_fullscreen_system.after(sync_fullscreen_layer_renderables_system),
                gate_gameplay_camera_system,
                update_owned_ships_panel_system,
                handle_owned_ships_panel_buttons,
                update_loading_overlay_system,
                update_runtime_stream_icon_system,
                watch_in_world_bootstrap_failures,
                update_topdown_camera_system
                    .after(lock_player_entity_to_controlled_entity_end_of_frame),
                sync_ui_overlay_camera_to_gameplay_camera_system
                    .after(update_topdown_camera_system),
                update_camera_motion_state.after(update_topdown_camera_system),
                update_hud_system,
                toggle_debug_overlay_system,
                draw_debug_overlay_system.after(toggle_debug_overlay_system),
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
                backdrop::update_starfield_material_system
                    .after(lock_camera_to_player_entity_end_of_frame),
                backdrop::update_space_background_material_system
                    .after(backdrop::update_starfield_material_system),
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
                logout::logout_despawn_world_entities_system
                    .run_if(resource_equals(LogoutCleanupRequested(true))),
                logout::logout_cleanup_system.run_if(resource_equals(LogoutCleanupRequested(true))),
            );
            #[cfg(target_arch = "wasm32")]
            let logout_chain = (
                logout::request_logout_system.run_if(in_state(ClientAppState::InWorld)),
                logout::request_logout_system.run_if(in_state(ClientAppState::WorldLoading)),
                logout::request_logout_system.run_if(in_state(ClientAppState::CharacterSelect)),
                logout::send_disconnect_notify_and_trigger_system,
                logout::logout_despawn_world_entities_system
                    .run_if(resource_equals(LogoutCleanupRequested(true))),
                logout::logout_cleanup_system.run_if(resource_equals(LogoutCleanupRequested(true))),
            );
            app.add_systems(PreUpdate, logout_chain.chain());
        }
        app.add_systems(
            FixedUpdate,
            (
                apply_predicted_input_to_action_queue,
                enforce_controlled_planar_motion,
            )
                .chain()
                .before(avian2d::prelude::PhysicsSystems::StepSimulation)
                .run_if(in_state(ClientAppState::InWorld)),
        );
    }
    app.run();
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
        // Prevent world sprites/meshes from being rendered twice by the UI overlay camera.
        RenderLayers::layer(UI_OVERLAY_RENDER_LAYER),
        UiOverlayCamera,
    ));
}

fn insert_embedded_fonts(app: &mut App) {
    static BOLD: &[u8] = include_bytes!("../../../../data/fonts/FiraSans-Bold.ttf");
    static REGULAR: &[u8] = include_bytes!("../../../../data/fonts/FiraSans-Regular.ttf");

    let mut fonts = app.world_mut().resource_mut::<Assets<Font>>();
    let bold = fonts
        .add(Font::try_from_bytes(BOLD.to_vec()).expect("embedded FiraSans-Bold.ttf is valid"));
    let regular = fonts.add(
        Font::try_from_bytes(REGULAR.to_vec()).expect("embedded FiraSans-Regular.ttf is valid"),
    );
    app.insert_resource(EmbeddedFonts { bold, regular });
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

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
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
    mut session_ready: ResMut<'_, SessionReadyState>,
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
                    match auth_net::enter_world_request(
                        &client,
                        &gateway_url,
                        access_token,
                        &selected_player_entity_id,
                    ) {
                        Ok(response) if response.accepted => {
                            session.player_entity_id = Some(selected_player_entity_id);
                            auth_state.sent_for_client_entities.clear();
                            auth_state.last_player_entity_id = None;
                            session_ready.ready_player_entity_id = None;
                            session.status =
                                "World entry accepted. Waiting for replication bind...".to_string();
                            next_state.set(ClientAppState::WorldLoading);
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
    shaders::reload_streamed_shaders(&asset_server, &mut shaders, &asset_root.0);
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

    // Black scene background (no shader dependency). Starfield draws on top with transparent background.
    let fallback_mesh = meshes.add(Rectangle::new(1.0, 1.0));
    let fallback_material = color_materials.add(ColorMaterial::from(Color::BLACK));
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
        Camera2d,
        Camera {
            order: 0,
            is_active: false,
            clear_color: ClearColorConfig::None,
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 80.0),
        GameplayCamera,
        TopDownCamera {
            distance: 50.0,
            target_distance: 50.0,
            min_distance: 1.0,
            max_distance: 100.0,
            zoom_units_per_wheel: 2.0,
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

#[allow(clippy::type_complexity)]
fn ensure_fullscreen_layer_fallback_system(
    mut commands: Commands<'_, '_>,
    layers: Query<
        '_,
        '_,
        (
            Entity,
            Option<&FallbackFullscreenLayer>,
            Option<&FullscreenLayerRenderable>,
        ),
        With<FullscreenLayer>,
    >,
    asset_manager: Res<'_, LocalAssetManager>,
    watchdog: Res<'_, BootstrapWatchdogState>,
) {
    let mut fallback_entities = Vec::new();
    let mut has_authoritative_renderable_layer = false;
    for (entity, fallback_marker, renderable) in &layers {
        if fallback_marker.is_some() {
            fallback_entities.push(entity);
        } else if renderable.is_some() {
            has_authoritative_renderable_layer = true;
        }
    }
    if has_authoritative_renderable_layer {
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
        let has_streamed_shader = shaders::fullscreen_layer_shader_ready(
            &asset_root.0,
            &asset_manager,
            &layer.shader_asset_id,
        );
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
            // Temporarily disabled: space background (starfield only).
            if layer.layer_kind == "space_background" {
                continue;
            }
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
                .remove::<FallbackFullscreenLayer>()
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
    let Some(viewport_size) = platform::safe_viewport_size(window) else {
        return;
    };
    let width = viewport_size.x;
    let height = viewport_size.y;
    for mut transform in &mut backdrop_query {
        transform.translation.x = 0.0;
        transform.translation.y = 0.0;
        // Mesh2d uses screen-space-like world units with the 2D camera, so size against viewport.
        transform.scale = Vec3::new(width, height, 1.0);
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

fn streamed_visual_asset_path(asset_id: &str, asset_manager: &LocalAssetManager) -> Option<String> {
    let relative = asset_manager.cached_relative_path(asset_id)?;
    if !(relative.ends_with(".png")
        || relative.ends_with(".jpg")
        || relative.ends_with(".jpeg")
        || relative.ends_with(".webp"))
    {
        return None;
    }
    Some(format!("data/cache_stream/{relative}"))
}

fn streamed_sprite_shader_path(
    asset_id: &str,
    asset_manager: &LocalAssetManager,
) -> Option<String> {
    let relative = asset_manager.cached_relative_path(asset_id)?;
    if !relative.ends_with(".wgsl") {
        return None;
    }
    Some(format!("data/cache_stream/{relative}"))
}

fn resolved_world_sprite_size(
    texture_size_px: Option<UVec2>,
    size_m: Option<&SizeM>,
) -> Option<Vec2> {
    let bounds = size_m.map(|size| Vec2::new(size.width.max(0.1), size.length.max(0.1)));
    match (texture_size_px, bounds) {
        (Some(px), Some(bounds)) if px.x > 0 && px.y > 0 => {
            // Preserve texture aspect while fitting inside physical ship bounds.
            let px_size = Vec2::new(px.x as f32, px.y as f32);
            let scale = (bounds.x / px_size.x).min(bounds.y / px_size.y);
            Some(px_size * scale)
        }
        (None, Some(bounds)) => Some(bounds),
        _ => None,
    }
}

fn read_png_dimensions(path: &std::path::Path) -> Option<UVec2> {
    let bytes = std::fs::read(path).ok()?;
    if bytes.len() < 24 {
        return None;
    }
    const PNG_SIG: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
    if bytes[0..8] != PNG_SIG {
        return None;
    }
    let width = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
    let height = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
    if width == 0 || height == 0 {
        return None;
    }
    Some(UVec2::new(width, height))
}

#[allow(clippy::type_complexity)]
fn suppress_duplicate_predicted_interpolated_visuals_system(
    mut commands: Commands<'_, '_>,
    world_entities: Query<
        '_,
        '_,
        (
            Entity,
            Option<&EntityGuid>,
            Has<ControlledEntity>,
            Has<lightyear::prelude::Predicted>,
            Has<SuppressedPredictedDuplicateVisual>,
        ),
        With<WorldEntity>,
    >,
) {
    let mut best_entity_by_guid = HashMap::<uuid::Uuid, (Entity, i32)>::new();
    for (entity, guid, is_controlled, is_predicted, _is_suppressed) in &world_entities {
        let Some(guid) = guid else { continue };
        let score = if is_controlled {
            3
        } else if is_predicted {
            2
        } else {
            1
        };
        match best_entity_by_guid.get_mut(&guid.0) {
            Some((winner, winner_score)) => {
                if score > *winner_score {
                    *winner = entity;
                    *winner_score = score;
                }
            }
            None => {
                best_entity_by_guid.insert(guid.0, (entity, score));
            }
        }
    }

    for (entity, guid, _is_controlled, _is_predicted, is_suppressed) in &world_entities {
        let should_suppress = guid
            .and_then(|guid| best_entity_by_guid.get(&guid.0).copied())
            .is_some_and(|(winner, _)| winner != entity);
        if should_suppress {
            if let Ok(mut entity_commands) = commands.get_entity(entity) {
                if !is_suppressed {
                    entity_commands.insert(SuppressedPredictedDuplicateVisual);
                }
                entity_commands.insert(Visibility::Hidden);
            }
        } else if is_suppressed && let Ok(mut entity_commands) = commands.get_entity(entity) {
            entity_commands
                .remove::<SuppressedPredictedDuplicateVisual>()
                .insert(Visibility::Visible);
        }
    }
}

#[allow(clippy::type_complexity)]
fn cleanup_streamed_visual_children_system(
    mut commands: Commands<'_, '_>,
    parents: Query<
        '_,
        '_,
        (
            Entity,
            &'_ Children,
            Option<&'_ StreamedVisualAssetId>,
            Has<StreamedVisualAttached>,
            Has<SuppressedPredictedDuplicateVisual>,
            Option<&'_ PlayerTag>,
        ),
        With<WorldEntity>,
    >,
    visual_children: Query<'_, '_, (), With<StreamedVisualChild>>,
) {
    for (
        parent_entity,
        children,
        visual_asset_id,
        has_visual_attached,
        is_suppressed,
        player_tag,
    ) in &parents
    {
        let should_clear_visual =
            visual_asset_id.is_none() || is_suppressed || player_tag.is_some();
        if !should_clear_visual {
            continue;
        }
        let mut removed_any_child = false;
        for child in children.iter() {
            if visual_children.get(child).is_ok() {
                if let Ok(mut entity_commands) = commands.get_entity(child) {
                    entity_commands.despawn();
                }
                removed_any_child = true;
            }
        }
        if (has_visual_attached || removed_any_child)
            && let Ok(mut parent_commands) = commands.get_entity(parent_entity)
        {
            parent_commands.remove::<StreamedVisualAttached>();
        }
    }
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn attach_streamed_visual_assets_system(
    mut commands: Commands<'_, '_>,
    asset_server: Res<'_, AssetServer>,
    images: Res<'_, Assets<Image>>,
    asset_root: Res<'_, AssetRootPath>,
    asset_manager: Res<'_, LocalAssetManager>,
    mut meshes: ResMut<'_, Assets<Mesh>>,
    mut sprite_shader_materials: ResMut<'_, Assets<StreamedSpriteShaderMaterial>>,
    candidates: Query<
        '_,
        '_,
        (
            Entity,
            &StreamedVisualAssetId,
            Option<&SizeM>,
            Option<&StreamedSpriteShaderAssetId>,
        ),
        (
            With<WorldEntity>,
            Without<StreamedVisualAttached>,
            Without<SuppressedPredictedDuplicateVisual>,
        ),
    >,
) {
    for (entity, asset_id, size_m, sprite_shader) in &candidates {
        let Some(path) = streamed_visual_asset_path(&asset_id.0, &asset_manager) else {
            continue;
        };
        let Ok(mut entity_commands) = commands.get_entity(entity) else {
            continue;
        };
        let image_handle = asset_server.load(path.clone());
        let rooted_path = std::path::PathBuf::from(&asset_root.0).join(&path);
        let texture_size_px = images
            .get(&image_handle)
            .map(|image| image.size())
            .or_else(|| read_png_dimensions(&rooted_path));
        let custom_size = resolved_world_sprite_size(texture_size_px, size_m);
        if let Some(sprite_shader) = sprite_shader
            && let Some(shader_path) = streamed_sprite_shader_path(&sprite_shader.0, &asset_manager)
        {
            if shader_path != STREAMED_SPRITE_PIXEL_SHADER_PATH {
                warn!(
                    "unsupported streamed sprite shader path={} (expected {}); falling back to plain sprite",
                    shader_path, STREAMED_SPRITE_PIXEL_SHADER_PATH
                );
            } else if std::path::PathBuf::from(&asset_root.0)
                .join(STREAMED_SPRITE_PIXEL_SHADER_PATH)
                .is_file()
            {
                let quad_mesh = meshes.add(Rectangle::new(1.0, 1.0));
                let material = sprite_shader_materials.add(StreamedSpriteShaderMaterial {
                    image: image_handle.clone(),
                });
                let sprite_size = custom_size.unwrap_or(Vec2::splat(16.0));
                entity_commands.with_children(|child| {
                    child.spawn((
                        StreamedVisualChild,
                        Mesh2d(quad_mesh),
                        MeshMaterial2d(material),
                        Transform::from_xyz(0.0, 0.0, 0.2).with_scale(Vec3::new(
                            sprite_size.x,
                            sprite_size.y,
                            1.0,
                        )),
                    ));
                });
                entity_commands.try_insert(StreamedVisualAttached);
                continue;
            }
        }
        entity_commands.with_children(|child| {
            child.spawn((
                StreamedVisualChild,
                Sprite {
                    image: image_handle,
                    custom_size,
                    ..Default::default()
                },
                Transform::from_xyz(0.0, 0.0, 0.2),
            ));
        });
        entity_commands.try_insert(StreamedVisualAttached);
    }
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
    asset_server: Res<'_, AssetServer>,
    mut shaders: ResMut<'_, Assets<bevy::shader::Shader>>,
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
                if matches!(
                    chunk.asset_id.as_str(),
                    id if id == default_starfield_shader_asset_id()
                        || id == default_space_background_shader_asset_id()
                ) {
                    shaders::reload_streamed_shaders(&asset_server, &mut shaders, &asset_root.0);
                }
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
        With<WorldEntity>,
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
    mut control_request_state: ResMut<'_, ClientControlRequestState>,
    mut panel_state: ResMut<'_, OwnedShipsPanelState>,
) {
    for (interaction, button, mut color) in &mut interactions {
        match *interaction {
            Interaction::Pressed => {
                match &button.action {
                    OwnedShipsPanelAction::FreeRoam => {
                        let target = session.player_entity_id.clone();
                        player_view_state.desired_controlled_entity_id = target.clone();
                        control_request_state.next_request_seq =
                            control_request_state.next_request_seq.saturating_add(1);
                        control_request_state.pending_controlled_entity_id = target;
                        control_request_state.pending_request_seq =
                            Some(control_request_state.next_request_seq);
                        control_request_state.last_sent_request_seq = None;
                        control_request_state.last_sent_at_s = 0.0;
                        // Control swap is ack-gated: keep current authoritative control target
                        // until matching server ack/reject arrives.
                        player_view_state.detached_free_camera = false;
                        player_view_state.selected_entity_id = None;
                    }
                    OwnedShipsPanelAction::ControlEntity(entity_id) => {
                        player_view_state.desired_controlled_entity_id = Some(entity_id.clone());
                        control_request_state.next_request_seq =
                            control_request_state.next_request_seq.saturating_add(1);
                        control_request_state.pending_controlled_entity_id =
                            Some(entity_id.clone());
                        control_request_state.pending_request_seq =
                            Some(control_request_state.next_request_seq);
                        control_request_state.last_sent_request_seq = None;
                        control_request_state.last_sent_at_s = 0.0;
                        // Control swap is ack-gated: do not move local prediction ownership
                        // until authoritative control ack/reject is received.
                        player_view_state.detached_free_camera = false;
                        player_view_state.selected_entity_id = Some(entity_id.clone());
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
        (
            &Transform,
            Option<&Rotation>,
            Option<&LinearVelocity>,
            &HealthPool,
        ),
        With<ControlledEntity>,
    >,
    camera_query: Query<'_, '_, &Transform, With<GameplayCamera>>,
    mut hud_query: Query<'_, '_, &mut Text, With<HudText>>,
) {
    let (pos, heading_rad, vel, health_text) = if let Ok((
        transform,
        maybe_rotation,
        maybe_velocity,
        health,
    )) = controlled_query.single()
    {
        let vel = maybe_velocity.map_or(Vec2::ZERO, |velocity| velocity.0);
        let heading_rad = maybe_rotation
            .map(|rotation| rotation.as_radians())
            .unwrap_or_else(|| vel.to_angle());
        (
            transform.translation,
            heading_rad,
            vel,
            format!("{:.0}/{:.0}", health.current, health.maximum),
        )
    } else {
        let Ok(camera_transform) = camera_query.single() else {
            return;
        };
        (
            camera_transform.translation,
            0.0,
            Vec2::ZERO,
            "--/--".to_string(),
        )
    };
    let Ok(mut text) = hud_query.single_mut() else {
        return;
    };

    // Convert math convention (CCW from +Y) to compass convention (CW from north).
    let heading_deg = {
        let raw = (-heading_rad.to_degrees()).rem_euclid(360.0);
        if raw == 0.0 { 0.0_f32 } else { raw }
    };
    let speed = vel.length();
    let content = format!(
        "SIDEREAL FLIGHT\nPos: ({:.0}, {:.0})\nSpeed: {:.1} m/s\nVel: ({:.1}, {:.1})\nHeading: {:.0}\u{00b0}\nHealth: {}\nControls: W/S thrust, A/D turn, SPACE brake, F3 debug overlay, ESC logout",
        pos.x, pos.y, speed, vel.x, vel.y, heading_deg, health_text
    );
    content.clone_into(&mut **text);
}

#[cfg(test)]
mod tests {
    use super::*;
    use sidereal_game::{ActionQueue, FlightComputer};
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
        assert!(replication::should_defer_controlled_predicted_adoption(
            true, false, true, true
        ));
        assert!(replication::should_defer_controlled_predicted_adoption(
            true, true, false, true
        ));
        assert!(replication::should_defer_controlled_predicted_adoption(
            true, true, true, false
        ));
    }

    #[test]
    fn predicted_controlled_adoption_proceeds_when_requirements_met() {
        assert!(!replication::should_defer_controlled_predicted_adoption(
            true, true, true, true
        ));
        assert!(!replication::should_defer_controlled_predicted_adoption(
            false, false, false, false
        ));
    }

    #[test]
    fn realtime_input_send_policy_sends_on_input_or_target_change() {
        assert!(input::should_send_realtime_input_message(
            10.0, 9.95, true, false
        ));
        assert!(input::should_send_realtime_input_message(
            10.0, 9.95, false, true
        ));
    }

    #[test]
    fn realtime_input_send_policy_sends_heartbeat_when_idle() {
        assert!(input::should_send_realtime_input_message(
            10.0, 9.89, false, false
        ));
    }

    #[test]
    fn realtime_input_send_policy_skips_when_idle_within_heartbeat_window() {
        assert!(!input::should_send_realtime_input_message(
            10.0, 9.95, false, false
        ));
    }

    #[test]
    fn camera_anchor_prefers_local_player_entity() {
        let mut registry = RuntimeEntityHierarchy::default();
        let player_entity = Entity::from_bits(1);
        let controlled_entity = Entity::from_bits(2);
        registry
            .by_entity_id
            .insert("player:test".to_string(), player_entity);
        registry
            .by_entity_id
            .insert("ship:test".to_string(), controlled_entity);

        let session = ClientSession {
            player_entity_id: Some("player:test".to_string()),
            ..Default::default()
        };
        let player_view_state = LocalPlayerViewState {
            controlled_entity_id: Some("ship:test".to_string()),
            ..Default::default()
        };

        let resolved = replication::resolve_camera_anchor_entity(&session, &player_view_state, &registry);
        assert_eq!(resolved, Some(player_entity));
    }

    #[test]
    fn camera_anchor_missing_player_entity_returns_none() {
        let mut registry = RuntimeEntityHierarchy::default();
        registry
            .by_entity_id
            .insert("ship:test".to_string(), Entity::from_bits(2));
        let session = ClientSession {
            player_entity_id: Some("player:test".to_string()),
            ..Default::default()
        };
        let player_view_state = LocalPlayerViewState {
            controlled_entity_id: Some("ship:test".to_string()),
            ..Default::default()
        };

        let resolved = replication::resolve_camera_anchor_entity(&session, &player_view_state, &registry);
        assert!(resolved.is_none());
    }

    #[test]
    fn cleanup_streamed_visual_children_removes_stale_player_visuals() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_systems(Update, cleanup_streamed_visual_children_system);

        let parent = app
            .world_mut()
            .spawn((WorldEntity, PlayerTag, StreamedVisualAttached))
            .id();
        let child = app
            .world_mut()
            .spawn((
                StreamedVisualChild,
                Transform::default(),
                GlobalTransform::default(),
            ))
            .id();
        app.world_mut().entity_mut(parent).add_child(child);

        app.update();

        assert!(app.world().get_entity(child).is_err());
        let parent_ref = app.world().entity(parent);
        assert!(!parent_ref.contains::<StreamedVisualAttached>());
    }

    #[test]
    fn motion_ownership_enforcement_strips_remote_root_writers() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.insert_resource(LocalSimulationDebugMode(false));
        app.insert_resource(NearbyCollisionProxyTuning {
            radius_m: 200.0,
            max_proxies: 4,
        });
        app.insert_resource(ClientSession {
            player_entity_id: Some("player:test".to_string()),
            ..Default::default()
        });
        app.insert_resource(LocalPlayerViewState {
            controlled_entity_id: Some("ship:local".to_string()),
            ..Default::default()
        });
        let mut registry = RuntimeEntityHierarchy::default();
        app.add_systems(Update, enforce_motion_ownership_for_world_entities);

        let local = app.world_mut().spawn((WorldEntity,));
        let local_id = local.id();
        registry
            .by_entity_id
            .insert("ship:local".to_string(), local_id);
        app.insert_resource(registry);

        let remote = app.world_mut().spawn((
            WorldEntity,
            ActionQueue::default(),
            FlightComputer {
                profile: "test".to_string(),
                throttle: 0.0,
                yaw_input: 0.0,
                brake_active: false,
                turn_rate_deg_s: 0.0,
            },
            RigidBody::Dynamic,
            Mass(1000.0),
            lightyear::prelude::Predicted,
        ));
        let remote_id = remote.id();

        app.update();

        let remote_ref = app.world().entity(remote_id);
        assert!(!remote_ref.contains::<ActionQueue>());
        assert!(!remote_ref.contains::<FlightComputer>());
        assert!(!remote_ref.contains::<RigidBody>());
        assert!(!remote_ref.contains::<Mass>());
        assert!(!remote_ref.contains::<lightyear::prelude::Predicted>());
        assert!(remote_ref.contains::<lightyear::prelude::Interpolated>());
    }

    #[test]
    fn motion_ownership_enforcement_keeps_controlled_root_predicted() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.insert_resource(LocalSimulationDebugMode(false));
        app.insert_resource(NearbyCollisionProxyTuning {
            radius_m: 200.0,
            max_proxies: 4,
        });
        app.insert_resource(ClientSession {
            player_entity_id: Some("player:test".to_string()),
            ..Default::default()
        });
        app.insert_resource(LocalPlayerViewState {
            controlled_entity_id: Some("ship:test".to_string()),
            ..Default::default()
        });
        let mut registry = RuntimeEntityHierarchy::default();
        app.add_systems(Update, enforce_motion_ownership_for_world_entities);

        let controlled = app.world_mut().spawn((
            WorldEntity,
            ActionQueue::default(),
            FlightComputer {
                profile: "test".to_string(),
                throttle: 0.0,
                yaw_input: 0.0,
                brake_active: false,
                turn_rate_deg_s: 0.0,
            },
            ControlledEntity {
                entity_id: "ship:test".to_string(),
                player_entity_id: "player:test".to_string(),
            },
            lightyear::prelude::Interpolated,
        ));
        let controlled_id = controlled.id();
        registry
            .by_entity_id
            .insert("ship:test".to_string(), controlled_id);
        app.insert_resource(registry);

        app.update();

        let controlled_ref = app.world().entity(controlled_id);
        assert!(controlled_ref.contains::<ActionQueue>());
        assert!(controlled_ref.contains::<FlightComputer>());
        assert!(controlled_ref.contains::<lightyear::prelude::Predicted>());
        assert!(!controlled_ref.contains::<lightyear::prelude::Interpolated>());
    }

    #[test]
    fn transform_sync_applies_replicated_position_and_rotation() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_systems(Update, sync_world_entity_transforms_from_physics);

        let expected_position = Vec2::new(42.0, -17.5);
        let expected_rotation = Rotation::radians(0.7);
        let entity = app.world_mut().spawn((
            WorldEntity,
            Transform::default(),
            Position(expected_position),
            expected_rotation,
        ));
        let entity_id = entity.id();

        app.update();

        let transform = app.world().entity(entity_id).get::<Transform>().unwrap();
        assert_eq!(transform.translation.x, expected_position.x);
        assert_eq!(transform.translation.y, expected_position.y);
        assert_eq!(transform.translation.z, 0.0);
        assert_eq!(transform.rotation, Quat::from(expected_rotation));
    }

    #[test]
    fn transform_sync_skips_interpolated_entities() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_systems(Update, sync_world_entity_transforms_from_physics);

        let expected_position = Vec2::new(42.0, -17.5);
        let expected_rotation = Rotation::radians(0.7);
        let entity = app.world_mut().spawn((
            WorldEntity,
            lightyear::prelude::Interpolated,
            Transform::default(),
            Position(expected_position),
            expected_rotation,
        ));
        let entity_id = entity.id();

        app.update();

        let transform = app.world().entity(entity_id).get::<Transform>().unwrap();
        assert_eq!(transform.translation, Vec3::ZERO);
        assert_eq!(transform.rotation, Quat::IDENTITY);
    }

    #[test]
    fn interpolated_visual_smoothing_moves_between_snapshots() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_systems(
            Update,
            (
                refresh_interpolated_visual_targets_system,
                apply_interpolated_visual_smoothing_system
                    .after(refresh_interpolated_visual_targets_system),
            ),
        );

        let entity_id = app
            .world_mut()
            .spawn((
                WorldEntity,
                lightyear::prelude::Interpolated,
                Transform::default(),
                Position(Vec2::ZERO),
                Rotation::IDENTITY,
            ))
            .id();

        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(16));
        app.update();
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(16));
        app.update();

        {
            let mut entity = app.world_mut().entity_mut(entity_id);
            entity.insert(Position(Vec2::new(10.0, 0.0)));
        }

        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(16));
        app.update();

        let transform = app.world().entity(entity_id).get::<Transform>().unwrap();
        assert!(transform.translation.x > 0.0);
        assert!(transform.translation.x < 10.0);
    }

    #[test]
    fn motion_ownership_enforcement_defers_when_control_target_unresolved() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.insert_resource(LocalSimulationDebugMode(false));
        app.insert_resource(NearbyCollisionProxyTuning {
            radius_m: 200.0,
            max_proxies: 4,
        });
        app.insert_resource(ClientSession {
            player_entity_id: Some("player:test".to_string()),
            ..Default::default()
        });
        app.insert_resource(LocalPlayerViewState {
            controlled_entity_id: Some("ship:missing".to_string()),
            ..Default::default()
        });
        app.insert_resource(RuntimeEntityHierarchy::default());
        app.add_systems(Update, enforce_motion_ownership_for_world_entities);

        let remote = app.world_mut().spawn((
            WorldEntity,
            ActionQueue::default(),
            FlightComputer {
                profile: "test".to_string(),
                throttle: 0.0,
                yaw_input: 0.0,
                brake_active: false,
                turn_rate_deg_s: 0.0,
            },
            RigidBody::Dynamic,
            Mass(1000.0),
            lightyear::prelude::Predicted,
        ));
        let remote_id = remote.id();

        app.update();

        // No stripping while authoritative control target is unresolved.
        let remote_ref = app.world().entity(remote_id);
        assert!(remote_ref.contains::<ActionQueue>());
        assert!(remote_ref.contains::<FlightComputer>());
        assert!(remote_ref.contains::<RigidBody>());
        assert!(remote_ref.contains::<Mass>());
    }

    #[test]
    fn motion_ownership_enforcement_keeps_nearby_remote_collision_proxy() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.insert_resource(LocalSimulationDebugMode(false));
        app.insert_resource(NearbyCollisionProxyTuning {
            radius_m: 200.0,
            max_proxies: 4,
        });
        app.insert_resource(ClientSession {
            player_entity_id: Some("player:test".to_string()),
            ..Default::default()
        });
        app.insert_resource(LocalPlayerViewState {
            controlled_entity_id: Some("ship:local".to_string()),
            ..Default::default()
        });
        let mut registry = RuntimeEntityHierarchy::default();
        app.add_systems(Update, enforce_motion_ownership_for_world_entities);

        let local = app.world_mut().spawn((
            WorldEntity,
            Position(Vec2::ZERO),
            Rotation::IDENTITY,
            LinearVelocity(Vec2::ZERO),
        ));
        let local_id = local.id();
        registry
            .by_entity_id
            .insert("ship:local".to_string(), local_id);
        app.insert_resource(registry);

        let remote = app.world_mut().spawn((
            WorldEntity,
            Position(Vec2::new(50.0, 0.0)),
            Rotation::IDENTITY,
            LinearVelocity(Vec2::ZERO),
            ActionQueue::default(),
            FlightComputer {
                profile: "test".to_string(),
                throttle: 0.0,
                yaw_input: 0.0,
                brake_active: false,
                turn_rate_deg_s: 0.0,
            },
        ));
        let remote_id = remote.id();

        app.update();

        let remote_ref = app.world().entity(remote_id);
        assert!(remote_ref.contains::<NearbyCollisionProxy>());
        assert!(remote_ref.contains::<RigidBody>());
        assert!(remote_ref.contains::<Collider>());
        assert!(!remote_ref.contains::<ActionQueue>());
        assert!(!remote_ref.contains::<FlightComputer>());
    }
}
