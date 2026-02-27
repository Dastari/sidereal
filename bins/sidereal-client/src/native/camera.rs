//! Top-down camera, overlay sync, motion state, and camera audit.

use bevy::camera::visibility::RenderLayers;
use bevy::ecs::query::Has;
use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
use sidereal_runtime_sync::RuntimeEntityHierarchy;

use super::components::{
    ControlledEntity, GameplayCamera, GameplayHud, TopDownCamera, UiOverlayCamera,
};
use super::platform::ORTHO_SCALE_PER_DISTANCE;
use super::replication::resolve_camera_anchor_entity;
use super::resources::CameraMotionState;
use super::state::{ClientSession, FreeCameraState, LocalPlayerViewState};
use avian2d::prelude::Position;

#[allow(clippy::too_many_arguments)]
#[allow(clippy::type_complexity)]
pub(crate) fn update_topdown_camera_system(
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
    use bevy::input::mouse::MouseScrollUnit;
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
        (free_camera.position_xy, false)
    } else if let Some(anchor_xy) = follow_anchor {
        free_camera.position_xy = anchor_xy;
        free_camera.initialized = true;
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
pub(crate) fn sync_ui_overlay_camera_to_gameplay_camera_system(
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

pub(crate) fn update_camera_motion_state(
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

    let pos_alpha = 1.0 - (-20.0 * dt).exp();
    motion.smoothed_position_xy = motion.smoothed_position_xy.lerp(current_xy, pos_alpha);

    if dt > 0.0 {
        let raw_velocity = frame_delta_xy / dt;
        let vel_alpha = 1.0 - (-12.0 * dt).exp();
        motion.smoothed_velocity_xy = motion.smoothed_velocity_xy.lerp(raw_velocity, vel_alpha);
    }
    motion.prev_position_xy = current_xy;
}

pub(crate) fn gate_gameplay_camera_system(
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
pub(crate) fn audit_active_world_cameras_system(
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
        bevy::log::warn!(
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
