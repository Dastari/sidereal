use bevy::log::info;
use bevy::prelude::*;
use sidereal_game::process_character_movement_actions;

use super::app_state::ClientAppState;
use super::camera::{
    audit_active_world_cameras_system, gate_gameplay_camera_system, gate_menu_camera_system,
    sync_ui_overlay_camera_to_gameplay_camera_system, update_camera_motion_state,
    update_topdown_camera_system,
};
use super::components::{WeaponTracerCooldowns, WeaponTracerPool};
use super::debug_overlay::{
    draw_debug_overlay_system, log_prediction_runtime_state, toggle_debug_overlay_system,
    update_debug_fps_text_system,
};
use super::motion::{apply_predicted_input_to_action_queue, enforce_controlled_planar_motion};
use super::resources::LogoutCleanupRequested;
use super::transforms::{
    lock_camera_to_player_entity_end_of_frame,
    lock_player_entity_to_controlled_entity_end_of_frame,
    sync_remote_controlled_ship_roots_from_player_anchors,
    sync_world_entity_transforms_from_physics,
};
use super::{
    assets, audio, auth_net, auth_ui, bootstrap, control, dialog_ui, input, logout, replication,
    scene, scene_world, transport, ui, visuals,
};

pub(super) struct ClientBootstrapPlugin {
    pub(super) headless: bool,
}

impl Plugin for ClientBootstrapPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(replication::ensure_parent_spatial_components_on_children_added);
        app.add_systems(Update, replication::ensure_ui_node_spatial_components);
        if self.headless {
            app.add_systems(Startup, auth_net::configure_headless_session_from_env);
            app.add_systems(Startup, transport::start_lightyear_client_transport);
            app.add_systems(Startup, || {
                info!("sidereal-client headless transport mode");
            });
            return;
        }

        scene::insert_embedded_fonts(app);
        auth_net::init_gateway_request_state(app);
        app.init_state::<ClientAppState>();
        app.add_systems(
            OnEnter(ClientAppState::Auth),
            logout::purge_stale_world_and_transport_on_enter_auth_system,
        );
        auth_ui::register_auth_ui(app);
        dialog_ui::register_dialog_ui(app);
        app.add_systems(Startup, scene::spawn_ui_overlay_camera);
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
        app.add_systems(
            OnEnter(ClientAppState::InWorld),
            (
                transport::ensure_lightyear_client_system,
                scene_world::spawn_world_scene,
                bootstrap::reset_bootstrap_watchdog_on_enter_in_world,
            )
                .chain(),
        );
    }
}

pub(super) struct ClientTransportPlugin {
    pub(super) headless: bool,
}

impl Plugin for ClientTransportPlugin {
    fn build(&self, app: &mut App) {
        if self.headless {
            app.add_systems(
                Update,
                (
                    auth_net::apply_headless_account_switch_system,
                    transport::ensure_client_transport_channels,
                    auth_net::send_lightyear_auth_messages,
                ),
            );
        } else {
            app.add_systems(
                Update,
                (
                    transport::ensure_client_transport_channels,
                    auth_net::send_lightyear_auth_messages,
                    auth_net::receive_lightyear_session_ready_messages,
                    auth_net::receive_lightyear_session_denied_messages,
                ),
            );
        }
    }
}

pub(super) struct ClientReplicationPlugin {
    pub(super) headless: bool,
}

