#[path = "../auth_ui.rs"]
mod auth_ui;
#[path = "../dialog_ui.rs"]
mod dialog_ui;

mod auth_net;
mod backdrop;
mod bootstrap;
mod components;
mod control;
mod input;
mod logout;
mod platform;
mod remote;
mod resources;
mod shaders;
mod state;
mod transport;

pub(crate) use auth_net::submit_auth_request;
pub(crate) use backdrop::{SpaceBackgroundMaterial, StarfieldMaterial, StreamedSpriteShaderMaterial};
pub(crate) use components::*;
pub(crate) use platform::*;
pub(crate) use remote::*;
pub(crate) use resources::*;
pub(crate) use state::*;

use avian2d::prelude::*;
use bevy::asset::{AssetApp, AssetPlugin};
use bevy::camera::visibility::RenderLayers;
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
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
use lightyear::prediction::correction::CorrectionPolicy;
use lightyear::prediction::prelude::PredictionManager;
use lightyear::prelude::client::ClientPlugins;
use lightyear::prelude::client::{Client, Connected};
use lightyear::prelude::input::native::ActionState;
use lightyear::prelude::{MessageReceiver, MessageSender};
use sidereal_asset_runtime::{
    AssetCacheIndexRecord, cache_index_path, load_cache_index, save_cache_index, sha256_hex,
};
use sidereal_core::remote_inspect::RemoteInspectConfig;
use sidereal_game::{
    ActionQueue, ControlledEntityGuid, EntityGuid, FlightComputer, FullscreenLayer,
    Hardpoint, HealthPool, MountedOn, OwnerId, PlayerTag, ScannerRangeM, SiderealGameCorePlugin,
    SizeM, SpriteShaderAssetId, TotalMassKg, VisualAssetId, angular_inertia_from_size,
    apply_engine_thrust, clamp_angular_velocity, default_corvette_asset_id,
    default_corvette_mass_kg, default_corvette_size, default_flight_action_capabilities,
    default_space_background_shader_asset_id, default_starfield_shader_asset_id,
    process_flight_actions, recompute_total_mass, stabilize_idle_motion,
    validate_action_capabilities,
};
use sidereal_net::{
    AssetAckMessage, AssetRequestMessage, AssetStreamChunkMessage, AssetStreamManifestMessage,
    ControlChannel, PlayerInput, RequestedAsset, register_lightyear_protocol,
};
use sidereal_runtime_sync::{
    RuntimeEntityHierarchy, parse_guid_from_entity_id, register_runtime_entity,
};
use std::collections::{HashMap, HashSet};
use std::time::Duration;

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
    // Prefer concrete runtime entities first (ship/module/hardpoint/player).
    // Legacy worlds may have GUID collisions across entity families.
    for prefix in ["ship", "player", "module", "hardpoint"] {
        let candidate = format!("{prefix}:{guid}");
        if entity_registry.by_entity_id.contains_key(&candidate) {
            return Some(candidate);
        }
    }
    if parse_guid_from_entity_id(local_player_entity_id)
        .is_some_and(|player_guid| player_guid.to_string() == guid)
    {
        return Some(local_player_entity_id.to_string());
    }
    None
}

fn resolve_authoritative_control_entity_id_from_registry(
    entity_registry: &RuntimeEntityHierarchy,
    local_player_entity_id: &str,
    controlled_entity_guid: Option<&ControlledEntityGuid>,
) -> Option<String> {
    let control_guid = controlled_entity_guid.and_then(|v| v.0.as_deref())?;

    if parse_guid_from_entity_id(local_player_entity_id)
        .is_some_and(|player_guid| player_guid.to_string() == control_guid)
    {
        return Some(local_player_entity_id.to_string());
    }

    runtime_entity_id_from_guid(entity_registry, local_player_entity_id, control_guid)
}

