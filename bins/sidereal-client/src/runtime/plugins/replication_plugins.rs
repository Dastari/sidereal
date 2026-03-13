use bevy::prelude::*;
use sidereal_game::process_character_movement_actions;

use crate::runtime::app_state::ClientAppState;
use crate::runtime::motion::{
    apply_predicted_input_to_action_queue, enforce_controlled_planar_motion,
};
use crate::runtime::{
    assets, backdrop, control, input, owner_manifest, replication, tactical, transforms,
};

pub(crate) struct ClientReplicationPlugin {
    pub(crate) headless: bool,
}

fn add_shared_replication_maintenance_systems(app: &mut App) {
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
}

fn add_shared_replication_runtime_systems(app: &mut App) {
    app.add_systems(
        Update,
        (
            replication::ensure_prediction_manager_present_system,
            replication::configure_prediction_manager_tuning,
            replication::prune_runtime_entity_registry_system,
            (
                assets::mark_runtime_asset_dependency_state_dirty_system
                    .after(replication::prune_runtime_entity_registry_system),
                assets::sync_runtime_asset_dependency_state_system
                    .after(assets::mark_runtime_asset_dependency_state_dirty_system),
                assets::queue_missing_catalog_assets_system
                    .after(assets::sync_runtime_asset_dependency_state_system),
                assets::poll_runtime_asset_http_fetches_system
                    .after(assets::queue_missing_catalog_assets_system),
            ),
            replication::adopt_native_lightyear_replicated_entities
                .after(replication::prune_runtime_entity_registry_system),
            transforms::sync_frame_interpolation_markers_for_world_entities
                .after(replication::adopt_native_lightyear_replicated_entities),
            transforms::sync_confirmed_world_entity_transforms_from_physics
                .after(transforms::sync_frame_interpolation_markers_for_world_entities),
            transforms::sync_confirmed_world_entity_transforms_from_world_space
                .after(transforms::sync_confirmed_world_entity_transforms_from_physics),
            transforms::sync_interpolated_world_entity_transforms_without_history
                .after(transforms::sync_confirmed_world_entity_transforms_from_world_space),
            transforms::reveal_world_entities_when_initial_transform_ready
                .after(transforms::sync_interpolated_world_entity_transforms_without_history),
        ),
    );
}

fn add_shared_replication_control_systems(app: &mut App) {
    app.add_systems(
        Update,
        (
            (
                replication::sync_local_player_view_state_system
                    .after(transforms::sync_confirmed_world_entity_transforms_from_world_space),
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
            assets::receive_asset_catalog_version_messages
                .after(control::receive_lightyear_control_results),
            owner_manifest::receive_owner_asset_manifest_messages
                .after(assets::receive_asset_catalog_version_messages),
            tactical::receive_tactical_snapshot_messages
                .after(owner_manifest::receive_owner_asset_manifest_messages),
        ),
    );
}

fn add_non_headless_replication_transition_systems(app: &mut App) {
    app.add_systems(
        Update,
        (
            replication::transition_world_loading_to_in_world
                .after(transforms::sync_confirmed_world_entity_transforms_from_world_space),
            replication::transition_asset_loading_to_in_world
                .after(replication::transition_world_loading_to_in_world),
        ),
    );
}

impl Plugin for ClientReplicationPlugin {
    fn build(&self, app: &mut App) {
        add_shared_replication_maintenance_systems(app);
        add_shared_replication_runtime_systems(app);
        add_shared_replication_control_systems(app);
        if !self.headless {
            add_non_headless_replication_transition_systems(app);
        }
    }
}

pub(crate) struct ClientPredictionPlugin {
    pub(crate) headless: bool,
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
