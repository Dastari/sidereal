//! F3 debug overlay: toggle and draw (AABB, velocity arrows, visibility circle).

use avian2d::prelude::{LinearVelocity, Position, Rotation};
use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use lightyear::interpolation::interpolation_history::ConfirmedHistory;
use lightyear::prediction::prelude::{PredictionHistory, PredictionManager};
use sidereal_game::{
    CollisionAabbM, CollisionOutlineM, EntityGuid, Hardpoint, MountedOn, ScannerRangeM, SizeM,
};
use sidereal_runtime_sync::RuntimeEntityHierarchy;

use super::app_state::{ClientSession, LocalPlayerViewState};
use super::components::{ControlledEntity, HudFpsText, WorldEntity};
use super::resources::{
    BootstrapWatchdogState, DebugOverlayEnabled, DeferredPredictedAdoptionState,
    LocalSimulationDebugMode, PredictionBootstrapTuning,
};

#[derive(Default)]
pub(crate) struct RollbackSampleState {
    last_active: bool,
    total_entries: u64,
    entries_since_log: u64,
    active_frames_since_log: u64,
    last_confirmed_tick: Option<u16>,
}

pub(crate) fn toggle_debug_overlay_system(
    input: Res<'_, ButtonInput<KeyCode>>,
    mut debug_overlay: ResMut<'_, DebugOverlayEnabled>,
) {
    if input.just_pressed(KeyCode::F3) {
        debug_overlay.enabled = !debug_overlay.enabled;
    }
}

#[derive(Default)]
pub(crate) struct DebugFpsSmoothingState {
    ema_fps: Option<f64>,
}