fn resolve_authoritative_control_entity_id_with_snapshot(
    entity_registry: &RuntimeEntityHierarchy,
    runtime_entity_id_by_guid: &HashMap<String, String>,
    local_player_entity_id: &str,
    controlled_entity_guid: Option<&ControlledEntityGuid>,
) -> Option<String> {
    let control_guid = controlled_entity_guid.and_then(|v| v.0.as_deref())?;

    if parse_guid_from_entity_id(local_player_entity_id)
        .is_some_and(|player_guid| player_guid.to_string() == control_guid)
    {
        return Some(local_player_entity_id.to_string());
    }

    runtime_entity_id_by_guid
        .get(control_guid)
        .cloned()
        .or_else(|| {
            runtime_entity_id_from_guid(entity_registry, local_player_entity_id, control_guid)
        })
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
                input::enforce_single_input_marker_owner.before(input::send_lightyear_input_messages),
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
                control::receive_lightyear_control_results.after(control::send_lightyear_control_requests),
                control::log_client_control_state_changes.after(control::receive_lightyear_control_results),
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
                control::receive_lightyear_control_results.after(control::send_lightyear_control_requests),
                control::log_client_control_state_changes.after(control::receive_lightyear_control_results),
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
                backdrop::update_starfield_material_system.after(update_camera_motion_state),
                backdrop::update_space_background_material_system.after(update_camera_motion_state),
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
            )
                .chain()
                .run_if(in_state(ClientAppState::InWorld)),
        );
        app.add_systems(
            FixedPreUpdate,
            (
                input::enforce_single_input_marker_owner.before(input::send_lightyear_input_messages),
                input::send_lightyear_input_messages,
                bevy::ecs::schedule::ApplyDeferred,
            )
                .chain()
                .in_set(lightyear::prelude::client::input::InputSystems::WriteClientInputs)
                .run_if(in_state(ClientAppState::InWorld)),
        );
        app.add_systems(
            PreUpdate,
            (
                logout::logout_to_auth_system.run_if(in_state(ClientAppState::InWorld)),
                logout::logout_to_auth_system.run_if(in_state(ClientAppState::WorldLoading)),
                logout::logout_to_auth_system.run_if(in_state(ClientAppState::CharacterSelect)),
            ),
        );
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
        (Without<Camera>, Without<GameplayCamera>),
    >,
    mut free_camera: ResMut<'_, FreeCameraState>,
    mut camera_query: Query<
        '_,
        '_,
        (&mut Transform, &mut Projection, &mut TopDownCamera),
        (With<GameplayCamera>, Without<ControlledEntity>),
    >,
) {
    let Ok((mut camera_transform, mut projection, mut camera)) = camera_query.single_mut() else {
        return;
    };

    let mut wheel_delta_y = 0.0f32;
    for event in mouse_wheel_events.read() {
        let normalized = match event.unit {
            MouseScrollUnit::Line => event.y,
            MouseScrollUnit::Pixel => event.y / 32.0,
        };
        wheel_delta_y += normalized.clamp(-4.0, 4.0);
    }
    if wheel_delta_y != 0.0 {
        camera.target_distance = (camera.target_distance
            - wheel_delta_y * camera.zoom_units_per_wheel)
            .clamp(camera.min_distance, camera.max_distance);
    }
    let dt = time.delta_secs();
    let zoom_alpha = 1.0 - (-camera.zoom_smoothness * dt).exp();
    camera.distance = camera.distance.lerp(camera.target_distance, zoom_alpha);
    // Camera2d zoom is controlled by orthographic projection scale, not transform.z.
    if let Projection::Orthographic(ortho) = &mut *projection {
        ortho.scale = (camera.distance * ORTHO_SCALE_PER_DISTANCE).max(0.01);
    }

    let follow_anchor =
        resolve_camera_anchor_entity(&session, &player_view_state, &entity_registry)
            .and_then(|entity| anchor_query.get(entity).ok())
            .map(|(anchor_transform, anchor_position)| {
                anchor_position
                    .map(|p| p.0)
                    .unwrap_or_else(|| anchor_transform.translation.truncate())
            });

    let (focus_xy, snap_focus) = if player_view_state.detached_free_camera {
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
        // Detached free-camera can be smoothed for ergonomics.
        (free_camera.position_xy, false)
    } else if let Some(anchor_xy) = follow_anchor {
        free_camera.position_xy = anchor_xy;
        free_camera.initialized = true;
        // Controlled mode must hard lock to anchor every frame.
        (anchor_xy, true)
    } else {
        let fallback_xy = camera_transform.translation.truncate();
        free_camera.position_xy = fallback_xy;
        free_camera.initialized = true;
        (fallback_xy, true)
    };
    if !camera.focus_initialized {
        camera.filtered_focus_xy = focus_xy;
        camera.focus_initialized = true;
    } else if snap_focus {
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
    camera_transform.translation.z = 80.0;
    camera_transform.rotation = Quat::IDENTITY;
}

#[allow(clippy::type_complexity)]
fn sync_ui_overlay_camera_to_gameplay_camera_system(
    gameplay_camera: Query<
        '_,
        '_,
        (&Transform, &Projection),
        (With<GameplayCamera>, Without<UiOverlayCamera>),
    >,
    mut ui_camera: Query<
        '_,
        '_,
        (&mut Transform, &mut Projection),
        (With<UiOverlayCamera>, Without<GameplayCamera>),
    >,
) {
    let Ok((gameplay_transform, gameplay_projection)) = gameplay_camera.single() else {
        return;
    };
    for (mut ui_transform, mut ui_projection) in &mut ui_camera {
        ui_transform.translation.x = gameplay_transform.translation.x;
        ui_transform.translation.y = gameplay_transform.translation.y;
        ui_transform.translation.z = gameplay_transform.translation.z;
        if let (Projection::Orthographic(ui_ortho), Projection::Orthographic(game_ortho)) =
            (&mut *ui_projection, gameplay_projection)
        {
            ui_ortho.scale = game_ortho.scale;
        }
    }
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
fn lock_player_entity_to_controlled_entity_end_of_frame(
    session: Res<'_, ClientSession>,
    player_view_state: Res<'_, LocalPlayerViewState>,
    entity_registry: Res<'_, RuntimeEntityHierarchy>,
    mut queries: ParamSet<
        '_,
        '_,
        (
            Query<
                '_,
                '_,
                (
                    &'_ Transform,
                    Option<&'_ Position>,
                    Option<&'_ Rotation>,
                    Option<&'_ LinearVelocity>,
                    Option<&'_ AngularVelocity>,
                ),
                Without<Camera>,
            >,
            Query<
                '_,
                '_,
                (
                    &'_ mut Transform,
                    Option<&'_ mut Position>,
                    Option<&'_ mut Rotation>,
                    Option<&'_ mut LinearVelocity>,
                    Option<&'_ mut AngularVelocity>,
                ),
                (With<PlayerTag>, Without<Camera>),
            >,
        ),
    >,
) {
    let Some(player_runtime_id) = session.player_entity_id.as_ref() else {
        return;
    };
    let Some(&player_entity) = entity_registry.by_entity_id.get(player_runtime_id.as_str()) else {
        return;
    };
    let controlled_runtime_id = player_view_state
        .controlled_entity_id
        .as_deref()
        .unwrap_or(player_runtime_id.as_str());
    let Some(&controlled_entity) = entity_registry.by_entity_id.get(controlled_runtime_id) else {
        return;
    };
    if player_entity == controlled_entity {
        // Self-control is valid; nothing to mirror.
        return;
    }
    let (
        source_xy,
        source_z,
        source_transform_rotation,
        source_rotation,
        source_linear_velocity,
        source_angular_velocity,
    ) = {
        let source_query = queries.p0();
        let Ok((
            source_transform,
            source_position,
            source_rotation,
            source_linear_velocity,
            source_angular_velocity,
        )) = source_query.get(controlled_entity)
        else {
            return;
        };
        (
            source_position
                .map(|position| position.0)
                .unwrap_or_else(|| source_transform.translation.truncate()),
            source_transform.translation.z,
            source_transform.rotation,
            source_rotation.copied(),
            source_linear_velocity.map(|v| v.0),
            source_angular_velocity.map(|v| v.0),
        )
    };

    let mut player_query = queries.p1();
    let Ok((
        mut player_transform,
        player_position,
        player_rotation,
        player_linear_velocity,
        player_angular_velocity,
    )) = player_query.get_mut(player_entity)
    else {
        return;
    };

    player_transform.translation.x = source_xy.x;
    player_transform.translation.y = source_xy.y;
    player_transform.translation.z = source_z;
    player_transform.rotation = source_transform_rotation;

    if let Some(mut player_position) = player_position {
        player_position.0 = source_xy;
    }
    if let (Some(mut player_rotation), Some(source_rotation)) = (player_rotation, source_rotation) {
        *player_rotation = source_rotation;
    }
    if let (Some(mut player_linear_velocity), Some(source_linear_velocity)) =
        (player_linear_velocity, source_linear_velocity)
    {
        player_linear_velocity.0 = source_linear_velocity;
    }
    if let (Some(mut player_angular_velocity), Some(source_angular_velocity)) =
        (player_angular_velocity, source_angular_velocity)
    {
        player_angular_velocity.0 = source_angular_velocity;
    }
}

#[allow(clippy::type_complexity)]
fn lock_camera_to_player_entity_end_of_frame(
    session: Res<'_, ClientSession>,
    entity_registry: Res<'_, RuntimeEntityHierarchy>,
    anchor_query: Query<
        '_,
        '_,
        (&'_ Transform, Option<&'_ Position>),
        (Without<Camera>, Without<GameplayCamera>),
    >,
    mut camera_query: Query<
        '_,
        '_,
        (&'_ mut Transform, &'_ mut TopDownCamera),
        (With<GameplayCamera>, Without<ControlledEntity>),
    >,
) {
    let Some(player_runtime_id) = session.player_entity_id.as_ref() else {
        return;
    };
    let Some(&player_entity) = entity_registry.by_entity_id.get(player_runtime_id.as_str()) else {
        return;
    };
    let Ok((anchor_transform, anchor_position)) = anchor_query.get(player_entity) else {
        return;
    };
    let Ok((mut camera_transform, mut camera)) = camera_query.single_mut() else {
        return;
    };
    let anchor_xy = anchor_position
        .map(|p| p.0)
        .unwrap_or_else(|| anchor_transform.translation.truncate());
    camera.look_ahead_offset = Vec2::ZERO;
    camera.filtered_focus_xy = anchor_xy;
    camera.focus_initialized = true;
    camera_transform.translation.x = anchor_xy.x;
    camera_transform.translation.y = anchor_xy.y;
    camera_transform.translation.z = 80.0;
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

#[allow(clippy::type_complexity)]
fn audit_active_world_cameras_system(
    time: Res<'_, Time>,
    mut last_log_at_s: Local<'_, f64>,
    cameras: Query<
        '_,
        '_,
        (
            Entity,
            &'_ Camera,
            Option<&'_ RenderLayers>,
            Has<GameplayCamera>,
            Has<UiOverlayCamera>,
        ),
    >,
) {
    let now_s = time.elapsed_secs_f64();
    if now_s - *last_log_at_s < 5.0 {
        return;
    }
    *last_log_at_s = now_s;
    let world_cameras = cameras
        .iter()
        .filter(|(_, camera, layers, _, _)| camera.is_active && layers.is_none())
        .collect::<Vec<_>>();
    if world_cameras.len() > 1 {
        warn!(
            "multiple active default-layer cameras detected: {:?}",
            world_cameras
                .iter()
                .map(|(entity, camera, _, is_gameplay, is_ui)| format!(
                    "entity={entity:?} order={} gameplay={} ui={}",
                    camera.order, is_gameplay, is_ui
                ))
                .collect::<Vec<_>>()
        );
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
        let has_streamed_shader =
            shaders::fullscreen_layer_shader_ready(&asset_root.0, &asset_manager, &layer.shader_asset_id);
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

#[allow(clippy::type_complexity)]
fn sync_world_entity_transforms_from_physics(
    mut entities: Query<
        '_,
        '_,
        (&mut Transform, Option<&Position>, Option<&Rotation>),
        (
            With<WorldEntity>,
            Or<(With<Position>, With<Rotation>)>,
            Without<Camera>,
            Without<lightyear::prelude::Interpolated>,
        ),
    >,
) {
    for (mut transform, position, rotation) in &mut entities {
        if let Some(position) = position {
            transform.translation.x = position.0.x;
            transform.translation.y = position.0.y;
        }
        if let Some(rotation) = rotation {
            transform.rotation = (*rotation).into();
        }
        // Keep 2D gameplay entities constrained to planar render depth.
        transform.translation.z = 0.0;
    }
}

#[allow(clippy::type_complexity)]
fn refresh_interpolated_visual_targets_system(
    time: Res<'_, Time>,
    mut commands: Commands<'_, '_>,
    mut entities: Query<
        '_,
        '_,
        (
            Entity,
            &Position,
            Option<&Rotation>,
            &mut Transform,
            Option<&mut InterpolatedVisualSmoothing>,
        ),
        (
            With<WorldEntity>,
            With<lightyear::prelude::Interpolated>,
            Without<SuppressedPredictedDuplicateVisual>,
            Or<(Changed<Position>, Changed<Rotation>)>,
        ),
    >,
) {
    let now_s = time.elapsed_secs_f64();
    for (entity, position, rotation, mut transform, smoothing) in &mut entities {
        let target_pos = position.0;
        let target_rot: Quat = rotation
            .copied()
            .map(Quat::from)
            .unwrap_or(transform.rotation);

        if let Some(mut smoothing) = smoothing {
            let interval_s = (now_s - smoothing.last_snapshot_at_s) as f32;
            let duration_s = interval_s.clamp(1.0 / 120.0, 0.25);
            smoothing.from_pos = transform.translation.truncate();
            smoothing.to_pos = target_pos;
            smoothing.from_rot = transform.rotation;
            smoothing.to_rot = target_rot;
            smoothing.elapsed_s = 0.0;
            smoothing.duration_s = duration_s;
            smoothing.last_snapshot_at_s = now_s;
        } else {
            transform.translation.x = target_pos.x;
            transform.translation.y = target_pos.y;
            transform.translation.z = 0.0;
            transform.rotation = target_rot;
            commands.entity(entity).insert(InterpolatedVisualSmoothing {
                from_pos: target_pos,
                to_pos: target_pos,
                from_rot: target_rot,
                to_rot: target_rot,
                elapsed_s: 1.0 / 30.0,
                duration_s: 1.0 / 30.0,
                last_snapshot_at_s: now_s,
            });
        }
    }
}

#[allow(clippy::type_complexity)]
fn apply_interpolated_visual_smoothing_system(
    time: Res<'_, Time>,
    mut entities: Query<
        '_,
        '_,
        (&mut Transform, &mut InterpolatedVisualSmoothing),
        (
            With<WorldEntity>,
            With<lightyear::prelude::Interpolated>,
            Without<SuppressedPredictedDuplicateVisual>,
        ),
    >,
) {
    let dt = time.delta_secs().max(0.0);
    for (mut transform, mut smoothing) in &mut entities {
        smoothing.elapsed_s = (smoothing.elapsed_s + dt).max(0.0);
        let alpha = if smoothing.duration_s <= 0.0 {
            1.0
        } else {
            (smoothing.elapsed_s / smoothing.duration_s).clamp(0.0, 1.0)
        };
        let pos = smoothing.from_pos.lerp(smoothing.to_pos, alpha);
        transform.translation.x = pos.x;
        transform.translation.y = pos.y;
        transform.translation.z = 0.0;
        transform.rotation = smoothing.from_rot.slerp(smoothing.to_rot, alpha);
    }
}

fn transition_world_loading_to_in_world(
    app_state: Option<Res<'_, State<ClientAppState>>>,
    session: Res<'_, ClientSession>,
    session_ready: Res<'_, SessionReadyState>,
    entity_registry: Res<'_, RuntimeEntityHierarchy>,
    mut next_state: ResMut<'_, NextState<ClientAppState>>,
) {
    if !app_state
        .as_ref()
        .is_some_and(|state| **state == ClientAppState::WorldLoading)
    {
        return;
    }
    let Some(local_player_entity_id) = session.player_entity_id.as_ref() else {
        return;
    };
    if session_ready.ready_player_entity_id.as_deref() != Some(local_player_entity_id.as_str()) {
        return;
    }
    if !entity_registry
        .by_entity_id
        .contains_key(local_player_entity_id)
    {
        return;
    }
    next_state.set(ClientAppState::InWorld);
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
            Option<&'_ VisualAssetId>,
            Option<&'_ SpriteShaderAssetId>,
        ),
        (
            With<lightyear::prelude::Replicated>,
            Without<ReplicatedAdoptionHandled>,
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
    let mut runtime_entity_id_by_guid = HashMap::<String, String>::new();
    for (_, guid, _, mounted_on, hardpoint, player_tag, _, _, _, _, _, _, _, _) in
        &replicated_entities
    {
        let Some(guid) = guid else {
            continue;
        };
        if mounted_on.is_some() || hardpoint.is_some() {
            continue;
        }
        let runtime_entity_id = if player_tag.is_some() {
            format!("player:{}", guid.0)
        } else {
            format!("ship:{}", guid.0)
        };
        let guid_key = guid.0.to_string();
        match runtime_entity_id_by_guid.get(&guid_key) {
            Some(existing) => {
                // Deterministic disambiguation for legacy GUID collisions: prefer ship over player.
                if existing.starts_with("player:") && runtime_entity_id.starts_with("ship:") {
                    runtime_entity_id_by_guid.insert(guid_key, runtime_entity_id);
                }
            }
            None => {
                runtime_entity_id_by_guid.insert(guid_key, runtime_entity_id);
            }
        }
    }

    // Resolve authoritative controlled ID from the replicated local player entity
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
        _,
        _,
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
        let controlled_id = resolve_authoritative_control_entity_id_with_snapshot(
            &entity_registry,
            &runtime_entity_id_by_guid,
            local_player_entity_id,
            controlled_entity_guid,
        );
        if let Some(controlled_id) = controlled_id {
            player_view_state.controlled_entity_id = Some(controlled_id);
            authoritative_controlled_entity_id = player_view_state.controlled_entity_id.clone();
        }
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
        visual_asset_id,
        sprite_shader_asset_id,
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
            commands
                .entity(entity)
                .insert((ReplicatedAdoptionHandled, Visibility::Hidden));
            continue;
        }
        let is_root_entity = mounted_on.is_none() && hardpoint.is_none() && player_tag.is_none();
        let is_local_controlled_entity = is_root_entity
            && authoritative_controlled_entity_id.as_deref() == Some(runtime_entity_id.as_str());
        let is_local_player_entity = runtime_entity_id == *local_player_entity_id;
        if is_local_player_entity
            && let Some(controlled_id) = resolve_authoritative_control_entity_id_with_snapshot(
                &entity_registry,
                &runtime_entity_id_by_guid,
                local_player_entity_id,
                controlled_entity_guid,
            )
        {
            player_view_state.controlled_entity_id = Some(controlled_id);
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
                    commands
                        .entity(entity)
                        .insert((ReplicatedAdoptionHandled, Visibility::Hidden))
                        .remove::<(
                            ControlledEntity,
                            StreamedVisualAssetId,
                            StreamedVisualAttached,
                            StreamedSpriteShaderAssetId,
                        )>();
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
                            StreamedVisualAssetId,
                            StreamedVisualAttached,
                            StreamedSpriteShaderAssetId,
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
            ReplicatedAdoptionHandled,
            Transform::default(),
            GlobalTransform::default(),
            WorldEntity,
            DespawnOnExit(ClientAppState::InWorld),
            Visibility::Visible,
            InheritedVisibility::default(),
            ViewVisibility::default(),
        ));

        if player_tag.is_none() {
            if let Some(visual_asset_id) = visual_asset_id {
                entity_commands.insert(StreamedVisualAssetId(visual_asset_id.0.clone()));
            } else {
                entity_commands.remove::<(StreamedVisualAssetId, StreamedVisualAttached)>();
            }
        } else {
            // Observer/player entities are camera anchors and must never render ship visuals.
            entity_commands.remove::<(
                StreamedVisualAssetId,
                StreamedVisualAttached,
                StreamedSpriteShaderAssetId,
            )>();
        }
        if player_tag.is_none()
            && let Some(sprite_shader_asset_id) = sprite_shader_asset_id
            && let Some(shader_asset_id) = sprite_shader_asset_id.0.as_ref()
        {
            entity_commands.insert(StreamedSpriteShaderAssetId(shader_asset_id.clone()));
        } else {
            entity_commands.remove::<StreamedSpriteShaderAssetId>();
        }

        if is_local_controlled_entity {
            let size = size_m.copied().unwrap_or_else(default_corvette_size);
            let mass_kg = total_mass_kg
                .map(|m| m.0)
                .filter(|m| *m > 0.0)
                .unwrap_or_else(default_corvette_mass_kg);
            let position = position.map(|p| p.0).unwrap_or(Vec2::ZERO);
            let rotation = rotation.copied().unwrap_or(Rotation::IDENTITY);
            let velocity = linear_velocity.map(|v| v.0).unwrap_or(Vec2::ZERO);
            entity_commands.insert((
                RigidBody::Dynamic,
                Collider::rectangle(size.width, size.length),
                Mass(mass_kg),
                angular_inertia_from_size(mass_kg, &size),
                Position(position),
                rotation,
                LinearVelocity(velocity),
                AngularVelocity::default(),
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
            // Remote/non-controlled roots must be receive-only on client.
            // Leaving local flight-authoring components here allows client-side
            // simulation to diverge from authoritative server state.
            entity_commands.remove::<(ActionQueue, FlightComputer)>();
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
            "multiple controlled entities detected under native replication; keeping latest control target"
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
    entity_registry: Res<'_, RuntimeEntityHierarchy>,
    player_query: Query<'_, '_, Option<&'_ ControlledEntityGuid>, With<PlayerTag>>,
) {
    let Some(local_player_entity_id) = session.player_entity_id.as_ref() else {
        return;
    };

    let Some(&local_player_entity) = entity_registry
        .by_entity_id
        .get(local_player_entity_id.as_str())
    else {
        return;
    };
    let Ok(controlled) = player_query.get(local_player_entity) else {
        return;
    };

    if let Some(authoritative_controlled_id) = resolve_authoritative_control_entity_id_from_registry(
        &entity_registry,
        local_player_entity_id,
        controlled,
    ) {
        player_view_state.controlled_entity_id = Some(authoritative_controlled_id);
        if player_view_state.desired_controlled_entity_id.is_none() {
            player_view_state.desired_controlled_entity_id =
                player_view_state.controlled_entity_id.clone();
        }
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

    // Route local input strictly to authoritative control.
    // If control points to a non-localized runtime entity (transient hydration/order gap),
    // keep existing tags this frame instead of falling back to player and breaking prediction ownership.
    let target_entity_id = match player_view_state.controlled_entity_id.as_ref() {
        Some(id) if entity_registry.by_entity_id.contains_key(id.as_str()) => Some(id),
        Some(_) => return,
        None => Some(local_player_entity_id),
    };
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

fn resolve_camera_anchor_entity(
    session: &ClientSession,
    _player_view_state: &LocalPlayerViewState,
    entity_registry: &RuntimeEntityHierarchy,
) -> Option<Entity> {
    let preferred_runtime_id = session
        .player_entity_id
        .as_ref()
        .filter(|id| entity_registry.by_entity_id.contains_key(id.as_str()))?;
    entity_registry
        .by_entity_id
        .get(preferred_runtime_id.as_str())
        .copied()
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

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn enforce_motion_ownership_for_world_entities(
    mut commands: Commands<'_, '_>,
    local_mode: Res<'_, LocalSimulationDebugMode>,
    proxy_tuning: Res<'_, NearbyCollisionProxyTuning>,
    session: Res<'_, ClientSession>,
    player_view_state: Res<'_, LocalPlayerViewState>,
    entity_registry: Res<'_, RuntimeEntityHierarchy>,
    root_world_entities: Query<
        '_,
        '_,
        (
            Entity,
            Option<&'_ ControlledEntity>,
            Option<&'_ MountedOn>,
            Option<&'_ Hardpoint>,
            Option<&'_ PlayerTag>,
            Option<&'_ EntityGuid>,
            Option<&'_ Position>,
            Option<&'_ Transform>,
            Option<&'_ Rotation>,
            Option<&'_ LinearVelocity>,
            Option<&'_ SizeM>,
            Option<&'_ TotalMassKg>,
            Has<ControlledEntityGuid>,
            Has<RigidBody>,
            Has<SuppressedPredictedDuplicateVisual>,
        ),
        (With<WorldEntity>, Without<Camera>),
    >,
) {
    let target_entity_id = match player_view_state.controlled_entity_id.as_ref() {
        Some(id) if entity_registry.by_entity_id.contains_key(id.as_str()) => Some(id),
        // Avoid destructive stripping during transient unresolved control mapping.
        Some(_) => return,
        None => session
            .player_entity_id
            .as_ref()
            .filter(|id| entity_registry.by_entity_id.contains_key(id.as_str())),
    };
    let target_entity =
        target_entity_id.and_then(|id| entity_registry.by_entity_id.get(id.as_str()).copied());

    let Some(target_entity) = target_entity else {
        // Control target not resolved yet (bootstrap/handoff). Avoid destructive stripping.
        return;
    };
    let mut target_guid: Option<uuid::Uuid> = None;
    for (
        entity,
        _,
        mounted_on,
        hardpoint,
        player_tag,
        guid,
        _,
        _,
        _,
        _,
        _,
        _,
        has_controlled_entity_guid,
        _,
        _,
    ) in &root_world_entities
    {
        let is_root_ship = mounted_on.is_none()
            && hardpoint.is_none()
            && player_tag.is_none()
            && guid.is_some()
            && !has_controlled_entity_guid;
        if entity == target_entity && is_root_ship {
            target_guid = guid.map(|guid| guid.0);
            break;
        }
    }

    let target_position = root_world_entities.iter().find_map(
        |(
            entity,
            _,
            mounted_on,
            hardpoint,
            player_tag,
            _,
            position,
            transform,
            _,
            _,
            _,
            _,
            _,
            _,
            _,
        )| {
            if entity != target_entity
                || mounted_on.is_some()
                || hardpoint.is_some()
                || player_tag.is_some()
            {
                return None;
            }
            position
                .map(|p| p.0)
                .or_else(|| transform.map(|t| t.translation.truncate()))
        },
    );
    let mut nearby_remote_candidates = Vec::<(Entity, f32)>::new();
    if let Some(target_position) = target_position {
        let max_dist_sq = proxy_tuning.radius_m * proxy_tuning.radius_m;
        for (
            entity,
            controlled,
            mounted_on,
            hardpoint,
            player_tag,
            guid,
            position,
            transform,
            _,
            _,
            _,
            _,
            _,
            has_controlled_entity_guid,
            is_suppressed,
        ) in &root_world_entities
        {
            let is_root_ship = mounted_on.is_none()
                && hardpoint.is_none()
                && player_tag.is_none()
                && guid.is_some()
                && !has_controlled_entity_guid;
            if !is_root_ship || controlled.is_some() || entity == target_entity || is_suppressed {
                continue;
            }
            if guid.is_some_and(|guid| Some(guid.0) == target_guid) {
                // Never keep a nearby proxy for logical duplicates of the locally controlled entity.
                // Duplicate local copies can collide and create client-only velocity drift.
                continue;
            }
            let Some(remote_pos) = position
                .map(|p| p.0)
                .or_else(|| transform.map(|t| t.translation.truncate()))
            else {
                continue;
            };
            let dist_sq = (remote_pos - target_position).length_squared();
            if dist_sq <= max_dist_sq {
                nearby_remote_candidates.push((entity, dist_sq));
            }
        }
    }
    nearby_remote_candidates.sort_by(|a, b| a.1.total_cmp(&b.1));
    let nearby_proxy_entities = nearby_remote_candidates
        .into_iter()
        .take(proxy_tuning.max_proxies)
        .map(|(entity, _)| entity)
        .collect::<HashSet<_>>();

    for (
        entity,
        controlled,
        mounted_on,
        hardpoint,
        player_tag,
        _guid,
        position,
        _transform,
        rotation,
        linear_velocity,
        size_m,
        total_mass_kg,
        has_controlled_entity_guid,
        has_rigidbody,
        is_suppressed,
    ) in &root_world_entities
    {
        let is_root_ship = mounted_on.is_none()
            && hardpoint.is_none()
            && player_tag.is_none()
            && _guid.is_some()
            && !has_controlled_entity_guid;
        if !is_root_ship {
            continue;
        }
        if is_suppressed {
            commands.entity(entity).remove::<NearbyCollisionProxy>();
            commands.entity(entity).remove::<(
                ActionQueue,
                FlightComputer,
                RigidBody,
                Collider,
                Mass,
                AngularInertia,
                LockedAxes,
                LinearDamping,
                AngularDamping,
            )>();
            if !local_mode.0 {
                commands
                    .entity(entity)
                    .insert(lightyear::prelude::Interpolated)
                    .remove::<lightyear::prelude::Predicted>();
            }
            continue;
        }

        if controlled.is_some() || entity == target_entity {
            if entity == target_entity {
                let size = size_m.copied().unwrap_or_else(default_corvette_size);
                let mass_kg = total_mass_kg
                    .map(|m| m.0)
                    .filter(|m| *m > 0.0)
                    .unwrap_or_else(default_corvette_mass_kg);
                let position = position.map(|p| p.0).unwrap_or(Vec2::ZERO);
                let rotation = rotation.copied().unwrap_or(Rotation::IDENTITY);
                let linear_velocity = linear_velocity.map(|v| v.0).unwrap_or(Vec2::ZERO);
                let mut entity_commands = commands.entity(entity);

                if !has_rigidbody {
                    entity_commands.insert((
                        RigidBody::Dynamic,
                        Collider::rectangle(size.width, size.length),
                        Mass(mass_kg),
                        angular_inertia_from_size(mass_kg, &size),
                        LinearDamping(0.0),
                        AngularDamping(0.0),
                    ));
                }
                entity_commands.insert((
                    Position(position),
                    rotation,
                    LinearVelocity(linear_velocity),
                ));
            }
            if !local_mode.0 {
                commands
                    .entity(entity)
                    .insert(lightyear::prelude::Predicted)
                    .remove::<lightyear::prelude::Interpolated>();
            }
            continue;
        }

        let keep_nearby_proxy = nearby_proxy_entities.contains(&entity);
        if keep_nearby_proxy {
            let size = size_m.copied().unwrap_or_else(default_corvette_size);
            let mut entity_commands = commands.entity(entity);
            if !has_rigidbody {
                entity_commands.insert(RigidBody::Kinematic);
            }
            entity_commands.insert(Collider::rectangle(size.width, size.length));
            entity_commands.insert(NearbyCollisionProxy);
            // Kinematic collision proxy should not carry local dynamic mass/inertia writers.
            entity_commands.remove::<(Mass, AngularInertia)>();
        } else {
            // Remote/non-controlled ships must remain receive-only on client every tick.
            // Replication may re-add these components after initial adoption.
            commands.entity(entity).remove::<NearbyCollisionProxy>();
            commands.entity(entity).remove::<(
                RigidBody,
                Collider,
                Mass,
                AngularInertia,
                LockedAxes,
                LinearDamping,
                AngularDamping,
            )>();
        }
        commands
            .entity(entity)
            .remove::<(ActionQueue, FlightComputer)>();
        if !local_mode.0 {
            commands
                .entity(entity)
                .insert(lightyear::prelude::Interpolated)
                .remove::<lightyear::prelude::Predicted>();
        }
    }
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn audit_motion_ownership_system(
    time: Res<'_, Time>,
    enabled: Res<'_, MotionOwnershipAuditEnabled>,
    local_mode: Res<'_, LocalSimulationDebugMode>,
    session: Res<'_, ClientSession>,
    player_view_state: Res<'_, LocalPlayerViewState>,
    entity_registry: Res<'_, RuntimeEntityHierarchy>,
    mut audit_state: ResMut<'_, MotionOwnershipAuditState>,
    roots: Query<
        '_,
        '_,
        (
            Entity,
            Option<&'_ Name>,
            Option<&'_ MountedOn>,
            Option<&'_ Hardpoint>,
            Option<&'_ PlayerTag>,
            Has<lightyear::prelude::Predicted>,
            Has<lightyear::prelude::Interpolated>,
            Has<ActionQueue>,
            Has<FlightComputer>,
            Has<RigidBody>,
            Has<NearbyCollisionProxy>,
            Has<Position>,
            Has<Rotation>,
            Has<LinearVelocity>,
        ),
        With<WorldEntity>,
    >,
) {
    if !enabled.0 {
        return;
    }
    let now_s = time.elapsed_secs_f64();
    if now_s - audit_state.last_logged_at_s < 0.5 {
        return;
    }
    audit_state.last_logged_at_s = now_s;

    let target_entity_id = match player_view_state.controlled_entity_id.as_ref() {
        Some(id) if entity_registry.by_entity_id.contains_key(id.as_str()) => Some(id),
        Some(_) => {
            warn!(
                controlled = ?player_view_state.controlled_entity_id,
                "motion audit: controlled entity unresolved in registry"
            );
            return;
        }
        None => session.player_entity_id.as_ref(),
    };
    let target_entity =
        target_entity_id.and_then(|id| entity_registry.by_entity_id.get(id.as_str()).copied());

    let mut anomalies = Vec::new();
    for (
        entity,
        name,
        mounted_on,
        hardpoint,
        player_tag,
        is_predicted,
        is_interpolated,
        has_action_queue,
        has_flight_computer,
        has_rigidbody,
        has_nearby_proxy,
        has_position,
        has_rotation,
        has_linear_velocity,
    ) in &roots
    {
        let is_root_ship = mounted_on.is_none() && hardpoint.is_none() && player_tag.is_none();
        if !is_root_ship {
            continue;
        }
        let entity_name = name
            .map(|n| n.as_str().to_string())
            .unwrap_or_else(|| format!("<entity:{entity:?}>"));
        let is_target = Some(entity) == target_entity;

        if is_target && !local_mode.0 {
            if !is_predicted || is_interpolated {
                anomalies.push(format!(
                    "{} target markers invalid predicted={} interpolated={}",
                    entity_name, is_predicted, is_interpolated
                ));
            }
            if !has_rigidbody || !has_position || !has_rotation || !has_linear_velocity {
                anomalies.push(format!(
                    "{} target motion components missing rb={} pos={} rot={} vel={}",
                    entity_name, has_rigidbody, has_position, has_rotation, has_linear_velocity
                ));
            }
            continue;
        }

        if is_predicted
            || has_action_queue
            || has_flight_computer
            || (has_rigidbody && !has_nearby_proxy)
        {
            anomalies.push(format!(
                "{} remote writers present predicted={} action_queue={} flight_computer={} rb={} nearby_proxy={}",
                entity_name,
                is_predicted,
                has_action_queue,
                has_flight_computer,
                has_rigidbody,
                has_nearby_proxy
            ));
        }
    }

    if !anomalies.is_empty() {
        warn!(
            "motion ownership audit anomalies ({}): {}",
            anomalies.len(),
            anomalies.join(" | ")
        );
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
        if let Some(mut pos) = position
            && !pos.0.is_finite()
        {
            pos.0 = Vec2::ZERO;
        }
        if let Some(mut vel) = velocity
            && !vel.0.is_finite()
        {
            vel.0 = Vec2::ZERO;
        }
        if let Some(mut ang_vel) = angular_velocity
            && !ang_vel.0.is_finite()
        {
            ang_vel.0 = 0.0;
        }
        if !transform.translation.is_finite() {
            transform.translation = Vec3::ZERO;
        }
        let mut heading = if let Some(rot) = rotation.as_ref() {
            if rot.is_finite() {
                rot.as_radians()
            } else {
                0.0
            }
        } else if transform.rotation.is_finite() {
            transform.rotation.to_euler(EulerRot::ZYX).2
        } else {
            0.0
        };
        if !heading.is_finite() {
            heading = 0.0;
        }
        let planar_rot = Quat::from_rotation_z(heading);
        if let Some(mut rot) = rotation {
            *rot = Rotation::radians(heading);
        }
        transform.translation.z = 0.0;
        transform.rotation = planar_rot;
    }
}

#[allow(clippy::type_complexity)]
fn reconcile_controlled_prediction_with_confirmed(
    mut controlled_query: Query<
        '_,
        '_,
        (
            &mut Position,
            &mut Rotation,
            Option<&mut LinearVelocity>,
            Option<&mut Transform>,
            Option<&lightyear::prelude::Confirmed<Position>>,
            Option<&lightyear::prelude::Confirmed<Rotation>>,
            Option<&lightyear::prelude::Confirmed<LinearVelocity>>,
        ),
        (With<ControlledEntity>, With<lightyear::prelude::Predicted>),
    >,
) {
    const SNAP_POS_ERROR_M: f32 = 64.0;
    const SMOOTH_POS_ERROR_M: f32 = 2.0;
    const SMOOTH_FACTOR: f32 = 0.25;
    const SNAP_ROT_ERROR_RAD: f32 = 0.8;
    const SMOOTH_ROT_ERROR_RAD: f32 = 0.08;

    for (
        mut position,
        mut rotation,
        mut linear_velocity,
        transform,
        confirmed_position,
        confirmed_rotation,
        confirmed_linear_velocity,
    ) in &mut controlled_query
    {
        let Some(confirmed_position) = confirmed_position else {
            continue;
        };

        let confirmed_pos = confirmed_position.0.0;
        let pos_error = confirmed_pos - position.0;
        let pos_error_len = pos_error.length();
        if pos_error_len >= SNAP_POS_ERROR_M {
            position.0 = confirmed_pos;
            if let Some(velocity) = linear_velocity.as_mut() {
                velocity.0 = confirmed_linear_velocity.map_or(Vec2::ZERO, |v| v.0.0);
            }
        } else if pos_error_len >= SMOOTH_POS_ERROR_M {
            position.0 += pos_error * SMOOTH_FACTOR;
        }

        if let Some(velocity) = linear_velocity.as_mut()
            && let Some(confirmed_vel) = confirmed_linear_velocity
        {
            let confirmed = confirmed_vel.0.0;
            let vel_error = (confirmed - velocity.0).length();
            if pos_error_len >= SNAP_POS_ERROR_M || vel_error >= 2.0 {
                velocity.0 = confirmed;
            } else {
                velocity.0 = velocity.0.lerp(confirmed, 0.35);
            }
            if confirmed.length_squared() <= 1.0e-6 && velocity.0.length_squared() <= 1.0e-4 {
                velocity.0 = Vec2::ZERO;
            }
        }

        if let Some(confirmed_rotation) = confirmed_rotation {
            let confirmed_rot = confirmed_rotation.0;
            let rot_error = rotation.angle_between(confirmed_rot);
            if rot_error >= SNAP_ROT_ERROR_RAD {
                *rotation = confirmed_rot;
            } else if rot_error >= SMOOTH_ROT_ERROR_RAD {
                *rotation = rotation.slerp(confirmed_rot, SMOOTH_FACTOR);
            }
        }

        if let Some(mut transform) = transform {
            transform.translation.x = position.0.x;
            transform.translation.y = position.0.y;
            transform.rotation = (*rotation).into();
            transform.translation.z = 0.0;
        }
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
    player_view_state: Res<'_, LocalPlayerViewState>,
    entity_registry: Res<'_, RuntimeEntityHierarchy>,
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
    let local_controlled_entity =
        player_view_state
            .controlled_entity_id
            .as_ref()
            .and_then(|runtime_id| {
                entity_registry
                    .by_entity_id
                    .get(runtime_id.as_str())
                    .copied()
            });
    const VELOCITY_ARROW_SCALE: f32 = 0.5;
    const HARDPOINT_CROSS_HALF_SIZE: f32 = 2.0;
    let collision_color = Color::srgb(0.2, 0.8, 0.2);
    let velocity_color = Color::srgb(0.2, 0.5, 1.0);
    let hardpoint_color = Color::srgb(1.0, 0.8, 0.2);
    let controlled_predicted_color = Color::srgb(0.2, 1.0, 1.0);
    let controlled_confirmed_color = Color::srgb(1.0, 0.2, 1.0);
    let prediction_error_color = Color::srgb(1.0, 0.2, 0.2);
    let visibility_range_color = Color::srgb(0.9, 0.9, 0.15);
    let mut controlled_visibility_circle: Option<(Vec3, f32)> = None;

    for (
        entity,
        transform,
        size_m,
        linear_velocity,
        mounted_on,
        hardpoint,
        controlled_marker,
        scanner_range,
        _entity_guid,
        confirmed_position,
        confirmed_rotation,
        _is_predicted,
        _is_replicated,
        _is_interpolated,
    ) in &entities
    {
        let pos = transform.translation;
        let rot = transform.rotation;
        let half_extents =
            size_m.map(|size| Vec3::new(size.width * 0.5, size.length * 0.5, size.height * 0.5));

        let is_local_controlled = (mounted_on.is_none()
            && hardpoint.is_none()
            && Some(entity) == local_controlled_entity)
            // keep fallback for brief handoff frames where registry may lag one frame
            || controlled_marker.is_some_and(|controlled| {
                session
                    .player_entity_id
                    .as_deref()
                    .is_some_and(|player_id| controlled.player_entity_id == player_id)
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
                let confirmed_rot: Quat = confirmed_rotation.0.into();
                let confirmed_pos = confirmed_position.0.0.extend(0.0);
                let confirmed_transform =
                    Transform::from_translation(confirmed_pos).with_rotation(confirmed_rot);
                gizmos.aabb_3d(aabb, confirmed_transform, controlled_confirmed_color);
                gizmos.line(pos, confirmed_pos, prediction_error_color);
            }
        }

        if mounted_on.is_none() && hardpoint.is_none() && is_local_controlled {
            // Expected client visibility range circle.
            // Fallback to 300m when scanner range component is unavailable.
            let range_m = scanner_range
                .map(|r| r.0.max(0.0))
                .unwrap_or(300.0)
                .max(1.0);
            controlled_visibility_circle = Some((pos, range_m));
        }

        if mounted_on.is_none()
            && let Some(vel) = linear_velocity
        {
            let len = vel.0.length();
            if len > 0.01 {
                let end = pos + vel.0.extend(0.0) * VELOCITY_ARROW_SCALE;
                gizmos.arrow(pos, end, velocity_color);
            }
        }

        if hardpoint.is_some() {
            let isometry = bevy::math::Isometry3d::new(pos, rot);
            gizmos.cross(isometry, HARDPOINT_CROSS_HALF_SIZE, hardpoint_color);
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

    #[test]
    fn realtime_input_send_policy_sends_on_input_or_target_change() {
        assert!(input::should_send_realtime_input_message(10.0, 9.95, true, false));
        assert!(input::should_send_realtime_input_message(10.0, 9.95, false, true));
    }

    #[test]
    fn realtime_input_send_policy_sends_heartbeat_when_idle() {
        assert!(input::should_send_realtime_input_message(10.0, 9.89, false, false));
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

        let resolved = resolve_camera_anchor_entity(&session, &player_view_state, &registry);
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

        let resolved = resolve_camera_anchor_entity(&session, &player_view_state, &registry);
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
