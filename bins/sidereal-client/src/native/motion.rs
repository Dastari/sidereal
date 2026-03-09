//! Prediction input application and motion ownership enforcement.

use avian2d::prelude::*;
use bevy::ecs::query::Has;
use bevy::prelude::*;
use lightyear::interpolation::interpolation_history::ConfirmedHistory;
use lightyear::prelude::input::native::{ActionState, InputMarker};
use lightyear::prelude::is_in_rollback;
use sidereal_game::{
    ActionQueue, CollisionAabbM, CollisionOutlineM, CollisionProfile, EntityGuid,
    FlightControlAuthority, Hardpoint, MountedOn, PlayerTag, SimulationMotionWriter, SizeM,
    TotalMassKg, angular_inertia_from_size, collider_from_collision_shape,
    default_flight_action_capabilities,
};
use sidereal_net::PlayerInput;
use sidereal_runtime_sync::{RuntimeEntityHierarchy, parse_guid_from_entity_id};
use std::collections::HashSet;

use super::app_state::{ClientSession, LocalPlayerViewState};
use super::components::{
    ControlledEntity, NearbyCollisionProxy, SuppressedPredictedDuplicateVisual, WorldEntity,
};
use super::resources::{
    LocalSimulationDebugMode, MotionOwnershipAuditEnabled, MotionOwnershipAuditState,
    MotionOwnershipReconcileState, NearbyCollisionProxyTuning,
};

pub(crate) fn mark_motion_ownership_dirty_signals(
    session: Res<'_, ClientSession>,
    player_view_state: Res<'_, LocalPlayerViewState>,
    added_world_entities: Query<'_, '_, Entity, Added<WorldEntity>>,
    mut reconcile_state: ResMut<'_, MotionOwnershipReconcileState>,
) {
    if session.is_changed() || player_view_state.is_changed() || !added_world_entities.is_empty() {
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
            // Replace with current snapshot so we don't accumulate across ticks (same as server).
            queue.clear();
            for action in &action_state.0.actions {
                queue.push(*action);
            }
        } else {
            commands.entity(entity).insert((
                ActionQueue {
                    pending: action_state.0.actions.clone(),
                },
                default_flight_action_capabilities(),
            ));
        }
    }
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(crate) fn enforce_motion_ownership_for_world_entities(
    mut commands: Commands<'_, '_>,
    _local_mode: Res<'_, LocalSimulationDebugMode>,
    proxy_tuning: Res<'_, NearbyCollisionProxyTuning>,
    time: Res<'_, Time>,
    session: Res<'_, ClientSession>,
    player_view_state: Res<'_, LocalPlayerViewState>,
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
    let mut target_entity: Option<Entity> = None;
    let mut target_entity_score: i32 = -1;
    let is_player_anchor_target = session
        .player_entity_id
        .as_deref()
        .and_then(parse_guid_from_entity_id)
        .zip(Some(target_guid))
        .is_some_and(|(player_guid, control_guid)| player_guid == control_guid);
    let mut target_entity_is_predicted = false;
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
    let mut nearby_remote_candidates = Vec::<(Entity, f32)>::new();
    if let Some(target_position) = target_position {
        let max_dist_sq = proxy_tuning.radius_m * proxy_tuning.radius_m;
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
            let has_physics_data = size_m.is_some() && total_mass_kg.is_some_and(|m| m.0 > 0.0);
            let allow_collider = collision_profile
                .copied()
                .is_some_and(CollisionProfile::is_collidable);
            let has_rigidbody = rigidbody_markers.get(entity).is_ok();
            if has_physics_data && allow_collider && !has_rigidbody {
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

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(crate) fn audit_motion_ownership_system(
    time: Res<'_, Time>,
    enabled: Res<'_, MotionOwnershipAuditEnabled>,
    local_mode: Res<'_, LocalSimulationDebugMode>,
    session: Res<'_, ClientSession>,
    player_view_state: Res<'_, LocalPlayerViewState>,
    entity_registry: Res<'_, RuntimeEntityHierarchy>,
    mut audit_state: ResMut<'_, MotionOwnershipAuditState>,
    roots: Query<
        '_,
        '_,
        (
            Entity,
            Option<&'_ Name>,
            Option<&'_ MountedOn>,
            Option<&'_ Hardpoint>,
            Option<&'_ PlayerTag>,
            Has<lightyear::prelude::Predicted>,
            Has<lightyear::prelude::Interpolated>,
            Has<ActionQueue>,
            Has<FlightControlAuthority>,
            Has<RigidBody>,
            Has<NearbyCollisionProxy>,
            Has<Position>,
            Has<Rotation>,
            Has<LinearVelocity>,
        ),
        With<WorldEntity>,
    >,
) {
    if !enabled.0 {
        return;
    }
    let now_s = time.elapsed_secs_f64();
    if now_s - audit_state.last_logged_at_s < 0.5 {
        return;
    }
    audit_state.last_logged_at_s = now_s;

    let target_entity_id = match player_view_state.controlled_entity_id.as_ref() {
        Some(id) if entity_registry.by_entity_id.contains_key(id.as_str()) => Some(id),
        Some(_) => {
            warn!(
                controlled = ?player_view_state.controlled_entity_id,
                "motion audit: controlled entity unresolved in registry"
            );
            return;
        }
        None => session.player_entity_id.as_ref(),
    };
    let target_entity =
        target_entity_id.and_then(|id| entity_registry.by_entity_id.get(id.as_str()).copied());

    let mut anomalies = Vec::new();
    for (
        entity,
        name,
        mounted_on,
        hardpoint,
        player_tag,
        is_predicted,
        is_interpolated,
        has_action_queue,
        has_flight_control_authority,
        has_rigidbody,
        has_nearby_proxy,
        has_position,
        has_rotation,
        has_linear_velocity,
    ) in &roots
    {
        let is_root_entity = mounted_on.is_none() && hardpoint.is_none() && player_tag.is_none();
        if !is_root_entity {
            continue;
        }
        let entity_name = name
            .map(|n| n.as_str().to_string())
            .unwrap_or_else(|| format!("<entity:{entity:?}>"));
        let is_target = Some(entity) == target_entity;

        if is_target && !local_mode.0 {
            if !is_predicted || is_interpolated {
                anomalies.push(format!(
                    "{} target markers invalid predicted={} interpolated={}",
                    entity_name, is_predicted, is_interpolated
                ));
            }
            if !has_rigidbody || !has_position || !has_rotation || !has_linear_velocity {
                anomalies.push(format!(
                    "{} target motion components missing rb={} pos={} rot={} vel={}",
                    entity_name, has_rigidbody, has_position, has_rotation, has_linear_velocity
                ));
            }
            continue;
        }

        if is_predicted
            || has_action_queue
            || has_flight_control_authority
            || (has_rigidbody && !has_nearby_proxy)
        {
            anomalies.push(format!(
                "{} remote writers present predicted={} action_queue={} flight_authority={} rb={} nearby_proxy={}",
                entity_name,
                is_predicted,
                has_action_queue,
                has_flight_control_authority,
                has_rigidbody,
                has_nearby_proxy
            ));
        }
    }

    if !anomalies.is_empty() {
        warn!(
            "motion ownership audit anomalies ({}): {}",
            anomalies.len(),
            anomalies.join(" | ")
        );
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
            pos.0 = Vec2::ZERO;
        }
        if let Some(mut vel) = velocity
            && !vel.0.is_finite()
        {
            vel.0 = Vec2::ZERO;
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
