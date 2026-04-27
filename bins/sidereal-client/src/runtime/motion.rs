//! Prediction input application and motion ownership enforcement.

use avian2d::prelude::*;
use bevy::ecs::query::Has;
use bevy::{math::DVec2, prelude::*};
use lightyear::frame_interpolation::FrameInterpolate;
use lightyear::interpolation::interpolation_history::ConfirmedHistory;
use lightyear::prediction::prelude::PredictionHistory;
use lightyear::prelude::input::native::{ActionState, InputMarker};
use lightyear::prelude::is_in_rollback;
use lightyear::prelude::{Confirmed, ConfirmedTick, LocalTimeline};
use sidereal_game::{
    ActionQueue, CollisionAabbM, CollisionOutlineM, CollisionProfile, EntityGuid,
    FlightControlAuthority, Hardpoint, MountedOn, PlayerTag, SimulationMotionWriter, SizeM,
    TotalMassKg, angular_inertia_from_size, collider_from_collision_shape,
    default_flight_action_capabilities,
};
use sidereal_net::{PlayerInput, replace_action_queue_from_player_input};
use sidereal_runtime_sync::parse_guid_from_entity_id;
use std::collections::HashSet;

use super::app_state::{ClientSession, LocalPlayerViewState};
use super::components::{
    ControlledEntity, NearbyCollisionProxy, PredictedMotionBootstrapSeed,
    SuppressedPredictedDuplicateVisual, WorldEntity,
};
use super::resources::{
    ControlBootstrapPhase, ControlBootstrapState, MotionOwnershipReconcileState,
    NearbyCollisionProxyTuning,
};

#[allow(clippy::type_complexity)]
pub(crate) fn mark_motion_ownership_dirty_signals(
    session: Res<'_, ClientSession>,
    player_view_state: Res<'_, LocalPlayerViewState>,
    added_world_entities: Query<'_, '_, Entity, Added<WorldEntity>>,
    changed_physics_setup: Query<
        '_,
        '_,
        Entity,
        (
            With<WorldEntity>,
            Or<(
                Changed<SizeM>,
                Changed<TotalMassKg>,
                Changed<CollisionProfile>,
                Changed<CollisionAabbM>,
                Changed<CollisionOutlineM>,
            )>,
        ),
    >,
    mut reconcile_state: ResMut<'_, MotionOwnershipReconcileState>,
) {
    if session.is_changed()
        || player_view_state.is_changed()
        || !added_world_entities.is_empty()
        || !changed_physics_setup.is_empty()
    {
        reconcile_state.dirty = true;
    }
}

