use bevy::log::info;
use bevy::prelude::*;
use lightyear::frame_interpolation::FrameInterpolationSystems;
use lightyear::prelude::RollbackSystems;
use sidereal_game::process_character_movement_actions;

use super::app_state::ClientAppState;
use super::camera::{
    audit_active_world_cameras_system, gate_gameplay_camera_system, gate_menu_camera_system,
    sync_debug_overlay_camera_to_gameplay_camera_system,
    sync_planet_body_camera_to_gameplay_camera_system,
    sync_ui_overlay_camera_to_gameplay_camera_system, update_camera_motion_state,
    update_topdown_camera_system,
};
use super::components::{WeaponImpactSparkPool, WeaponTracerCooldowns, WeaponTracerPool};
use super::debug_overlay::{
    audit_prediction_entity_lifecycle, collect_debug_overlay_snapshot_system,
    debug_overlay_enabled, draw_debug_overlay_system, log_prediction_runtime_state,
    sync_debug_velocity_arrow_mesh_system, toggle_debug_overlay_system,
};
use super::motion::{apply_predicted_input_to_action_queue, enforce_controlled_planar_motion};
use super::resources::LogoutCleanupRequested;
use super::{
    asset_loading_ui, assets, audio, auth_net, auth_ui, backdrop, bootstrap, control, dev_console,
    dialog_ui, input, lighting, logout, owner_manifest, pause_menu, render_layers, replication,
    scene, scene_world, tactical, transforms, transport, ui, visuals, world_loading_ui,
};

fn env_flag(name: &str) -> bool {
    std::env::var(name)
        .ok()
        .map(|v| v.trim().to_string())
        .is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"))
}

pub(super) struct ClientBootstrapPlugin {
    pub(super) headless: bool,
}

impl Plugin for ClientBootstrapPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(replication::ensure_parent_spatial_components_on_children_added);
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
                bootstrap::reset_bootstrap_watchdog_on_enter_world_loading,
                world_loading_ui::setup_world_loading_screen,
            )
                .chain(),
        );
        app.add_systems(
            OnEnter(ClientAppState::AssetLoading),
            asset_loading_ui::setup_asset_loading_screen,
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
                )
                    .chain(),
            );
        } else {
            app.add_systems(
                Update,
                (
                    transport::ensure_client_transport_channels,
                    transport::handle_unexpected_server_disconnect_system,
                    auth_net::send_lightyear_auth_messages,
                    auth_net::receive_lightyear_session_ready_messages,
                    auth_net::receive_lightyear_session_denied_messages,
                    auth_net::watch_session_ready_timeout_system,
                )
                    .chain(),
            );
        }
    }
}

pub(super) struct ClientReplicationPlugin {
    pub(super) headless: bool,
}