impl Plugin for ClientReplicationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PreUpdate,
            (
                replication::ensure_replicated_entity_spatial_components,
                replication::ensure_hierarchy_parent_spatial_components
                    .after(replication::ensure_replicated_entity_spatial_components),
            ),
        );
        app.add_systems(
            PostUpdate,
            replication::ensure_hierarchy_parent_spatial_components
                .after(sidereal_game::sync_mounted_hierarchy)
                .before(bevy::transform::TransformSystems::Propagate),
        );
        if self.headless {
            app.add_systems(
                Update,
                (
                    replication::configure_prediction_manager_tuning,
                    assets::receive_lightyear_asset_stream_messages,
                    assets::ensure_critical_assets_available_system
                        .after(assets::receive_lightyear_asset_stream_messages),
                    replication::adopt_native_lightyear_replicated_entities,
                    sync_remote_controlled_ship_roots_from_player_anchors
                        .after(replication::adopt_native_lightyear_replicated_entities),
                    sync_world_entity_transforms_from_physics
                        .after(sync_remote_controlled_ship_roots_from_player_anchors),
                    replication::sync_local_player_view_state_system
                        .after(replication::adopt_native_lightyear_replicated_entities),
                    replication::sync_controlled_entity_tags_system
                        .after(replication::sync_local_player_view_state_system),
                    replication::converge_local_prediction_markers_system
                        .after(replication::sync_controlled_entity_tags_system),
                    control::send_lightyear_control_requests
                        .after(replication::converge_local_prediction_markers_system),
                    control::receive_lightyear_control_results
                        .after(control::send_lightyear_control_requests),
                    control::log_client_control_state_changes
                        .after(control::receive_lightyear_control_results),
                    log_prediction_runtime_state,
                ),
            );
        } else {
            app.add_systems(
                Update,
                (
                    replication::configure_prediction_manager_tuning,
                    assets::receive_lightyear_asset_stream_messages,
                    assets::ensure_critical_assets_available_system
                        .after(assets::receive_lightyear_asset_stream_messages),
                    replication::adopt_native_lightyear_replicated_entities,
                    sync_remote_controlled_ship_roots_from_player_anchors
                        .after(replication::adopt_native_lightyear_replicated_entities),
                    sync_world_entity_transforms_from_physics
                        .after(sync_remote_controlled_ship_roots_from_player_anchors),
                    replication::transition_world_loading_to_in_world
                        .after(replication::adopt_native_lightyear_replicated_entities),
                    replication::sync_local_player_view_state_system
                        .after(replication::adopt_native_lightyear_replicated_entities),
                    replication::sync_controlled_entity_tags_system
                        .after(replication::sync_local_player_view_state_system),
                    replication::converge_local_prediction_markers_system
                        .after(replication::sync_controlled_entity_tags_system),
                    control::send_lightyear_control_requests
                        .after(replication::converge_local_prediction_markers_system),
                    control::receive_lightyear_control_results
                        .after(control::send_lightyear_control_requests),
                    control::log_client_control_state_changes
                        .after(control::receive_lightyear_control_results),
                    log_prediction_runtime_state,
                ),
            );
        }
    }
}

pub(super) struct ClientPredictionPlugin {
    pub(super) headless: bool,
}

impl Plugin for ClientPredictionPlugin {
    fn build(&self, app: &mut App) {
        let send_input = (
            input::enforce_single_input_marker_owner.before(input::send_lightyear_input_messages),
            input::send_lightyear_input_messages,
            bevy::ecs::schedule::ApplyDeferred,
        )
            .chain()
            .in_set(lightyear::prelude::client::input::InputSystems::WriteClientInputs);
        if self.headless {
            app.add_systems(FixedPreUpdate, send_input);
        } else {
            app.add_systems(
                FixedPreUpdate,
                send_input.run_if(in_state(ClientAppState::InWorld)),
            );
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
    }
}

pub(super) struct ClientVisualsPlugin;

impl Plugin for ClientVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WeaponTracerPool>();
        app.init_resource::<WeaponTracerCooldowns>();
        app.add_systems(
            Update,
            (
                visuals::ensure_fullscreen_layer_fallback_system
                    .after(replication::adopt_native_lightyear_replicated_entities),
                visuals::suppress_duplicate_predicted_interpolated_visuals_system
                    .after(replication::adopt_native_lightyear_replicated_entities),
                visuals::cleanup_streamed_visual_children_system
                    .after(visuals::suppress_duplicate_predicted_interpolated_visuals_system),
                visuals::attach_thruster_plume_visuals_system
                    .after(visuals::suppress_duplicate_predicted_interpolated_visuals_system),
                visuals::update_thruster_plume_visuals_system
                    .after(visuals::attach_thruster_plume_visuals_system),
                visuals::ensure_weapon_tracer_pool_system
                    .after(visuals::suppress_duplicate_predicted_interpolated_visuals_system),
                visuals::emit_weapon_tracer_visuals_system
                    .after(visuals::ensure_weapon_tracer_pool_system),
                visuals::receive_remote_weapon_tracer_messages_system
                    .after(visuals::ensure_weapon_tracer_pool_system),
                visuals::update_weapon_tracer_visuals_system
                    .after(visuals::emit_weapon_tracer_visuals_system)
                    .after(visuals::receive_remote_weapon_tracer_messages_system),
                visuals::update_weapon_impact_sparks_system
                    .after(visuals::update_weapon_tracer_visuals_system),
                visuals::attach_streamed_visual_assets_system
                    .after(assets::receive_lightyear_asset_stream_messages),
                visuals::sync_fullscreen_layer_renderables_system
                    .after(replication::adopt_native_lightyear_replicated_entities),
                visuals::sync_backdrop_fullscreen_system
                    .after(visuals::sync_fullscreen_layer_renderables_system),
            )
                .run_if(in_state(ClientAppState::InWorld)),
        );
    }
}