pub(crate) fn update_debug_fps_text_system(
    debug_overlay: Res<'_, DebugOverlayEnabled>,
    diagnostics: Res<'_, DiagnosticsStore>,
    mut smoothing: Local<'_, DebugFpsSmoothingState>,
    mut fps_query: Query<
        '_,
        '_,
        (&'_ mut Text, &'_ mut TextColor, &'_ mut Visibility),
        With<HudFpsText>,
    >,
) {
    if fps_query.is_empty() {
        return;
    }

    if !debug_overlay.enabled {
        for (mut text, mut text_color, mut visibility) in &mut fps_query {
            text.0.clear();
            text_color.0.set_alpha(0.0);
            *visibility = Visibility::Hidden;
        }
        return;
    }

    let instant_fps = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|fps| fps.smoothed().or_else(|| fps.value()));

    if let Some(sample) = instant_fps {
        // Additional long-window EMA on top of diagnostics smoothing.
        let alpha = 0.08_f64;
        smoothing.ema_fps = Some(match smoothing.ema_fps {
            Some(previous) => previous + (sample - previous) * alpha,
            None => sample,
        });
    }

    let display = smoothing
        .ema_fps
        .map(|fps| format!("FPS {}", fps.round() as i64))
        .unwrap_or_else(|| "FPS --".to_string());
    for (mut text, mut text_color, mut visibility) in &mut fps_query {
        text.0 = display.clone();
        text_color.0.set_alpha(1.0);
        *visibility = Visibility::Visible;
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn draw_debug_overlay_system(
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
            &'_ GlobalTransform,
            Option<&'_ SizeM>,
            Option<&'_ CollisionAabbM>,
            Option<&'_ CollisionOutlineM>,
            Option<&'_ LinearVelocity>,
            Option<&'_ MountedOn>,
            Option<&'_ Hardpoint>,
            Option<&'_ ControlledEntity>,
            Option<&'_ ScannerRangeM>,
            Option<&'_ EntityGuid>,
            Option<&'_ lightyear::prelude::Confirmed<Position>>,
            Option<&'_ lightyear::prelude::Confirmed<Rotation>>,
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
        global_transform,
        size_m,
        collision_aabb,
        collision_outline,
        linear_velocity,
        mounted_on,
        hardpoint,
        controlled_marker,
        scanner_range,
        _entity_guid,
        confirmed_position,
        confirmed_rotation,
    ) in &entities
    {
        let world = global_transform.compute_transform();
        let pos = world.translation;
        let rot = world.rotation;
        let half_extents = collision_aabb.map(|aabb| aabb.half_extents).or_else(|| {
            size_m.map(|size| Vec3::new(size.width * 0.5, size.length * 0.5, size.height * 0.5))
        });

        let is_local_controlled = (mounted_on.is_none()
            && hardpoint.is_none()
            && Some(entity) == local_controlled_entity)
            || controlled_marker.is_some_and(|controlled| {
                session
                    .player_entity_id
                    .as_deref()
                    .is_some_and(|player_id| controlled.player_entity_id == player_id)
            });

        if let Some(outline) = collision_outline {
            let draw_color = if is_local_controlled && mounted_on.is_none() {
                controlled_predicted_color
            } else {
                collision_color
            };
            for idx in 0..outline.points.len() {
                let a = outline.points[idx];
                let b = outline.points[(idx + 1) % outline.points.len()];
                let world_a = pos + (rot * a.extend(0.0));
                let world_b = pos + (rot * b.extend(0.0));
                gizmos.line(world_a, world_b, draw_color);
            }

            if is_local_controlled
                && mounted_on.is_none()
                && let (Some(confirmed_position), Some(confirmed_rotation)) =
                    (confirmed_position, confirmed_rotation)
            {
                let confirmed_pos = confirmed_position.0.0.extend(0.0);
                let confirmed_rot: Quat = confirmed_rotation.0.into();
                for idx in 0..outline.points.len() {
                    let a = outline.points[idx];
                    let b = outline.points[(idx + 1) % outline.points.len()];
                    let world_a = confirmed_pos + (confirmed_rot * a.extend(0.0));
                    let world_b = confirmed_pos + (confirmed_rot * b.extend(0.0));
                    gizmos.line(world_a, world_b, controlled_confirmed_color);
                }
                gizmos.line(pos, confirmed_pos, prediction_error_color);
            }
        } else if let Some(half_extents) = half_extents {
            let aabb = bevy::math::bounding::Aabb3d::new(Vec3::ZERO, half_extents);
            let transform = Transform::from_translation(pos).with_rotation(rot);
            let draw_color = if is_local_controlled && mounted_on.is_none() {
                controlled_predicted_color
            } else {
                collision_color
            };
            gizmos.aabb_3d(aabb, transform, draw_color);

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
            let range_m = scanner_range
                .map(|r| r.0.max(0.0))
                .unwrap_or(300.0)
                .max(1.0);
            controlled_visibility_circle = Some((pos, range_m));
        }

        if mounted_on.is_none()
            && is_local_controlled
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

#[allow(clippy::too_many_arguments)]
#[allow(clippy::type_complexity)]
pub(crate) fn log_prediction_runtime_state(
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
    interpolated_spatial_entities: Query<
        '_,
        '_,
        Entity,
        (With<lightyear::prelude::Interpolated>, With<Position>),
    >,
    interpolation_history_probe: Query<
        '_,
        '_,
        Has<ConfirmedHistory<Position>>,
        (With<lightyear::prelude::Interpolated>, With<Position>),
    >,
    controlled_prediction_state: Query<
        '_,
        '_,
        (
            Entity,
            Option<&'_ EntityGuid>,
            Has<lightyear::prelude::Predicted>,
            Option<&'_ lightyear::prelude::Confirmed<Position>>,
            Option<&'_ lightyear::prelude::Confirmed<Rotation>>,
            Option<&'_ lightyear::prelude::Confirmed<LinearVelocity>>,
            Option<&'_ lightyear::prelude::ConfirmedTick>,
            Has<PredictionHistory<Position>>,
            Has<PredictionHistory<Rotation>>,
            Has<PredictionHistory<LinearVelocity>>,
            Option<&'_ Position>,
            Option<&'_ Rotation>,
            Option<&'_ LinearVelocity>,
        ),
        With<ControlledEntity>,
    >,
    prediction_managers: Query<
        '_,
        '_,
        (
            Entity,
            &'_ PredictionManager,
            Has<lightyear::prelude::client::Client>,
            Has<lightyear::prelude::Rollback>,
        ),
    >,
    mut rollback_sample: Local<'_, RollbackSampleState>,
) {
    let now_s = time.elapsed_secs_f64();
    let mut rollback_active = false;
    for (_, manager, _, _) in &prediction_managers {
        rollback_active |= manager.is_rollback();
    }
    if rollback_active {
        rollback_sample.active_frames_since_log =
            rollback_sample.active_frames_since_log.saturating_add(1);
        if !rollback_sample.last_active {
            rollback_sample.total_entries = rollback_sample.total_entries.saturating_add(1);
            rollback_sample.entries_since_log = rollback_sample.entries_since_log.saturating_add(1);
        }
    }
    rollback_sample.last_active = rollback_active;

    if now_s - adoption_state.last_runtime_summary_at_s < tuning.defer_summary_interval_s {
        return;
    }
    adoption_state.last_runtime_summary_at_s = now_s;
    let world_count = world_entities.iter().count();
    let replicated_count = replicated_entities.iter().count();
    let predicted_count = predicted_entities.iter().count();
    let interpolated_count = interpolated_entities.iter().count();
    let controlled_count = controlled_entities.iter().count();
    let manager_count = prediction_managers.iter().count();
    let manager_with_client_count = prediction_managers
        .iter()
        .filter(|(_, _, has_client, _)| *has_client)
        .count();
    let manager_with_rollback_count = prediction_managers
        .iter()
        .filter(|(_, _, _, has_rollback)| *has_rollback)
        .count();
    let manager_state = prediction_managers
        .iter()
        .next()
        .map(|(entity, manager, _, _)| {
            (
                entity,
                manager.rollback_policy.state,
                manager.rollback_policy.input,
                manager.rollback_policy.max_rollback_ticks,
                manager.get_rollback_start_tick().map(|tick| tick.0),
            )
        });
    let mode = if local_mode.0 { "local" } else { "predicted" };
    bevy::log::info!(
        "prediction runtime summary mode={} world={} replicated={} predicted={} interpolated={} controlled={} rollback_active={} rollback_entries_since_log={} rollback_entries_total={} rollback_active_frames_since_log={} managers={} managers_with_client={} managers_with_rollback={} manager_entity={:?} rollback_policy={:?}/{:?} rollback_max_ticks={} rollback_start_tick={:?} deferred_waiting={}",
        mode,
        world_count,
        replicated_count,
        predicted_count,
        interpolated_count,
        controlled_count,
        rollback_active,
        rollback_sample.entries_since_log,
        rollback_sample.total_entries,
        rollback_sample.active_frames_since_log,
        manager_count,
        manager_with_client_count,
        manager_with_rollback_count,
        manager_state.map(|v| v.0),
        manager_state.map(|v| v.1),
        manager_state.map(|v| v.2),
        manager_state.map(|v| v.3).unwrap_or_default(),
        manager_state.and_then(|v| v.4),
        adoption_state
            .waiting_entity_id
            .as_deref()
            .unwrap_or("<none>")
    );
    let interpolated_spatial_count = interpolated_spatial_entities.iter().count();
    let interp_with_history = interpolation_history_probe
        .iter()
        .filter(|has| *has)
        .count();
    let interp_without_history = interpolated_spatial_count.saturating_sub(interp_with_history);
    if interpolated_count > 0 {
        bevy::log::info!(
            "interpolation pipeline: interpolated={} spatial={} with_confirmed_history={} missing_history={}",
            interpolated_count,
            interpolated_spatial_count,
            interp_with_history,
            interp_without_history,
        );
    }
    if let Ok((
        controlled_entity,
        guid,
        is_predicted_marker,
        confirmed_position,
        confirmed_rotation,
        confirmed_velocity,
        confirmed_tick,
        has_position_history,
        has_rotation_history,
        has_velocity_history,
        current_position,
        current_rotation,
        current_velocity,
    )) = controlled_prediction_state.single()
    {
        let confirmed_tick_value = confirmed_tick.map(|tick| tick.tick.0);
        let confirmed_tick_advanced = confirmed_tick_value
            .zip(rollback_sample.last_confirmed_tick)
            .is_some_and(|(current, previous)| current != previous);
        bevy::log::info!(
            "prediction controlled entity={} guid={} predicted_marker={} confirmed_pos={} confirmed_rot={} confirmed_vel={} confirmed_tick={:?} confirmed_tick_advanced={} hist_pos={} hist_rot={} hist_vel={} current_pos={:?} current_rot_rad={:?} current_vel={:?}",
            controlled_entity,
            guid.map(|v| v.0.to_string())
                .unwrap_or_else(|| "<none>".to_string()),
            is_predicted_marker,
            confirmed_position.is_some(),
            confirmed_rotation.is_some(),
            confirmed_velocity.is_some(),
            confirmed_tick_value,
            confirmed_tick_advanced,
            has_position_history,
            has_rotation_history,
            has_velocity_history,
            current_position.map(|v| v.0),
            current_rotation.map(|v| v.as_radians()),
            current_velocity.map(|v| v.0),
        );
        rollback_sample.last_confirmed_tick = confirmed_tick_value;
    }
    rollback_sample.entries_since_log = 0;
    rollback_sample.active_frames_since_log = 0;
    if !local_mode.0 && watchdog.replication_state_seen {
        let in_world_age_s = watchdog
            .in_world_entered_at_s
            .map(|entered_at_s| (now_s - entered_at_s).max(0.0))
            .unwrap_or_default();
        if in_world_age_s > tuning.defer_dialog_after_s && controlled_count == 0 {
            bevy::log::warn!(
                "prediction runtime anomaly: no controlled entity after {:.2}s in predicted mode (replicated={} predicted={} interpolated={})",
                in_world_age_s,
                replicated_count,
                predicted_count,
                interpolated_count
            );
        }
        if replicated_count > 0 && predicted_count == 0 {
            bevy::log::warn!(
                "prediction runtime anomaly: replicated entities present but zero Predicted markers (replicated={} interpolated={})",
                replicated_count,
                interpolated_count
            );
        }
    }
}