impl Plugin for ClientReplicationPlugin {
    fn build(&self, app: &mut App) {
        let disable_runtime_asset_fetch = env_flag("SIDEREAL_CLIENT_DISABLE_RUNTIME_ASSET_FETCH");
        let disable_adoption = env_flag("SIDEREAL_CLIENT_DISABLE_REPLICATION_ADOPTION");
        if disable_runtime_asset_fetch {
            info!(
                "client runtime asset fetch systems disabled via SIDEREAL_CLIENT_DISABLE_RUNTIME_ASSET_FETCH"
            );
        }
        if disable_adoption {
            info!(
                "client replication adoption disabled via SIDEREAL_CLIENT_DISABLE_REPLICATION_ADOPTION"
            );
        }
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
            (
                replication::ensure_hierarchy_parent_spatial_components
                    .after(sidereal_game::sync_mounted_hierarchy),
                backdrop::detach_fullscreen_layer_hierarchy_links_system
                    .after(replication::ensure_hierarchy_parent_spatial_components),
                replication::sanitize_invalid_childof_hierarchy_links
                    .after(backdrop::detach_fullscreen_layer_hierarchy_links_system),
            )
                .before(bevy::transform::TransformSystems::Propagate),
        );
        if self.headless {
            app.add_systems(
                Update,
                (
                    replication::ensure_prediction_manager_present_system,
                    replication::configure_prediction_manager_tuning,
                    replication::prune_runtime_entity_registry_system,
                    (
                        assets::sync_runtime_asset_dependency_state_system
                            .after(replication::prune_runtime_entity_registry_system),
                        assets::queue_missing_catalog_assets_system
                            .after(assets::sync_runtime_asset_dependency_state_system)
                            .run_if(move || !disable_runtime_asset_fetch),
                        assets::poll_runtime_asset_http_fetches_system
                            .after(assets::queue_missing_catalog_assets_system)
                            .run_if(move || !disable_runtime_asset_fetch),
                    ),
                    replication::adopt_native_lightyear_replicated_entities
                        .after(replication::prune_runtime_entity_registry_system)
                        .run_if(move || !disable_adoption),
                    transforms::sync_frame_interpolation_markers_for_world_entities
                        .after(replication::adopt_native_lightyear_replicated_entities),
                    transforms::sync_confirmed_world_entity_transforms_from_physics
                        .after(transforms::sync_frame_interpolation_markers_for_world_entities),
                    transforms::sync_confirmed_world_entity_transforms_from_world_space
                        .after(transforms::sync_confirmed_world_entity_transforms_from_physics),
                    transforms::sync_interpolated_world_entity_transforms_without_history
                        .after(transforms::sync_confirmed_world_entity_transforms_from_world_space),
                    transforms::reveal_world_entities_when_initial_transform_ready.after(
                        transforms::sync_interpolated_world_entity_transforms_without_history,
                    ),
                    (
                        replication::sync_local_player_view_state_system.after(
                            transforms::sync_confirmed_world_entity_transforms_from_world_space,
                        ),
                        replication::sanitize_conflicting_prediction_interpolation_markers_system
                            .after(replication::sync_local_player_view_state_system),
                        replication::sync_controlled_entity_tags_system.after(
                            replication::sanitize_conflicting_prediction_interpolation_markers_system,
                        ),
                    ),
                    control::send_local_view_mode_updates
                        .after(replication::sync_local_player_view_state_system),
                    control::send_lightyear_control_requests
                        .after(replication::sync_controlled_entity_tags_system)
                        .after(control::send_local_view_mode_updates),
                    control::receive_lightyear_control_results
                        .after(control::send_lightyear_control_requests),
                    control::audit_client_control_handover_resolution
                        .after(control::receive_lightyear_control_results),
                    assets::receive_asset_catalog_version_messages
                        .after(control::audit_client_control_handover_resolution),
                    owner_manifest::receive_owner_asset_manifest_messages
                        .after(assets::receive_asset_catalog_version_messages),
                    tactical::receive_tactical_snapshot_messages
                        .after(owner_manifest::receive_owner_asset_manifest_messages),
                    control::log_client_control_state_changes
                        .after(tactical::receive_tactical_snapshot_messages),
                ),
            );
            app.add_systems(Update, log_prediction_runtime_state);
        } else {
            app.add_systems(
                Update,
                (
                    replication::ensure_prediction_manager_present_system,
                    replication::configure_prediction_manager_tuning,
                    replication::prune_runtime_entity_registry_system,
                    (
                        assets::sync_runtime_asset_dependency_state_system
                            .after(replication::prune_runtime_entity_registry_system),
                        assets::queue_missing_catalog_assets_system
                            .after(assets::sync_runtime_asset_dependency_state_system)
                            .run_if(move || !disable_runtime_asset_fetch),
                        assets::poll_runtime_asset_http_fetches_system
                            .after(assets::queue_missing_catalog_assets_system)
                            .run_if(move || !disable_runtime_asset_fetch),
                    ),
                    replication::adopt_native_lightyear_replicated_entities
                        .after(replication::prune_runtime_entity_registry_system)
                        .run_if(move || !disable_adoption),
                    transforms::sync_frame_interpolation_markers_for_world_entities
                        .after(replication::adopt_native_lightyear_replicated_entities),
                    transforms::sync_confirmed_world_entity_transforms_from_physics
                        .after(transforms::sync_frame_interpolation_markers_for_world_entities),
                    transforms::sync_confirmed_world_entity_transforms_from_world_space
                        .after(transforms::sync_confirmed_world_entity_transforms_from_physics),
                    transforms::sync_interpolated_world_entity_transforms_without_history
                        .after(transforms::sync_confirmed_world_entity_transforms_from_world_space),
                    transforms::reveal_world_entities_when_initial_transform_ready.after(
                        transforms::sync_interpolated_world_entity_transforms_without_history,
                    ),
                    replication::transition_world_loading_to_in_world
                        .after(transforms::sync_confirmed_world_entity_transforms_from_world_space),
                    replication::transition_asset_loading_to_in_world
                        .after(replication::transition_world_loading_to_in_world),
                ),
            );
            app.add_systems(
                Update,
                (
                    (
                        replication::sync_local_player_view_state_system.after(
                            transforms::sync_confirmed_world_entity_transforms_from_world_space,
                        ),
                        replication::sanitize_conflicting_prediction_interpolation_markers_system
                            .after(replication::sync_local_player_view_state_system),
                        replication::sync_controlled_entity_tags_system.after(
                            replication::sanitize_conflicting_prediction_interpolation_markers_system,
                        ),
                    ),
                    control::send_local_view_mode_updates
                        .after(replication::sync_local_player_view_state_system),
                    control::send_lightyear_control_requests
                        .after(replication::sync_controlled_entity_tags_system)
                        .after(control::send_local_view_mode_updates),
                    control::receive_lightyear_control_results
                        .after(control::send_lightyear_control_requests),
                    control::audit_client_control_handover_resolution
                        .after(control::receive_lightyear_control_results),
                    assets::receive_asset_catalog_version_messages
                        .after(control::audit_client_control_handover_resolution),
                    owner_manifest::receive_owner_asset_manifest_messages
                        .after(assets::receive_asset_catalog_version_messages),
                    tactical::receive_tactical_snapshot_messages
                        .after(owner_manifest::receive_owner_asset_manifest_messages),
                    control::log_client_control_state_changes
                        .after(tactical::receive_tactical_snapshot_messages),
                ),
            );
            app.add_systems(
                Update,
                log_prediction_runtime_state.run_if(in_state(ClientAppState::InWorld)),
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
            input::enforce_single_input_marker_owner,
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
        app.init_resource::<WeaponImpactSparkPool>();
        app.init_resource::<super::resources::RuntimeRenderLayerRegistry>();
        app.init_resource::<super::resources::RuntimeRenderLayerRegistryState>();
        app.init_resource::<super::resources::RuntimeRenderLayerAssignmentCache>();
        app.init_resource::<super::resources::RenderLayerPerfCounters>();
        app.init_resource::<super::resources::RuntimeSharedQuadMesh>();
        app.init_resource::<super::resources::DuplicateVisualResolutionState>();
        app.init_resource::<backdrop::FullscreenRenderCache>();
        app.init_resource::<backdrop::BackdropRenderPerfCounters>();
        let in_world_visuals_core = (
            super::shaders::sync_runtime_shader_assignments_system
                .after(replication::adopt_native_lightyear_replicated_entities),
            render_layers::sync_runtime_render_layer_registry_system
                .after(replication::adopt_native_lightyear_replicated_entities),
            render_layers::resolve_runtime_render_layer_assignments_system
                .after(render_layers::sync_runtime_render_layer_registry_system),
            visuals::suppress_duplicate_predicted_interpolated_visuals_system
                .after(replication::adopt_native_lightyear_replicated_entities),
            visuals::cleanup_streamed_visual_children_system
                .after(visuals::suppress_duplicate_predicted_interpolated_visuals_system),
            visuals::cleanup_planet_body_visual_children_system
                .after(visuals::suppress_duplicate_predicted_interpolated_visuals_system),
            visuals::attach_planet_visual_stack_system
                .after(visuals::cleanup_planet_body_visual_children_system),
            visuals::ensure_planet_body_root_visibility_system
                .after(visuals::attach_planet_visual_stack_system),
            visuals::attach_ballistic_projectile_visuals_system
                .after(visuals::suppress_duplicate_predicted_interpolated_visuals_system),
            visuals::attach_thruster_plume_visuals_system
                .after(visuals::suppress_duplicate_predicted_interpolated_visuals_system),
        );
        let in_world_visuals_effects = (
            visuals::update_thruster_plume_visuals_system
                .after(visuals::attach_thruster_plume_visuals_system),
            visuals::ensure_weapon_tracer_pool_system
                .after(visuals::suppress_duplicate_predicted_interpolated_visuals_system),
            visuals::ensure_weapon_impact_spark_pool_system
                .after(visuals::suppress_duplicate_predicted_interpolated_visuals_system),
            visuals::emit_weapon_tracer_visuals_system
                .after(visuals::ensure_weapon_tracer_pool_system),
            visuals::receive_remote_weapon_tracer_messages_system
                .after(visuals::ensure_weapon_tracer_pool_system),
            visuals::update_weapon_tracer_visuals_system
                .after(visuals::emit_weapon_tracer_visuals_system)
                .after(visuals::receive_remote_weapon_tracer_messages_system)
                .after(visuals::ensure_weapon_impact_spark_pool_system),
            visuals::update_weapon_impact_sparks_system
                .after(visuals::update_weapon_tracer_visuals_system),
            visuals::attach_streamed_visual_assets_system
                .after(assets::poll_runtime_asset_http_fetches_system)
                .after(render_layers::resolve_runtime_render_layer_assignments_system),
            visuals::update_entity_visibility_fade_in_system
                .after(visuals::attach_streamed_visual_assets_system)
                .after(visuals::ensure_planet_body_root_visibility_system),
        );
        let in_world_backdrop = (
            backdrop::sync_fullscreen_layer_renderables_system
                .after(replication::adopt_native_lightyear_replicated_entities),
            backdrop::sync_runtime_post_process_renderables_system
                .after(replication::adopt_native_lightyear_replicated_entities),
            backdrop::sync_backdrop_camera_system
                .after(backdrop::sync_fullscreen_layer_renderables_system)
                .after(backdrop::sync_runtime_post_process_renderables_system),
            backdrop::sync_backdrop_fullscreen_system.after(backdrop::sync_backdrop_camera_system),
        );
        app.add_systems(
            Update,
            in_world_visuals_core.run_if(in_state(ClientAppState::InWorld)),
        );
        app.add_systems(
            Update,
            in_world_visuals_effects.run_if(in_state(ClientAppState::InWorld)),
        );
        app.add_systems(
            Update,
            in_world_backdrop.run_if(in_state(ClientAppState::InWorld)),
        );
    }
}

pub(super) struct ClientLightingPlugin;

impl Plugin for ClientLightingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<lighting::WorldLightingState>();
        app.init_resource::<lighting::CameraLocalLightSet>();
        let in_world_lighting = (
            lighting::sync_world_lighting_state_system
                .after(replication::adopt_native_lightyear_replicated_entities),
            lighting::collect_thruster_local_light_emitters_system
                .after(visuals::update_thruster_plume_visuals_system),
            visuals::update_asteroid_shader_lighting_system
                .after(lighting::sync_world_lighting_state_system),
        );
        app.add_systems(
            Update,
            in_world_lighting.run_if(in_state(ClientAppState::InWorld)),
        );
    }
}