pub(super) struct ClientUiPlugin;

impl Plugin for ClientUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(ClientAppState::Auth),
            audio::start_menu_loop_music_system,
        );
        app.add_systems(
            OnEnter(ClientAppState::WorldLoading),
            audio::start_menu_loop_music_system,
        );
        app.add_systems(
            OnEnter(ClientAppState::InWorld),
            audio::stop_menu_loop_music_system,
        );
        app.add_systems(Update, scene::handle_character_select_buttons);
        app.add_systems(Update, auth_net::poll_gateway_request_results);
        app.add_systems(
            Update,
            gate_menu_camera_system.run_if(not(in_state(ClientAppState::InWorld))),
        );
        app.add_systems(
            Update,
            (
                gate_gameplay_camera_system,
                ui::update_owned_entities_panel_system,
                ui::handle_owned_entities_panel_buttons,
                ui::update_loading_overlay_system,
                ui::update_runtime_stream_icon_system,
                bootstrap::watch_in_world_bootstrap_failures,
                update_topdown_camera_system,
                sync_ui_overlay_camera_to_gameplay_camera_system
                    .after(update_topdown_camera_system),
                update_camera_motion_state.after(update_topdown_camera_system),
                ui::propagate_ui_overlay_layer_system,
                ui::update_hud_system,
                ui::sync_ship_nameplates_system,
                toggle_debug_overlay_system,
                update_debug_fps_text_system,
            )
                .run_if(in_state(ClientAppState::InWorld)),
        );
        app.add_systems(
            Last,
            (
                lock_player_entity_to_controlled_entity_end_of_frame,
                lock_camera_to_player_entity_end_of_frame
                    .after(lock_player_entity_to_controlled_entity_end_of_frame),
                super::backdrop::compute_fullscreen_external_world_system
                    .after(lock_camera_to_player_entity_end_of_frame),
                super::backdrop::update_starfield_material_system
                    .after(super::backdrop::compute_fullscreen_external_world_system),
                super::backdrop::update_space_background_material_system
                    .after(super::backdrop::update_starfield_material_system),
                ui::update_ship_nameplate_positions_system
                    .after(super::backdrop::update_space_background_material_system),
                ui::update_segmented_bars_system.after(ui::update_ship_nameplate_positions_system),
                draw_debug_overlay_system.after(ui::update_segmented_bars_system),
            )
                .chain()
                .run_if(in_state(ClientAppState::InWorld)),
        );
        app.add_systems(
            Update,
            audit_active_world_cameras_system.run_if(in_state(ClientAppState::InWorld)),
        );

        #[cfg(not(target_arch = "wasm32"))]
        let logout_chain = (
            logout::request_logout_system.run_if(in_state(ClientAppState::InWorld)),
            logout::request_logout_system.run_if(in_state(ClientAppState::WorldLoading)),
            logout::request_logout_system.run_if(in_state(ClientAppState::CharacterSelect)),
            logout::request_logout_on_window_close_system.run_if(in_state(ClientAppState::InWorld)),
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
}

pub(super) struct ClientDiagnosticsPlugin;

impl Plugin for ClientDiagnosticsPlugin {
    fn build(&self, _app: &mut App) {}
}
