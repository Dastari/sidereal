//! F3 debug overlay: toggle and draw (AABB, velocity arrows, visibility circle).

use avian2d::prelude::{LinearVelocity, Position, Rotation};
use bevy::math::Isometry2d;
use bevy::prelude::*;
use lightyear::interpolation::interpolation_history::ConfirmedHistory;
use lightyear::prediction::correction::VisualCorrection;
use lightyear::prediction::prelude::{PredictionHistory, PredictionManager};
use sidereal_game::{
    CollisionAabbM, CollisionOutlineM, EntityGuid, Hardpoint, MountedOn, PlayerTag, SizeM,
    VisibilityRangeM,
};
use std::collections::{HashMap, HashSet};

use super::app_state::{ClientSession, LocalPlayerViewState};
use super::components::{ControlledEntity, WorldEntity};
use super::dev_console::{DevConsoleState, is_console_open};
use super::resources::{
    BootstrapWatchdogState, DebugOverlayEnabled, DeferredPredictedAdoptionState,
    LocalSimulationDebugMode, PredictionBootstrapTuning, PredictionLifecycleAuditConfig,
    PredictionLifecycleAuditState,
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
    dev_console_state: Option<Res<'_, DevConsoleState>>,
    mut debug_overlay: ResMut<'_, DebugOverlayEnabled>,
) {
    if is_console_open(dev_console_state.as_deref()) {
        return;
    }
    if input.just_pressed(KeyCode::F3) {
        debug_overlay.enabled = !debug_overlay.enabled;
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn draw_debug_overlay_system(
    debug_overlay: Res<'_, DebugOverlayEnabled>,
    session: Res<'_, ClientSession>,
    _player_view_state: Res<'_, LocalPlayerViewState>,
    mut gizmos: Gizmos,
    controlled_entities: Query<'_, '_, Entity, With<ControlledEntity>>,
    root_candidates: Query<
        '_,
        '_,
        (
            Entity,
            &'_ EntityGuid,
            Option<&'_ MountedOn>,
            Option<&'_ Hardpoint>,
            Option<&'_ PlayerTag>,
            Option<&'_ ControlledEntity>,
            Option<&'_ Visibility>,
            Has<lightyear::prelude::Replicated>,
            Has<lightyear::prelude::Interpolated>,
            Has<lightyear::prelude::Predicted>,
        ),
        With<WorldEntity>,
    >,
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
            Option<&'_ VisibilityRangeM>,
            Has<lightyear::prelude::Replicated>,
            Has<lightyear::prelude::Interpolated>,
            Has<lightyear::prelude::Predicted>,
            Option<&'_ lightyear::prelude::Confirmed<Position>>,
            Option<&'_ lightyear::prelude::Confirmed<Rotation>>,
        ),
        With<WorldEntity>,
    >,
) {
    if !debug_overlay.enabled {
        return;
    }
    const DEBUG_OVERLAY_Z_OFFSET: f32 = 6.0;
    const REPLICATED_OVERLAY_Z_STEP: f32 = 0.0;
    const INTERPOLATED_OVERLAY_Z_STEP: f32 = 0.18;
    const PREDICTED_OVERLAY_Z_STEP: f32 = 0.36;
    const CONFIRMED_OVERLAY_Z_STEP: f32 = 0.54;
    const CONFIRMED_OVERLAY_POSITION_EPSILON_M: f32 = 0.05;
    const CONFIRMED_OVERLAY_ROTATION_EPSILON_RAD: f32 = 0.01;
    let local_controlled_entity = controlled_entities
        .iter()
        .min_by_key(|entity| entity.to_bits());
    const VELOCITY_ARROW_SCALE: f32 = 0.5;
    const HARDPOINT_CROSS_HALF_SIZE: f32 = 2.0;
    let collision_color = Color::srgb(0.2, 0.8, 0.2);
    let velocity_color = Color::srgb(0.2, 0.5, 1.0);
    let hardpoint_color = Color::srgb(1.0, 0.8, 0.2);
    let controlled_predicted_color = Color::srgb(0.2, 1.0, 1.0);
    let controlled_confirmed_color = Color::srgb(1.0, 0.2, 1.0);
    let prediction_error_color = Color::srgb(1.0, 0.2, 0.2);
    let mut best_root_entity_by_guid = HashMap::<uuid::Uuid, (Entity, i32)>::new();

    // Keep the debug overlay aligned with the displayed runtime clone, not every stable duplicate.
    // Sidereal intentionally keeps confirmed roots alive beside Predicted/Interpolated clones for
    // confirmation/correction history, but drawing all of those roots in the debug overlay can
    // look like AABBs are flickering even when the underlying collision state is stable.
    for (
        entity,
        guid,
        mounted_on,
        hardpoint,
        player_tag,
        controlled_marker,
        visibility,
        is_replicated,
        is_interpolated,
        is_predicted,
    ) in &root_candidates
    {
        if mounted_on.is_some() || hardpoint.is_some() || player_tag.is_some() {
            continue;
        }
        if Some(entity) != local_controlled_entity
            && visibility.is_some_and(|visibility| *visibility == Visibility::Hidden)
        {
            continue;
        }

        let is_local_controlled = Some(entity) == local_controlled_entity
            || controlled_marker.is_some_and(|controlled| {
                session
                    .player_entity_id
                    .as_deref()
                    .is_some_and(|player_id| controlled.player_entity_id == player_id)
            });
        let score = if is_local_controlled {
            4
        } else if is_interpolated {
            3
        } else if is_predicted {
            2
        } else if is_replicated {
            1
        } else {
            0
        };
        match best_root_entity_by_guid.get_mut(&guid.0) {
            Some((winner, winner_score)) => {
                if score > *winner_score
                    || (score == *winner_score && entity.to_bits() < winner.to_bits())
                {
                    *winner = entity;
                    *winner_score = score;
                }
            }
            None => {
                best_root_entity_by_guid.insert(guid.0, (entity, score));
            }
        }
    }
    let winner_root_entities =
        HashSet::<Entity>::from_iter(best_root_entity_by_guid.values().map(|(entity, _)| *entity));

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
        is_replicated,
        is_interpolated,
        is_predicted,
        confirmed_position,
        confirmed_rotation,
    ) in &entities
    {
        if mounted_on.is_none() && hardpoint.is_none() && !winner_root_entities.contains(&entity) {
            continue;
        }
        let world = global_transform.compute_transform();
        let replication_z_step = if is_predicted {
            PREDICTED_OVERLAY_Z_STEP
        } else if is_interpolated {
            INTERPOLATED_OVERLAY_Z_STEP
        } else if is_replicated {
            REPLICATED_OVERLAY_Z_STEP
        } else {
            0.0
        };
        let pos =
            world.translation + Vec3::new(0.0, 0.0, DEBUG_OVERLAY_Z_OFFSET + replication_z_step);
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
                let confirmed_pos = confirmed_position
                    .0
                    .0
                    .extend(DEBUG_OVERLAY_Z_OFFSET + CONFIRMED_OVERLAY_Z_STEP);
                let confirmed_rot: Quat = confirmed_rotation.0.into();
                let should_draw_confirmed = pos.distance(confirmed_pos)
                    > CONFIRMED_OVERLAY_POSITION_EPSILON_M
                    || rot.angle_between(confirmed_rot) > CONFIRMED_OVERLAY_ROTATION_EPSILON_RAD;
                if should_draw_confirmed {
                    for idx in 0..outline.points.len() {
                        let a = outline.points[idx];
                        let b = outline.points[(idx + 1) % outline.points.len()];
                        let world_a = confirmed_pos + (confirmed_rot * a.extend(0.0));
                        let world_b = confirmed_pos + (confirmed_rot * b.extend(0.0));
                        gizmos.line(world_a, world_b, controlled_confirmed_color);
                    }
                    gizmos.line(pos, confirmed_pos, prediction_error_color);
                }
            } else if is_local_controlled && mounted_on.is_none() && is_replicated && !is_predicted
            {
                // Sidereal can temporarily end up with only the confirmed replica visible while a
                // dynamic handoff is waiting for Lightyear to materialize the Predicted clone.
                // In that state there is no separate entity carrying Confirmed<T> wrappers, so the
                // normal predicted-vs-confirmed ghost comparison disappears. Draw the confirmed
                // shape anyway so debugging still shows the authoritative ghost lane explicitly.
                for idx in 0..outline.points.len() {
                    let a = outline.points[idx];
                    let b = outline.points[(idx + 1) % outline.points.len()];
                    let world_a = pos + (rot * a.extend(0.0));
                    let world_b = pos + (rot * b.extend(0.0));
                    gizmos.line(world_a, world_b, controlled_confirmed_color);
                }
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
                let confirmed_pos = confirmed_position
                    .0
                    .0
                    .extend(DEBUG_OVERLAY_Z_OFFSET + CONFIRMED_OVERLAY_Z_STEP);
                let should_draw_confirmed = pos.distance(confirmed_pos)
                    > CONFIRMED_OVERLAY_POSITION_EPSILON_M
                    || rot.angle_between(confirmed_rot) > CONFIRMED_OVERLAY_ROTATION_EPSILON_RAD;
                if should_draw_confirmed {
                    let confirmed_transform =
                        Transform::from_translation(confirmed_pos).with_rotation(confirmed_rot);
                    gizmos.aabb_3d(aabb, confirmed_transform, controlled_confirmed_color);
                    gizmos.line(pos, confirmed_pos, prediction_error_color);
                }
            } else if is_local_controlled && mounted_on.is_none() && is_replicated && !is_predicted
            {
                // Same confirmed-only fallback as the outline path above: keep the authoritative
                // ghost visible even when the runtime has not yet produced a distinct Predicted
                // clone for this control target.
                gizmos.aabb_3d(aabb, transform, controlled_confirmed_color);
            }
        }

        let _ = scanner_range;

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
            Has<lightyear::prelude::Interpolated>,
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
            Option<&'_ VisualCorrection<Isometry2d>>,
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
        is_interpolated_marker,
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
        visual_correction,
    )) = controlled_prediction_state.single()
    {
        let confirmed_tick_value = confirmed_tick.map(|tick| tick.tick.0);
        let confirmed_tick_advanced = confirmed_tick_value
            .zip(rollback_sample.last_confirmed_tick)
            .is_some_and(|(current, previous)| current != previous);
        let correction_translation_magnitude =
            visual_correction.map(|value| value.error.translation.length());
        let correction_rotation_rad =
            visual_correction.map(|value| value.error.rotation.as_radians());
        bevy::log::info!(
            "prediction controlled entity={} guid={} predicted_marker={} interpolated_marker={} confirmed_pos={} confirmed_rot={} confirmed_vel={} confirmed_tick={:?} confirmed_tick_advanced={} hist_pos={} hist_rot={} hist_vel={} current_pos={:?} current_rot_rad={:?} current_vel={:?} visual_correction_active={} visual_correction_translation_m={:?} visual_correction_rotation_rad={:?}",
            controlled_entity,
            guid.map(|v| v.0.to_string())
                .unwrap_or_else(|| "<none>".to_string()),
            is_predicted_marker,
            is_interpolated_marker,
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
            visual_correction.is_some(),
            correction_translation_magnitude,
            correction_rotation_rad,
        );
        if !is_predicted_marker && is_interpolated_marker {
            bevy::log::warn!(
                "prediction runtime anomaly: controlled entity {} is interpolated instead of predicted; local motion should stay disabled until a Predicted clone exists",
                controlled_entity
            );
        }
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
        if let Some(missing_predicted) = adoption_state
            .missing_predicted_control_entity_id
            .as_deref()
        {
            bevy::log::warn!(
                "prediction runtime waiting for predicted control clone for {}",
                missing_predicted
            );
        }
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn audit_prediction_entity_lifecycle(
    time: Res<'_, Time>,
    session: Res<'_, ClientSession>,
    player_view_state: Res<'_, LocalPlayerViewState>,
    config: Res<'_, PredictionLifecycleAuditConfig>,
    mut state: ResMut<'_, PredictionLifecycleAuditState>,
    world_entities: Query<
        '_,
        '_,
        (
            Entity,
            &'_ EntityGuid,
            Has<lightyear::prelude::Predicted>,
            Has<lightyear::prelude::Interpolated>,
            Has<ControlledEntity>,
            Has<super::components::SuppressedPredictedDuplicateVisual>,
            &'_ Visibility,
            Option<&'_ Transform>,
        ),
        With<WorldEntity>,
    >,
) {
    if !config.enabled {
        return;
    }
    let now = time.elapsed_secs_f64();
    if now - state.last_logged_at_s < config.interval_s {
        return;
    }
    state.last_logged_at_s = now;

    let target_guid = config.target_guid.or_else(|| {
        player_view_state
            .controlled_entity_id
            .as_deref()
            .and_then(sidereal_runtime_sync::parse_guid_from_entity_id)
            .or_else(|| {
                session
                    .player_entity_id
                    .as_deref()
                    .and_then(sidereal_runtime_sync::parse_guid_from_entity_id)
            })
    });
    let Some(target_guid) = target_guid else {
        return;
    };

    let mut lines = Vec::new();
    for (
        entity,
        guid,
        is_predicted,
        is_interpolated,
        is_controlled,
        is_suppressed,
        visibility,
        transform,
    ) in &world_entities
    {
        if guid.0 != target_guid {
            continue;
        }
        let pos = transform.map(|value| value.translation.truncate());
        lines.push(format!(
            "entity={entity:?} predicted={is_predicted} interpolated={is_interpolated} controlled={is_controlled} suppressed={is_suppressed} visibility={visibility:?} pos={pos:?}"
        ));
    }
    if lines.is_empty() {
        info!(
            "lifecycle_audit guid={} no runtime entity candidates",
            target_guid
        );
        return;
    }
    info!(
        "lifecycle_audit guid={} candidates={}",
        target_guid,
        lines.join(" | ")
    );
}
