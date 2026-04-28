use bevy::prelude::*;
use lightyear::frame_interpolation::FrameInterpolationSystems;
use lightyear::prelude::RollbackSystems;

use crate::runtime::app_state::ClientAppState;
use crate::runtime::camera::{
    sync_debug_overlay_camera_to_gameplay_camera_system,
    sync_planet_body_camera_to_gameplay_camera_system,
    sync_ui_overlay_camera_to_gameplay_camera_system, update_camera_motion_state,
    update_topdown_camera_system,
};
use crate::runtime::debug_overlay::{
    collect_debug_overlay_snapshot_system, debug_overlay_enabled, draw_debug_overlay_system,
    sync_debug_velocity_arrow_mesh_system,
};
use crate::runtime::{backdrop, sensor_ring, ui, visuals};

pub(super) fn add_in_world_post_update_systems(app: &mut App) {
    app.add_systems(
        PostUpdate,
        (
            // Follow the same post-frame-interpolation ship transform that will actually be
            // rendered this frame. Running camera follow earlier in Update can make a
            // hard-locked camera disagree with the predicted ship after Lightyear applies
            // FrameInterpolate<Transform> and then VisualCorrection in PostUpdate.
            update_topdown_camera_system
                .after(FrameInterpolationSystems::Interpolate)
                .after(RollbackSystems::VisualCorrection),
            sync_planet_body_camera_to_gameplay_camera_system.after(update_topdown_camera_system),
            sync_ui_overlay_camera_to_gameplay_camera_system.after(update_topdown_camera_system),
            sync_debug_overlay_camera_to_gameplay_camera_system.after(update_topdown_camera_system),
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
            sensor_ring::update_tactical_sensor_ring_overlay_system
                .after(bevy::transform::TransformSystems::Propagate),
            ui::update_entity_nameplate_positions_system
                .after(bevy::transform::TransformSystems::Propagate),
            ui::update_segmented_bars_system.after(ui::update_entity_nameplate_positions_system),
            collect_debug_overlay_snapshot_system
                .after(FrameInterpolationSystems::Interpolate)
                .after(RollbackSystems::VisualCorrection)
                .after(ui::update_segmented_bars_system)
                .after(bevy::transform::TransformSystems::Propagate)
                .run_if(debug_overlay_enabled),
            ui::update_debug_overlay_text_ui_system.after(collect_debug_overlay_snapshot_system),
            ui::sync_debug_entity_callouts_system.after(collect_debug_overlay_snapshot_system),
        )
            .run_if(in_state(ClientAppState::InWorld)),
    );
}

pub(super) fn add_in_world_last_stage_systems(app: &mut App) {
    app.add_systems(
        Last,
        (
            backdrop::compute_fullscreen_external_world_system,
            backdrop::update_starfield_material_system
                .after(backdrop::compute_fullscreen_external_world_system),
            backdrop::update_space_background_material_system
                .after(backdrop::update_starfield_material_system),
            sync_debug_velocity_arrow_mesh_system
                .after(backdrop::update_space_background_material_system)
                .run_if(debug_overlay_enabled),
            draw_debug_overlay_system
                .after(sync_debug_velocity_arrow_mesh_system)
                .run_if(debug_overlay_enabled),
        )
            .chain()
            .run_if(in_state(ClientAppState::InWorld)),
    );
}

#[cfg(test)]
mod tests {
    use super::add_in_world_post_update_systems;
    use bevy::ecs::schedule::Schedules;
    use bevy::prelude::*;

    #[test]
    fn nameplate_projection_system_runs_in_post_update_not_update() {
        let mut app = App::new();
        super::super::in_world::add_in_world_ui_update_systems(&mut app);
        add_in_world_post_update_systems(&mut app);

        let mut schedules = app
            .world_mut()
            .remove_resource::<Schedules>()
            .expect("Schedules resource should exist");
        let update_system_names = {
            let update = schedules
                .get_mut(Update)
                .expect("Update schedule should exist");
            update
                .initialize(app.world_mut())
                .expect("Update schedule should initialize");
            update
                .systems()
                .expect("Update schedule should expose systems after initialization")
                .map(|(_, system)| system.name().to_string())
                .collect::<Vec<_>>()
        };
        let post_update_system_names = {
            let post_update = schedules
                .get_mut(PostUpdate)
                .expect("PostUpdate schedule should exist");
            post_update
                .initialize(app.world_mut())
                .expect("PostUpdate schedule should initialize");
            post_update
                .systems()
                .expect("PostUpdate schedule should expose systems after initialization")
                .map(|(_, system)| system.name().to_string())
                .collect::<Vec<_>>()
        };
        app.world_mut().insert_resource(schedules);

        assert!(
            !update_system_names
                .iter()
                .any(|name| name.contains("update_entity_nameplate_positions_system")),
            "nameplate projection should not run in Update"
        );
        assert!(
            !update_system_names
                .iter()
                .any(|name| name.contains("update_tactical_sensor_ring_overlay_system")),
            "sensor ring projection should not run in Update"
        );
        assert!(
            post_update_system_names
                .iter()
                .any(|name| name.contains("update_entity_nameplate_positions_system")),
            "nameplate projection should run in PostUpdate"
        );
        assert!(
            post_update_system_names
                .iter()
                .any(|name| name.contains("update_tactical_sensor_ring_overlay_system")),
            "sensor ring projection should run in PostUpdate"
        );
    }
}
