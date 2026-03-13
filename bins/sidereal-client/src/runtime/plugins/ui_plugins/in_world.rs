use bevy::prelude::*;

use crate::runtime::app_state::ClientAppState;
use crate::runtime::camera::{audit_active_world_cameras_system, gate_gameplay_camera_system};
use crate::runtime::debug_overlay::toggle_debug_overlay_system;
use crate::runtime::{bootstrap, owner_manifest, pause_menu, tactical, ui, visuals};

pub(super) fn add_in_world_ui_update_systems(app: &mut App) {
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
            ui::propagate_ui_overlay_layer_system,
            ui::update_hud_system,
            ui::sync_entity_nameplates_system
                .after(visuals::suppress_duplicate_predicted_interpolated_visuals_system),
            toggle_debug_overlay_system,
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
        Update,
        audit_active_world_cameras_system.run_if(in_state(ClientAppState::InWorld)),
    );
}