pub(super) struct ClientUiPlugin;

impl Plugin for ClientUiPlugin {
    fn build(&self, app: &mut App) {
        dev_console::register_console(app);
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
        app.add_systems(Update, auth_net::poll_asset_bootstrap_request_results);
        app.add_systems(Update, auth_net::trigger_asset_catalog_refresh_requests);
        app.add_systems(
            Update,
            gate_menu_camera_system.run_if(not(in_state(ClientAppState::InWorld))),
        );
        app.add_systems(
            Update,
            world_loading_ui::update_world_loading_screen
                .run_if(in_state(ClientAppState::WorldLoading)),
        );
        app.add_systems(
            Update,
            asset_loading_ui::update_asset_loading_screen
                .run_if(in_state(ClientAppState::AssetLoading)),
        );
        app.add_systems(
            Update,
            (
                gate_gameplay_camera_system,
                ui::toggle_tactical_map_mode_system,
                ui::sync_tactical_map_camera_zoom_system.after(ui::toggle_tactical_map_mode_system),
                ui::update_owned_entities_panel_system
                    .after(owner_manifest::receive_owner_asset_manifest_messages),
                ui::handle_owned_entities_panel_buttons,
                ui::update_tactical_map_overlay_system
                    .after(tactical::receive_tactical_snapshot_messages),
                ui::update_loading_overlay_system,
                ui::update_runtime_stream_icon_system,
                bootstrap::watch_in_world_bootstrap_failures,
                audit_prediction_entity_lifecycle,
                ui::propagate_ui_overlay_layer_system,
                ui::update_hud_system,
                ui::sync_entity_nameplates_system
                    .after(visuals::suppress_duplicate_predicted_interpolated_visuals_system),
                toggle_debug_overlay_system,
            )
                .run_if(in_state(ClientAppState::InWorld)),
        );
        app.add_systems(
            PostUpdate,
            (
                // Lightyear still owns observer interpolation by default. This fallback only
                // snaps a remote visual root back onto its interpolated spatial pose if the
                // visual Transform lane is obviously stale or never got seeded.
                transforms::recover_stalled_interpolated_world_entity_transforms
                    .after(FrameInterpolationSystems::Interpolate)
                    .after(RollbackSystems::VisualCorrection),
                // Follow the same post-frame-interpolation ship transform that will actually be
                // rendered this frame. Running camera follow earlier in Update can make a
                // hard-locked camera disagree with the predicted ship after Lightyear applies
                // FrameInterpolate<Transform> and then VisualCorrection in PostUpdate.
                //
                // Sidereal's controlled ship can remain visually corrected for multiple render
                // frames after a rollback/correction event, so sampling after interpolation alone
                // is still too early for a truly locked camera.
                update_topdown_camera_system
                    .after(FrameInterpolationSystems::Interpolate)
                    .after(RollbackSystems::VisualCorrection)
                    .after(transforms::recover_stalled_interpolated_world_entity_transforms)
                    .after(transforms::sync_interpolated_world_entity_transforms_without_history),
                sync_planet_body_camera_to_gameplay_camera_system
                    .after(update_topdown_camera_system),
                sync_ui_overlay_camera_to_gameplay_camera_system
                    .after(update_topdown_camera_system),
                sync_debug_overlay_camera_to_gameplay_camera_system
                    .after(update_topdown_camera_system),
                update_camera_motion_state.after(update_topdown_camera_system),
                visuals::update_streamed_visual_layer_transforms_system
                    .after(update_camera_motion_state)
                    .after(visuals::attach_streamed_visual_assets_system),
                visuals::update_planet_body_visuals_system
                    .after(update_camera_motion_state)
                    .after(visuals::ensure_planet_body_root_visibility_system)
                    .after(visuals::attach_planet_visual_stack_system),
            )
                .before(bevy::transform::TransformSystems::Propagate)
                .run_if(in_state(ClientAppState::InWorld)),
        );
        app.add_systems(
            PostUpdate,
            (
                collect_debug_overlay_snapshot_system
                    .after(FrameInterpolationSystems::Interpolate)
                    .after(RollbackSystems::VisualCorrection)
                    .after(transforms::recover_stalled_interpolated_world_entity_transforms)
                    .after(bevy::transform::TransformSystems::Propagate)
                    .run_if(debug_overlay_enabled),
                ui::update_debug_overlay_text_ui_system
                    .after(collect_debug_overlay_snapshot_system),
            )
                .run_if(in_state(ClientAppState::InWorld)),
        );
        app.add_systems(
            Update,
            ui::toggle_nameplates_system.run_if(in_state(ClientAppState::InWorld)),
        );
        app.add_systems(
            Update,
            (
                ui::update_entity_nameplate_positions_system
                    .after(ui::sync_entity_nameplates_system),
                ui::update_segmented_bars_system
                    .after(ui::update_entity_nameplate_positions_system),
            )
                .run_if(in_state(ClientAppState::InWorld)),
        );
        app.add_systems(
            Update,
            (
                pause_menu::toggle_pause_menu_system,
                pause_menu::sync_pause_menu_ui_system.after(pause_menu::toggle_pause_menu_system),
                pause_menu::handle_pause_menu_interactions_system,
            )
                .run_if(in_state(ClientAppState::InWorld)),
        );
        app.add_systems(
            Update,
            ui::update_runtime_screen_overlay_passes_system
                .after(ui::update_tactical_map_overlay_system)
                .run_if(in_state(ClientAppState::InWorld)),
        );
        app.add_systems(
            Last,
            (
                super::backdrop::compute_fullscreen_external_world_system,
                super::backdrop::update_starfield_material_system
                    .after(super::backdrop::compute_fullscreen_external_world_system),
                super::backdrop::update_space_background_material_system
                    .after(super::backdrop::update_starfield_material_system),
                sync_debug_velocity_arrow_mesh_system
                    .after(super::backdrop::update_space_background_material_system),
                draw_debug_overlay_system
                    .after(sync_debug_velocity_arrow_mesh_system)
                    .run_if(debug_overlay_enabled),
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
            logout::request_logout_system.run_if(in_state(ClientAppState::AssetLoading)),
            logout::request_logout_system.run_if(in_state(ClientAppState::CharacterSelect)),
            logout::request_logout_on_window_close_system.run_if(in_state(ClientAppState::InWorld)),
            logout::request_logout_on_window_close_system
                .run_if(in_state(ClientAppState::WorldLoading)),
            logout::request_logout_on_window_close_system
                .run_if(in_state(ClientAppState::AssetLoading)),
            logout::request_logout_on_window_close_system
                .run_if(in_state(ClientAppState::CharacterSelect)),
            logout::send_disconnect_notify_and_trigger_system,
            logout::reset_asset_bootstrap_state_system
                .run_if(resource_equals(LogoutCleanupRequested(true))),
            logout::reset_asset_hot_reload_state_system
                .run_if(resource_equals(LogoutCleanupRequested(true))),
            logout::reset_logout_ui_flags_system
                .run_if(resource_equals(LogoutCleanupRequested(true))),
            logout::logout_cleanup_system.run_if(resource_equals(LogoutCleanupRequested(true))),
        );
        #[cfg(target_arch = "wasm32")]
        let logout_chain = (
            logout::request_logout_system.run_if(in_state(ClientAppState::InWorld)),
            logout::request_logout_system.run_if(in_state(ClientAppState::WorldLoading)),
            logout::request_logout_system.run_if(in_state(ClientAppState::AssetLoading)),
            logout::request_logout_system.run_if(in_state(ClientAppState::CharacterSelect)),
            logout::send_disconnect_notify_and_trigger_system,
            logout::reset_asset_bootstrap_state_system
                .run_if(resource_equals(LogoutCleanupRequested(true))),
            logout::reset_asset_hot_reload_state_system
                .run_if(resource_equals(LogoutCleanupRequested(true))),
            logout::reset_logout_ui_flags_system
                .run_if(resource_equals(LogoutCleanupRequested(true))),
            logout::logout_cleanup_system.run_if(resource_equals(LogoutCleanupRequested(true))),
        );
        app.add_systems(PreUpdate, logout_chain.chain());
    }
}

pub(super) struct ClientDiagnosticsPlugin;

impl Plugin for ClientDiagnosticsPlugin {
    fn build(&self, _app: &mut App) {}
}