/// Translates the Lightyear-managed `ActionState<PlayerInput>` into `ActionQueue`
/// entries each `FixedUpdate` tick. This runs during normal simulation and during
/// rollback resimulation so the flight systems always see the correct input.
#[allow(clippy::type_complexity)]
pub(crate) fn apply_predicted_input_to_action_queue(
    mut commands: Commands<'_, '_>,
    mut query: Query<
        '_,
        '_,
        (Entity, &ActionState<PlayerInput>, Option<&mut ActionQueue>),
        (With<SimulationMotionWriter>, With<InputMarker<PlayerInput>>),
    >,
) {
    for (entity, action_state, maybe_queue) in &mut query {
        if let Some(mut queue) = maybe_queue {
            replace_action_queue_from_player_input(&mut queue, &action_state.0);
        } else {
            let mut queue = ActionQueue::default();
            replace_action_queue_from_player_input(&mut queue, &action_state.0);
            commands
                .entity(entity)
                .insert((queue, default_flight_action_capabilities()));
        }
    }
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(crate) fn seed_controlled_predicted_motion_from_confirmed(
    mut commands: Commands<'_, '_>,
    timeline: Res<'_, LocalTimeline>,
    control_bootstrap_state: Res<'_, ControlBootstrapState>,
    rollback_query: Query<'_, '_, (), With<lightyear::prelude::Rollback>>,
    mut query: Query<
        '_,
        '_,
        (
            Entity,
            Option<&'_ EntityGuid>,
            &'_ Confirmed<Position>,
            Option<&'_ Confirmed<Rotation>>,
            Option<&'_ Confirmed<LinearVelocity>>,
            Option<&'_ Confirmed<AngularVelocity>>,
            Option<&'_ ConfirmedTick>,
            Option<&'_ mut Position>,
            Option<&'_ mut Rotation>,
            Option<&'_ mut LinearVelocity>,
            Option<&'_ mut AngularVelocity>,
            Option<&'_ mut Transform>,
            Option<&'_ mut FrameInterpolate<Transform>>,
            Option<&'_ PredictedMotionBootstrapSeed>,
        ),
        (
            With<ControlledEntity>,
            With<SimulationMotionWriter>,
            With<lightyear::prelude::Predicted>,
        ),
    >,
    mut histories: Query<
        '_,
        '_,
        (
            Option<&'_ mut PredictionHistory<Position>>,
            Option<&'_ mut PredictionHistory<Rotation>>,
            Option<&'_ mut PredictionHistory<LinearVelocity>>,
            Option<&'_ mut PredictionHistory<AngularVelocity>>,
        ),
    >,
) {
    if is_in_rollback(rollback_query) {
        return;
    }
    let ControlBootstrapPhase::ActivePredicted {
        generation,
        entity: active_entity,
        ..
    } = control_bootstrap_state.phase
    else {
        return;
    };
    let Ok((
        entity,
        guid,
        confirmed_position,
        confirmed_rotation,
        confirmed_linear_velocity,
        confirmed_angular_velocity,
        confirmed_tick,
        position,
        rotation,
        linear_velocity,
        angular_velocity,
        transform,
        frame_interpolate,
        seed,
    )) = query.get_mut(active_entity)
    else {
        return;
    };
    if seed.is_some_and(|seed| seed.generation == generation) {
        return;
    }

    let position_value = confirmed_position.0;
    let rotation_value = confirmed_rotation
        .map(|value| value.0)
        .unwrap_or(Rotation::IDENTITY);
    let linear_velocity_value = confirmed_linear_velocity
        .map(|value| value.0)
        .unwrap_or(LinearVelocity::ZERO);
    let angular_velocity_value = confirmed_angular_velocity
        .map(|value| value.0)
        .unwrap_or(AngularVelocity::ZERO);

    if let Some(mut position) = position {
        *position = position_value;
    } else {
        commands.entity(entity).insert(position_value);
    }
    if let Some(mut rotation) = rotation {
        *rotation = rotation_value;
    } else {
        commands.entity(entity).insert(rotation_value);
    }
    if let Some(mut linear_velocity) = linear_velocity {
        *linear_velocity = linear_velocity_value;
    } else {
        commands.entity(entity).insert(linear_velocity_value);
    }
    if let Some(mut angular_velocity) = angular_velocity {
        *angular_velocity = angular_velocity_value;
    } else {
        commands.entity(entity).insert(angular_velocity_value);
    }

    let seeded_transform = Transform {
        translation: Vec3::new(position_value.0.x as f32, position_value.0.y as f32, 0.0),
        rotation: Quat::from_rotation_z(rotation_value.as_radians() as f32),
        ..Default::default()
    };
    if let Some(mut transform) = transform {
        *transform = seeded_transform;
    } else {
        commands.entity(entity).insert(seeded_transform);
    }
    if let Some(mut frame_interpolate) = frame_interpolate {
        frame_interpolate.previous_value = Some(seeded_transform);
        frame_interpolate.current_value = Some(seeded_transform);
    }

    let current_tick = timeline.tick();
    let seed_tick = confirmed_tick
        .map(|confirmed_tick| confirmed_tick.tick)
        .filter(|tick| *tick <= current_tick)
        .unwrap_or(current_tick);
    if let Ok((
        position_history,
        rotation_history,
        linear_velocity_history,
        angular_velocity_history,
    )) = histories.get_mut(entity)
    {
        if let Some(mut history) = position_history {
            seed_prediction_history(&mut history, seed_tick, current_tick, position_value);
        } else {
            commands.entity(entity).insert(new_prediction_history(
                seed_tick,
                current_tick,
                position_value,
            ));
        }
        if let Some(mut history) = rotation_history {
            seed_prediction_history(&mut history, seed_tick, current_tick, rotation_value);
        } else {
            commands.entity(entity).insert(new_prediction_history(
                seed_tick,
                current_tick,
                rotation_value,
            ));
        }
        if let Some(mut history) = linear_velocity_history {
            seed_prediction_history(&mut history, seed_tick, current_tick, linear_velocity_value);
        } else {
            commands.entity(entity).insert(new_prediction_history(
                seed_tick,
                current_tick,
                linear_velocity_value,
            ));
        }
        if let Some(mut history) = angular_velocity_history {
            seed_prediction_history(
                &mut history,
                seed_tick,
                current_tick,
                angular_velocity_value,
            );
        } else {
            commands.entity(entity).insert(new_prediction_history(
                seed_tick,
                current_tick,
                angular_velocity_value,
            ));
        }
    }
    commands
        .entity(entity)
        .insert(PredictedMotionBootstrapSeed { generation });

    bevy::log::info!(
        entity = ?entity,
        guid = ?guid.map(|guid| guid.0),
        generation,
        confirmed_tick = ?confirmed_tick.map(|tick| tick.tick),
        current_tick = ?current_tick,
        position = ?position_value.0,
        rotation_rad = rotation_value.as_radians(),
        "seeded controlled predicted motion from confirmed state"
    );
}

fn new_prediction_history<C: Component + Clone>(
    seed_tick: lightyear::prelude::Tick,
    current_tick: lightyear::prelude::Tick,
    value: C,
) -> PredictionHistory<C> {
    let mut history = PredictionHistory::<C>::default();
    seed_prediction_history(&mut history, seed_tick, current_tick, value);
    history
}

fn seed_prediction_history<C: Component + Clone>(
    history: &mut PredictionHistory<C>,
    seed_tick: lightyear::prelude::Tick,
    current_tick: lightyear::prelude::Tick,
    value: C,
) {
    history.clear();
    history.add_update(seed_tick, value.clone());
    if current_tick != seed_tick {
        history.add_update(current_tick, value);
    }
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(crate) fn enforce_motion_ownership_for_world_entities(
    mut commands: Commands<'_, '_>,
    proxy_tuning: Res<'_, NearbyCollisionProxyTuning>,
    time: Res<'_, Time>,
    session: Res<'_, ClientSession>,
    player_view_state: Res<'_, LocalPlayerViewState>,
    control_bootstrap_state: Res<'_, ControlBootstrapState>,
    mut reconcile_state: ResMut<'_, MotionOwnershipReconcileState>,
    rollback_query: Query<'_, '_, (), With<lightyear::prelude::Rollback>>,
    collision_aabbs: Query<'_, '_, &'_ CollisionAabbM>,
    collision_outlines: Query<'_, '_, &'_ CollisionOutlineM>,
    rigidbody_markers: Query<'_, '_, (), With<RigidBody>>,
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
            Option<&'_ SizeM>,
            Option<&'_ TotalMassKg>,
            Option<&'_ CollisionProfile>,
            Option<&'_ ConfirmedHistory<Position>>,
            Option<&'_ ConfirmedHistory<Rotation>>,
            Has<SuppressedPredictedDuplicateVisual>,
            Has<lightyear::prelude::Predicted>,
            Has<lightyear::prelude::Interpolated>,
        ),
        (With<EntityGuid>, Without<Camera>),
    >,
    mut missing_predicted_warn_at_s: Local<'_, f64>,
) {
    if is_in_rollback(rollback_query) {
        return;
    }
    let now_s = time.elapsed_secs_f64();
    let proxy_refresh_due = proxy_tuning.max_proxies > 0
        && (now_s - reconcile_state.last_reconcile_at_s)
            >= proxy_tuning.reconcile_interval_s.max(0.01);
    if !reconcile_state.dirty && !proxy_refresh_due {
        return;
    }
    let target_control_id = player_view_state
        .controlled_entity_id
        .as_ref()
        .or(session.player_entity_id.as_ref())
        .cloned();
    let target_guid = target_control_id
        .as_deref()
        .and_then(parse_guid_from_entity_id)
        .or_else(|| {
            target_control_id
                .as_deref()
                .and_then(|id| uuid::Uuid::parse_str(id).ok())
        });

    let Some(target_guid) = target_guid else {
        // Control target not resolved yet (bootstrap/handoff). Avoid destructive stripping.
        return;
    };
    let mut target_entity: Option<Entity> = match &control_bootstrap_state.phase {
        ControlBootstrapPhase::ActivePredicted { entity, .. } => Some(*entity),
        _ => None,
    };
    let mut target_entity_score: i32 = if target_entity.is_some() {
        i32::MAX
    } else {
        -1
    };
    let is_player_anchor_target = matches!(
        control_bootstrap_state.phase,
        ControlBootstrapPhase::ActiveAnchor { .. }
    ) || session
        .player_entity_id
        .as_deref()
        .and_then(parse_guid_from_entity_id)
        .zip(Some(target_guid))
        .is_some_and(|(player_guid, control_guid)| player_guid == control_guid);
    let mut target_entity_is_predicted = matches!(
        control_bootstrap_state.phase,
        ControlBootstrapPhase::ActivePredicted { .. }
    );
    for (
        candidate_entity,
        _,
        mounted_on,
        hardpoint,
        player_tag,
        guid,
        _position,
        _size_m,
        _total_mass_kg,
        _collision_profile,
        _position_history,
        _rotation_history,
        _is_suppressed,
        is_predicted,
        is_interpolated,
    ) in &root_world_entities
    {
        let is_player_anchor =
            mounted_on.is_none() && hardpoint.is_none() && player_tag.is_some() && guid.is_some();
        let is_root_entity =
            mounted_on.is_none() && hardpoint.is_none() && player_tag.is_none() && guid.is_some();
        let is_control_target_candidate = is_root_entity || is_player_anchor;
        if !is_control_target_candidate || guid.is_none_or(|g| g.0 != target_guid) {
            continue;
        }
        let score = if is_predicted {
            3
        } else if is_interpolated {
            2
        } else {
            1
        };
        if score > target_entity_score
            || (score == target_entity_score
                && target_entity.is_none_or(|winner| candidate_entity.to_bits() < winner.to_bits()))
        {
            target_entity = Some(candidate_entity);
            target_entity_score = score;
            target_entity_is_predicted = is_predicted;
        }
    }
    let Some(target_entity) = target_entity else {
        return;
    };
    if !is_player_anchor_target && !target_entity_is_predicted {
        // Sidereal's dynamic handoff means the desired control GUID can resolve before Lightyear
        // has spawned the Predicted clone. Do not promote a confirmed/interpolated ship into the
        // local motion-writer lane: that creates a second simulation writer and makes the runtime
        // feel "jerky" instead of truly predicted.
        if now_s - *missing_predicted_warn_at_s >= 1.0 {
            bevy::log::warn!(
                "motion ownership waiting for Predicted clone for control target {} before enabling local physics writes",
                target_guid
            );
            *missing_predicted_warn_at_s = now_s;
        }
        return;
    }

    let target_position = root_world_entities.iter().find_map(
        |(entity, _, mounted_on, hardpoint, player_tag, _, position, ..)| {
            if entity != target_entity
                || mounted_on.is_some()
                || hardpoint.is_some()
                || player_tag.is_some()
            {
                return None;
            }
            position.map(|p| p.0)
        },
    );
    let mut nearby_remote_candidates = Vec::<(Entity, f64)>::new();
    if let Some(target_position) = target_position {
        let max_dist_sq = f64::from(proxy_tuning.radius_m * proxy_tuning.radius_m);
        for (
            entity,
            controlled,
            mounted_on,
            hardpoint,
            player_tag,
            guid,
            position,
            _size_m,
            _total_mass_kg,
            _collision_profile,
            _position_history,
            _rotation_history,
            is_suppressed,
            _is_predicted,
            _is_interpolated,
        ) in &root_world_entities
        {
            let is_root_entity = mounted_on.is_none()
                && hardpoint.is_none()
                && player_tag.is_none()
                && guid.is_some();
            if !is_root_entity || controlled.is_some() || entity == target_entity || is_suppressed {
                continue;
            }
            if guid.is_some_and(|guid| guid.0 == target_guid) {
                // Never keep a nearby proxy for logical duplicates of the locally controlled entity.
                // Duplicate local copies can collide and create client-only velocity drift.
                continue;
            }
            let Some(remote_pos) = position.map(|p| p.0) else {
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
        _position,
        size_m,
        total_mass_kg,
        collision_profile,
        position_history,
        rotation_history,
        is_suppressed,
        _is_predicted,
        is_interpolated,
    ) in &root_world_entities
    {
        if entity == target_entity {
            if is_suppressed {
                commands
                    .entity(entity)
                    .remove::<SuppressedPredictedDuplicateVisual>();
            }
            let mut entity_commands = commands.entity(entity);
            let is_player_anchor = mounted_on.is_none()
                && hardpoint.is_none()
                && player_tag.is_some()
                && _guid.is_some();
            if is_player_anchor {
                // Free-roam routes input to the local player anchor (non-physics).
                // Keep ActionQueue on this entity so character prediction can run.
                entity_commands.remove::<NearbyCollisionProxy>();
                entity_commands.remove::<FlightControlAuthority>();
                entity_commands.remove::<(
                    RigidBody,
                    Collider,
                    Mass,
                    AngularInertia,
                    LockedAxes,
                    LinearDamping,
                    AngularDamping,
                )>();
                continue;
            }
            entity_commands.insert(FlightControlAuthority);
            let has_physics_data = size_m.is_some() && total_mass_kg.is_some_and(|m| m.0 > 0.0);
            let allow_collider = collision_profile
                .copied()
                .is_some_and(CollisionProfile::is_collidable);
            let has_rigidbody = rigidbody_markers.get(entity).is_ok();
            if has_physics_data && allow_collider {
                let size = size_m.copied().unwrap();
                let mass_kg = total_mass_kg.map(|m| m.0).unwrap();
                let collider = collider_from_collision_shape(
                    &size,
                    collision_aabbs.get(entity).ok(),
                    collision_outlines.get(entity).ok(),
                );
                entity_commands.insert((
                    RigidBody::Dynamic,
                    collider,
                    Mass(mass_kg),
                    angular_inertia_from_size(mass_kg, &size),
                    LinearDamping(0.0),
                    AngularDamping(0.0),
                ));
            } else if has_physics_data && !allow_collider && has_rigidbody {
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
            entity_commands.remove::<NearbyCollisionProxy>();
            continue;
        }

        if controlled.is_some() {
            commands.entity(entity).remove::<ControlledEntity>();
        }
        commands.entity(entity).remove::<FlightControlAuthority>();

        let is_root_entity =
            mounted_on.is_none() && hardpoint.is_none() && player_tag.is_none() && _guid.is_some();
        if !is_root_entity {
            continue;
        }
        if is_suppressed {
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
            continue;
        }

        let keep_nearby_proxy = nearby_proxy_entities.contains(&entity);
        if keep_nearby_proxy {
            let interpolation_history_ready = position_history.and_then(|h| h.end()).is_some()
                && rotation_history.and_then(|h| h.end()).is_some();
            if is_interpolated && !interpolation_history_ready {
                // Lightyear Avian explicitly warns against letting local physics bootstrap
                // interpolated entities before their observer spatial history exists: Avian can
                // seed default Position/Rotation/Transform at the origin, which is exactly the
                // "remote ship appears at 0,0 until it moves" bug Sidereal was hitting.
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
                continue;
            }
            let allow_collider = collision_profile
                .copied()
                .is_some_and(CollisionProfile::is_collidable);
            if let Some(size) = size_m
                && allow_collider
            {
                let collider = collider_from_collision_shape(
                    size,
                    collision_aabbs.get(entity).ok(),
                    collision_outlines.get(entity).ok(),
                );
                let mut entity_commands = commands.entity(entity);
                // Force nearby proxies to kinematic every tick so previously controlled
                // dynamic bodies cannot keep integrating locally after control handoff.
                entity_commands.insert(RigidBody::Kinematic);
                entity_commands.insert(collider);
                entity_commands.insert(NearbyCollisionProxy);
                // Kinematic collision proxy should not carry local dynamic mass/inertia writers.
                entity_commands.remove::<(Mass, AngularInertia)>();
            } else {
                commands
                    .entity(entity)
                    .remove::<(RigidBody, Collider, Mass, AngularInertia)>();
            }
            // No SizeM: physics does not apply; leave entity without RigidBody/Collider.
        } else {
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
    }
    reconcile_state.last_target_guid = Some(target_guid);
    reconcile_state.last_target_entity = Some(target_entity);
    reconcile_state.last_reconcile_at_s = now_s;
    reconcile_state.dirty = false;
}

#[allow(clippy::type_complexity)]
pub(crate) fn sync_controlled_mass_from_total_mass(
    mut controlled: Query<
        '_,
        '_,
        (
            &'_ TotalMassKg,
            Option<&'_ SizeM>,
            Option<&'_ mut Mass>,
            Option<&'_ mut AngularInertia>,
        ),
        With<ControlledEntity>,
    >,
) {
    for (total_mass, size, maybe_mass, maybe_inertia) in &mut controlled {
        let computed_total = total_mass.0.max(1.0);
        if let Some(mut mass) = maybe_mass {
            *mass = Mass(computed_total);
        }
        if let (Some(mut inertia), Some(size)) = (maybe_inertia, size) {
            *inertia = angular_inertia_from_size(computed_total, size);
        }
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn enforce_controlled_planar_motion(
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
            pos.0 = DVec2::ZERO;
        }
        if let Some(mut vel) = velocity
            && !vel.0.is_finite()
        {
            vel.0 = DVec2::ZERO;
        }
        if let Some(mut ang_vel) = angular_velocity
            && !ang_vel.0.is_finite()
        {
            ang_vel.0 = 0.0;
        }
        if !transform.translation.is_finite() {
            transform.translation = Vec3::ZERO;
        }
        if let Some(mut rot) = rotation
            && !rot.is_finite()
        {
            *rot = Rotation::IDENTITY;
        }
        if transform.translation.z != 0.0 {
            transform.translation.z = 0.0;
        }
        if !transform.rotation.is_finite() {
            transform.rotation = Quat::IDENTITY;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        enforce_motion_ownership_for_world_entities,
        seed_controlled_predicted_motion_from_confirmed,
    };
    use crate::runtime::app_state::{ClientSession, LocalPlayerViewState};
    use crate::runtime::components::{ControlledEntity, PredictedMotionBootstrapSeed, WorldEntity};
    use crate::runtime::resources::{
        ControlBootstrapPhase, ControlBootstrapState, MotionOwnershipReconcileState,
        NearbyCollisionProxyTuning,
    };
    use avian2d::prelude::{
        AngularInertia, AngularVelocity, LinearVelocity, Mass, Position, RigidBody, Rotation,
    };
    use bevy::app::Update;
    use bevy::prelude::{App, Time, Transform, Vec2};
    use lightyear::frame_interpolation::FrameInterpolate;
    use lightyear::prediction::prelude::PredictionHistory;
    use lightyear::prelude::{Confirmed, ConfirmedTick, LocalTimeline, Tick};
    use sidereal_game::{
        CollisionProfile, EntityGuid, FlightControlAuthority, SimulationMotionWriter, SizeM,
        TotalMassKg,
    };
    use uuid::Uuid;

    #[test]
    fn predicted_controlled_ship_gets_client_flight_authority() {
        let mut app = App::new();
        let player_id = "1521601b-7e69-4700-853f-eb1eb3a41199".to_string();
        let target_id = "ce9e421c-8b62-458a-803e-51e9ad272908".to_string();
        let stale_id = "c2f492d0-1000-4e11-b43e-729f2b2ea178".to_string();
        let target_guid = Uuid::parse_str(&target_id).unwrap();
        let stale_guid = Uuid::parse_str(&stale_id).unwrap();

        let target_entity = app
            .world_mut()
            .spawn((
                WorldEntity,
                EntityGuid(target_guid),
                Position::default(),
                Rotation::default(),
                lightyear::prelude::Predicted,
            ))
            .id();
        let stale_entity = app
            .world_mut()
            .spawn((
                WorldEntity,
                EntityGuid(stale_guid),
                Position::default(),
                Rotation::default(),
                FlightControlAuthority,
                ControlledEntity {
                    entity_id: stale_id,
                    player_entity_id: player_id.clone(),
                },
            ))
            .id();

        app.insert_resource(Time::<()>::default());
        app.insert_resource(ClientSession {
            player_entity_id: Some(player_id),
            ..Default::default()
        });
        app.insert_resource(LocalPlayerViewState {
            controlled_entity_id: Some(target_id.clone()),
            ..Default::default()
        });
        app.insert_resource(ControlBootstrapState {
            phase: ControlBootstrapPhase::ActivePredicted {
                target_entity_id: target_id.clone(),
                generation: 1,
                entity: target_entity,
            },
            authoritative_target_entity_id: Some(target_id),
            generation: 1,
            ..Default::default()
        });
        app.insert_resource(NearbyCollisionProxyTuning {
            radius_m: 0.0,
            max_proxies: 0,
            reconcile_interval_s: 1.0,
        });
        app.insert_resource(MotionOwnershipReconcileState {
            dirty: true,
            ..Default::default()
        });
        app.add_systems(Update, enforce_motion_ownership_for_world_entities);

        app.update();

        assert!(
            app.world()
                .get::<FlightControlAuthority>(target_entity)
                .is_some()
        );
        assert!(
            app.world()
                .get::<FlightControlAuthority>(stale_entity)
                .is_none()
        );
        assert!(app.world().get::<ControlledEntity>(stale_entity).is_none());
    }

    #[test]
    fn predicted_controlled_ship_repairs_missing_mass_and_inertia_on_existing_rigidbody() {
        let mut app = App::new();
        let player_id = "1521601b-7e69-4700-853f-eb1eb3a41199".to_string();
        let target_id = "ce9e421c-8b62-458a-803e-51e9ad272908".to_string();
        let target_guid = Uuid::parse_str(&target_id).unwrap();

        let target_entity = app
            .world_mut()
            .spawn((
                WorldEntity,
                EntityGuid(target_guid),
                Position::default(),
                Rotation::default(),
                RigidBody::Dynamic,
                SizeM {
                    length: 12.0,
                    width: 6.0,
                    height: 2.0,
                },
                TotalMassKg(480.0),
                CollisionProfile::solid_aabb(),
                lightyear::prelude::Predicted,
            ))
            .id();

        app.insert_resource(Time::<()>::default());
        app.insert_resource(ClientSession {
            player_entity_id: Some(player_id),
            ..Default::default()
        });
        app.insert_resource(LocalPlayerViewState {
            controlled_entity_id: Some(target_id.clone()),
            ..Default::default()
        });
        app.insert_resource(ControlBootstrapState {
            phase: ControlBootstrapPhase::ActivePredicted {
                target_entity_id: target_id.clone(),
                generation: 1,
                entity: target_entity,
            },
            authoritative_target_entity_id: Some(target_id),
            generation: 1,
            ..Default::default()
        });
        app.insert_resource(NearbyCollisionProxyTuning {
            radius_m: 0.0,
            max_proxies: 0,
            reconcile_interval_s: 1.0,
        });
        app.insert_resource(MotionOwnershipReconcileState {
            dirty: true,
            ..Default::default()
        });
        app.add_systems(Update, enforce_motion_ownership_for_world_entities);

        app.update();

        assert_eq!(app.world().get::<Mass>(target_entity), Some(&Mass(480.0)));
        assert!(
            app.world()
                .get::<AngularInertia>(target_entity)
                .is_some_and(|inertia| inertia.0 > 1.0)
        );
    }

    #[test]
    fn predicted_controlled_motion_seeds_from_confirmed_on_control_generation() {
        let mut app = App::new();
        let target_id = "ce9e421c-8b62-458a-803e-51e9ad272908".to_string();
        let target_guid = Uuid::parse_str(&target_id).unwrap();
        let target_entity = app
            .world_mut()
            .spawn((
                WorldEntity,
                EntityGuid(target_guid),
                ControlledEntity {
                    entity_id: target_id.clone(),
                    player_entity_id: "1521601b-7e69-4700-853f-eb1eb3a41199".to_string(),
                },
                SimulationMotionWriter,
                lightyear::prelude::Predicted,
                Position(Vec2::ZERO.into()),
                Rotation::IDENTITY,
                LinearVelocity(Vec2::ZERO.into()),
                AngularVelocity(0.0),
            ))
            .id();
        app.world_mut().entity_mut(target_entity).insert((
            Confirmed(Position(Vec2::new(100.0, -25.0).into())),
            Confirmed(Rotation::radians(0.75)),
            Confirmed(LinearVelocity(Vec2::new(6.0, -2.0).into())),
            Confirmed(AngularVelocity(0.25)),
            ConfirmedTick { tick: Tick(40) },
            PredictionHistory::<Position>::default(),
            PredictionHistory::<Rotation>::default(),
            PredictionHistory::<LinearVelocity>::default(),
            PredictionHistory::<AngularVelocity>::default(),
            Transform::default(),
            FrameInterpolate::<Transform>::default(),
        ));
        let mut timeline = LocalTimeline::default();
        timeline.apply_delta(50);
        app.insert_resource(timeline);
        app.insert_resource(ControlBootstrapState {
            authoritative_target_entity_id: Some(target_id.clone()),
            generation: 7,
            phase: ControlBootstrapPhase::ActivePredicted {
                target_entity_id: target_id,
                generation: 7,
                entity: target_entity,
            },
            last_transition_at_s: 0.0,
        });
        app.add_systems(Update, seed_controlled_predicted_motion_from_confirmed);

        app.update();

        let entity = app.world().entity(target_entity);
        assert_eq!(
            entity.get::<Position>().map(|value| value.0),
            Some(Vec2::new(100.0, -25.0).into())
        );
        assert_eq!(
            entity
                .get::<Rotation>()
                .map(|rotation| rotation.as_radians()),
            Some(0.75)
        );
        assert_eq!(
            entity.get::<LinearVelocity>().map(|value| value.0),
            Some(Vec2::new(6.0, -2.0).into())
        );
        assert_eq!(
            entity.get::<AngularVelocity>().map(|value| value.0),
            Some(0.25)
        );
        let transform = entity.get::<Transform>().expect("transform");
        assert_eq!(transform.translation.x, 100.0);
        assert_eq!(transform.translation.y, -25.0);
        let frame = entity
            .get::<FrameInterpolate<Transform>>()
            .expect("frame interpolation");
        assert!(frame.previous_value.is_some());
        assert!(frame.current_value.is_some());
        assert_eq!(
            entity.get::<PredictedMotionBootstrapSeed>(),
            Some(&PredictedMotionBootstrapSeed { generation: 7 })
        );
        assert_eq!(
            entity
                .get::<PredictionHistory<Position>>()
                .expect("position history")
                .len(),
            2
        );
    }

    #[test]
    fn predicted_controlled_motion_seed_does_not_repeat_same_generation() {
        let mut app = App::new();
        let target_id = "ce9e421c-8b62-458a-803e-51e9ad272908".to_string();
        let target_guid = Uuid::parse_str(&target_id).unwrap();
        let target_entity = app
            .world_mut()
            .spawn((
                EntityGuid(target_guid),
                ControlledEntity {
                    entity_id: target_id.clone(),
                    player_entity_id: "1521601b-7e69-4700-853f-eb1eb3a41199".to_string(),
                },
                SimulationMotionWriter,
                lightyear::prelude::Predicted,
                PredictedMotionBootstrapSeed { generation: 3 },
                Position(Vec2::new(5.0, 6.0).into()),
                Confirmed(Position(Vec2::new(100.0, -25.0).into())),
            ))
            .id();
        app.insert_resource(LocalTimeline::default());
        app.insert_resource(ControlBootstrapState {
            authoritative_target_entity_id: Some(target_id.clone()),
            generation: 3,
            phase: ControlBootstrapPhase::ActivePredicted {
                target_entity_id: target_id,
                generation: 3,
                entity: target_entity,
            },
            last_transition_at_s: 0.0,
        });
        app.add_systems(Update, seed_controlled_predicted_motion_from_confirmed);

        app.update();

        assert_eq!(
            app.world()
                .get::<Position>(target_entity)
                .map(|value| value.0),
            Some(Vec2::new(5.0, 6.0).into())
        );
    }
}
