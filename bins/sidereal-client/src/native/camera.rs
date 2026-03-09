//! Top-down camera, overlay sync, motion state, and camera audit.

use bevy::camera::ScalingMode;
use bevy::camera::visibility::RenderLayers;
use bevy::ecs::query::Has;
use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
use sidereal_game::EntityGuid;
use sidereal_runtime_sync::RuntimeEntityHierarchy;

use super::app_state::{ClientSession, FreeCameraState, LocalPlayerViewState};
use super::components::{
    ClientSceneEntity, ControlledEntity, DebugOverlayCamera, GameplayCamera, GameplayHud,
    TopDownCamera, UiOverlayCamera,
};
use super::dev_console::{DevConsoleState, is_console_open};
use super::platform::ORTHO_SCALE_PER_DISTANCE;
use super::resources::{CameraMotionState, DebugOverlayState, TacticalMapUiState};

fn parse_entity_id_guid(raw: &str) -> Option<uuid::Uuid> {
    sidereal_runtime_sync::parse_guid_from_entity_id(raw)
        .or_else(|| uuid::Uuid::parse_str(raw).ok())
}

#[allow(clippy::type_complexity)]
pub(crate) fn resolve_camera_anchor_entity(
    session: &ClientSession,
    player_view_state: &LocalPlayerViewState,
    entity_registry: &RuntimeEntityHierarchy,
    anchor_candidates: &Query<
        '_,
        '_,
        (
            Entity,
            Option<&'_ EntityGuid>,
            Has<lightyear::prelude::Predicted>,
            Has<lightyear::prelude::Interpolated>,
        ),
        (Without<Camera>, Without<GameplayCamera>),
    >,
) -> Option<Entity> {
    let find_best_runtime_by_guid = |target_guid: uuid::Uuid| {
        let mut winner: Option<(Entity, i32)> = None;
        for (entity, guid, is_predicted, is_interpolated) in anchor_candidates {
            if guid.is_none_or(|guid| guid.0 != target_guid) {
                continue;
            }
            let score = if is_predicted {
                3
            } else if is_interpolated {
                2
            } else {
                1
            };
            if winner.is_none_or(|(_, best_score)| score > best_score) {
                winner = Some((entity, score));
            }
        }
        winner.map(|(entity, _)| entity)
    };

    let player_id = session.player_entity_id.as_deref()?;
    let player_guid = sidereal_runtime_sync::parse_guid_from_entity_id(player_id)
        .or_else(|| uuid::Uuid::parse_str(player_id).ok());

    // When controlling another entity, follow that entity directly to avoid
    // dual-follow rubber-banding (controlled -> player -> camera).
    if let (Some(player_guid), Some(controlled_id)) = (
        player_guid,
        player_view_state.controlled_entity_id.as_deref(),
    ) && let Some(controlled_guid) = parse_entity_id_guid(controlled_id)
        && controlled_guid != player_guid
        && let Some(controlled_entity) = find_best_runtime_by_guid(controlled_guid)
    {
        return Some(controlled_entity);
    }

    if let Some(player_guid) = player_guid
        && let Some(entity) = find_best_runtime_by_guid(player_guid)
    {
        return Some(entity);
    }

    // Camera contract: always follow the local player entity.
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
pub(crate) fn update_topdown_camera_system(
    time: Res<'_, Time>,
    input: Option<Res<'_, ButtonInput<KeyCode>>>,
    dev_console_state: Option<Res<'_, DevConsoleState>>,
    tactical_map_state: Res<'_, TacticalMapUiState>,
    mut mouse_wheel_events: MessageReader<'_, '_, MouseWheel>,
    session: Res<'_, ClientSession>,
    player_view_state: Res<'_, LocalPlayerViewState>,
    entity_registry: Res<'_, RuntimeEntityHierarchy>,
    anchor_query: Query<'_, '_, &'_ Transform, (Without<Camera>, Without<GameplayCamera>)>,
    anchor_candidates: Query<
        '_,
        '_,
        (
            Entity,
            Option<&'_ EntityGuid>,
            Has<lightyear::prelude::Predicted>,
            Has<lightyear::prelude::Interpolated>,
        ),
        (Without<Camera>, Without<GameplayCamera>),
    >,
    controlled_anchor_candidates: Query<
        '_,
        '_,
        (
            &'_ Transform,
            Has<lightyear::prelude::Predicted>,
            Has<lightyear::prelude::Interpolated>,
        ),
        (
            With<ControlledEntity>,
            Without<Camera>,
            Without<GameplayCamera>,
        ),
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
    let suppress_for_console = is_console_open(dev_console_state.as_deref());

    let mut wheel_delta_y = 0.0f32;
    for event in mouse_wheel_events.read() {
        if suppress_for_console {
            continue;
        }
        let normalized = match event.unit {
            MouseScrollUnit::Line => event.y,
            MouseScrollUnit::Pixel => event.y / 32.0,
        };
        wheel_delta_y += normalized.clamp(-4.0, 4.0);
    }
    if wheel_delta_y != 0.0 && !tactical_map_state.enabled {
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

    // Prefer the concrete controlled runtime entity first. This avoids follow jitter from
    // GUID-level ambiguity when stale/interpolated duplicates coexist during relevance churn.
    let controlled_anchor = controlled_anchor_candidates
        .iter()
        .fold(
            None::<(Vec2, i32)>,
            |winner, (transform, is_predicted, is_interpolated)| {
                let score = if is_predicted {
                    3
                } else if is_interpolated {
                    2
                } else {
                    1
                };
                if winner.is_none_or(|(_, best_score)| score > best_score) {
                    Some((transform.translation.truncate(), score))
                } else {
                    winner
                }
            },
        )
        .map(|(xy, _)| xy);
    let follow_anchor = controlled_anchor.or_else(|| {
        resolve_camera_anchor_entity(
            &session,
            &player_view_state,
            &entity_registry,
            &anchor_candidates,
        )
        .and_then(|entity| anchor_query.get(entity).ok())
        .map(|anchor_transform| anchor_transform.translation.truncate())
    });

    let focus_xy = if player_view_state.detached_free_camera {
        if !free_camera.initialized {
            free_camera.position_xy = camera_transform.translation.truncate();
            free_camera.initialized = true;
        }
        let mut axis = Vec2::ZERO;
        if !suppress_for_console && let Some(keys) = input.as_ref() {
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
        free_camera.position_xy
    } else if let Some(anchor_xy) = follow_anchor {
        free_camera.position_xy = anchor_xy;
        free_camera.initialized = true;
        anchor_xy
    } else {
        let fallback_xy = camera_transform.translation.truncate();
        free_camera.position_xy = fallback_xy;
        free_camera.initialized = true;
        fallback_xy
    };
    let is_controlling_other = session
        .player_entity_id
        .as_deref()
        .and_then(parse_entity_id_guid)
        .zip(
            player_view_state
                .controlled_entity_id
                .as_deref()
                .and_then(parse_entity_id_guid),
        )
        .is_some_and(|(player_guid, controlled_guid)| controlled_guid != player_guid);

    if player_view_state.detached_free_camera {
        // Free camera should be direct and stable, not spring-filtered.
        camera.filtered_focus_xy = focus_xy;
        camera.focus_initialized = true;
    } else if is_controlling_other {
        // When controlling another entity, keep camera hard-locked to avoid
        // visible rubber-banding from follow smoothing.
        camera.filtered_focus_xy = focus_xy;
        camera.focus_initialized = true;
    } else if !camera.focus_initialized {
        camera.filtered_focus_xy = focus_xy;
        camera.focus_initialized = true;
    } else {
        // Keep attached follow simple and deterministic to avoid oscillation/lurch.
        let follow_smoothness = 18.0;
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

/// Keeps the UI overlay camera in true screen space: fixed at pixel (0,0) origin, orthographic
/// scale so 1 world unit = 1 pixel. HUD and nameplates then use stable pixel coordinates and
/// segment gaps stay consistent.
#[allow(clippy::type_complexity)]
pub(crate) fn sync_ui_overlay_camera_to_gameplay_camera_system(
    mut ui_camera: Query<
        '_,
        '_,
        (&mut Transform, &mut Projection),
        (With<UiOverlayCamera>, Without<GameplayCamera>),
    >,
) {
    // Keep the overlay camera in Bevy's default window-space mapping so UI layout
    // remains full-screen and independent from gameplay camera zoom/translation.
    for (mut ui_transform, mut ui_projection) in &mut ui_camera {
        ui_transform.translation = Vec3::ZERO;
        ui_transform.rotation = Quat::IDENTITY;
        if let Projection::Orthographic(ui_ortho) = &mut *ui_projection {
            ui_ortho.scaling_mode = ScalingMode::WindowSize;
            ui_ortho.scale = 1.0;
        }
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn sync_debug_overlay_camera_to_gameplay_camera_system(
    debug_overlay: Res<'_, DebugOverlayState>,
    gameplay_camera: Query<
        '_,
        '_,
        (&'_ Transform, &'_ Projection),
        (With<GameplayCamera>, Without<DebugOverlayCamera>),
    >,
    mut debug_camera: Query<
        '_,
        '_,
        (&'_ mut Camera, &'_ mut Transform, &'_ mut Projection),
        (With<DebugOverlayCamera>, Without<GameplayCamera>),
    >,
) {
    let Ok((gameplay_transform, gameplay_projection)) = gameplay_camera.single() else {
        return;
    };

    for (mut debug_camera, mut debug_transform, mut debug_projection) in &mut debug_camera {
        debug_camera.is_active = debug_overlay.enabled;
        if !debug_overlay.enabled {
            continue;
        }
        *debug_transform = *gameplay_transform;
        *debug_projection = gameplay_projection.clone();
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
    mut camera_query: Query<'_, '_, &mut Camera, (With<GameplayCamera>, Without<UiOverlayCamera>)>,
    mut hud_query: Query<'_, '_, &mut Visibility, With<GameplayHud>>,
    mut ui_camera_query: Query<
        '_,
        '_,
        &mut Camera,
        (With<UiOverlayCamera>, Without<GameplayCamera>),
    >,
) {
    for mut camera in &mut camera_query {
        camera.is_active = true;
    }
    for mut visibility in &mut hud_query {
        *visibility = Visibility::Visible;
    }
    for mut camera in &mut ui_camera_query {
        camera.clear_color = ClearColorConfig::None;
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn gate_menu_camera_system(
    mut scene_cameras: Query<
        '_,
        '_,
        &mut Camera,
        (
            With<ClientSceneEntity>,
            Without<UiOverlayCamera>,
            Without<GameplayCamera>,
        ),
    >,
    mut gameplay_cameras: Query<
        '_,
        '_,
        &mut Camera,
        (With<GameplayCamera>, Without<UiOverlayCamera>),
    >,
    mut hud_query: Query<'_, '_, &mut Visibility, With<GameplayHud>>,
    mut ui_camera_query: Query<
        '_,
        '_,
        &mut Camera,
        (With<UiOverlayCamera>, Without<GameplayCamera>),
    >,
) {
    for mut camera in &mut scene_cameras {
        camera.is_active = false;
    }
    for mut camera in &mut gameplay_cameras {
        camera.is_active = false;
    }
    for mut visibility in &mut hud_query {
        *visibility = Visibility::Hidden;
    }
    for mut camera in &mut ui_camera_query {
        camera.clear_color = ClearColorConfig::Custom(Color::BLACK);
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
