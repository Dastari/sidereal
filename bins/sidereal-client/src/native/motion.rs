//! Prediction input application, motion ownership enforcement, and reconciliation.

use avian2d::prelude::*;
use bevy::ecs::query::Has;
use bevy::prelude::*;
use lightyear::prelude::input::native::ActionState;
use sidereal_game::{
    ActionQueue, ControlledEntityGuid, EntityGuid, FlightComputer, Hardpoint, MountedOn, PlayerTag,
    SizeM, TotalMassKg, angular_inertia_from_size, default_flight_action_capabilities,
};
use sidereal_net::PlayerInput;
use sidereal_runtime_sync::RuntimeEntityHierarchy;
use std::collections::HashSet;
use std::sync::OnceLock;

use super::app_state::{ClientSession, LocalPlayerViewState};
use super::components::{
    ControlledEntity, NearbyCollisionProxy, SuppressedPredictedDuplicateVisual, WorldEntity,
};
use super::resources::{
    LocalSimulationDebugMode, MotionOwnershipAuditEnabled, MotionOwnershipAuditState,
    NearbyCollisionProxyTuning,
};

fn prediction_reconcile_debug_logging_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("SIDEREAL_DEBUG_PREDICTION_RECONCILE")
            .ok()
            .is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"))
    })
}

#[derive(Default)]
pub(crate) struct PredictionReconcileLogState {
    last_logged_at_s: f64,
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
        With<ControlledEntity>,
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
    local_mode: Res<'_, LocalSimulationDebugMode>,
    proxy_tuning: Res<'_, NearbyCollisionProxyTuning>,
    session: Res<'_, ClientSession>,
    player_view_state: Res<'_, LocalPlayerViewState>,
    entity_registry: Res<'_, RuntimeEntityHierarchy>,
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
            Option<&'_ Transform>,
            Option<&'_ Rotation>,
            Option<&'_ LinearVelocity>,
            Option<&'_ SizeM>,
            Option<&'_ TotalMassKg>,
            Has<ControlledEntityGuid>,
            Has<RigidBody>,
            Has<SuppressedPredictedDuplicateVisual>,
        ),
        (With<WorldEntity>, Without<Camera>),
    >,
) {
    let target_entity_id = match player_view_state.controlled_entity_id.as_ref() {
        Some(id) if entity_registry.by_entity_id.contains_key(id.as_str()) => Some(id),
        // Avoid destructive stripping during transient unresolved control mapping.
        Some(_) => return,
        None => session
            .player_entity_id
            .as_ref()
            .filter(|id| entity_registry.by_entity_id.contains_key(id.as_str())),
    };
    let target_entity =
        target_entity_id.and_then(|id| entity_registry.by_entity_id.get(id.as_str()).copied());

    let Some(target_entity) = target_entity else {
        // Control target not resolved yet (bootstrap/handoff). Avoid destructive stripping.
        return;
    };
    let mut target_guid: Option<uuid::Uuid> = None;
    for (
        entity,
        _,
        mounted_on,
        hardpoint,
        player_tag,
        guid,
        _,
        _,
        _,
        _,
        _,
        _,
        has_controlled_entity_guid,
        _,
        _,
    ) in &root_world_entities
    {
        let is_root_entity = mounted_on.is_none()
            && hardpoint.is_none()
            && player_tag.is_none()
            && guid.is_some()
            && !has_controlled_entity_guid;
        if entity == target_entity && is_root_entity {
            target_guid = guid.map(|guid| guid.0);
            break;
        }
    }

    let target_position = root_world_entities.iter().find_map(
        |(
            entity,
            _,
            mounted_on,
            hardpoint,
            player_tag,
            _,
            position,
            transform,
            _,
            _,
            _,
            _,
            _,
            _,
            _,
        )| {
            if entity != target_entity
                || mounted_on.is_some()
                || hardpoint.is_some()
                || player_tag.is_some()
            {
                return None;
            }
            position
                .map(|p| p.0)
                .or_else(|| transform.map(|t| t.translation.truncate()))
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
            transform,
            _,
            _,
            _,
            _,
            _,
            has_controlled_entity_guid,
            is_suppressed,
        ) in &root_world_entities
        {
            let is_root_entity = mounted_on.is_none()
                && hardpoint.is_none()
                && player_tag.is_none()
                && guid.is_some()
                && !has_controlled_entity_guid;
            if !is_root_entity || controlled.is_some() || entity == target_entity || is_suppressed {
                continue;
            }
            if guid.is_some_and(|guid| Some(guid.0) == target_guid) {
                // Never keep a nearby proxy for logical duplicates of the locally controlled entity.
                // Duplicate local copies can collide and create client-only velocity drift.
                continue;
            }
            let Some(remote_pos) = position
                .map(|p| p.0)
                .or_else(|| transform.map(|t| t.translation.truncate()))
            else {
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
        position,
        _transform,
        rotation,
        linear_velocity,
        size_m,
        total_mass_kg,
        has_controlled_entity_guid,
        has_rigidbody,
        is_suppressed,
    ) in &root_world_entities
    {
        if entity == target_entity {
            if is_suppressed {
                commands.entity(entity).remove::<SuppressedPredictedDuplicateVisual>();
            }
            let position = position.map(|p| p.0).unwrap_or(Vec2::ZERO);
            let rotation = rotation.copied().unwrap_or(Rotation::IDENTITY);
            let linear_velocity = linear_velocity.map(|v| v.0).unwrap_or(Vec2::ZERO);
            let mut entity_commands = commands.entity(entity);
            let has_physics_data = size_m.is_some() && total_mass_kg.is_some_and(|m| m.0 > 0.0);
            if has_physics_data && !has_rigidbody {
                let size = size_m.copied().unwrap();
                let mass_kg = total_mass_kg.map(|m| m.0).unwrap();
                entity_commands.insert((
                    RigidBody::Dynamic,
                    Collider::rectangle(size.width, size.length),
                    Mass(mass_kg),
                    angular_inertia_from_size(mass_kg, &size),
                    LinearDamping(0.0),
                    AngularDamping(0.0),
                ));
            }
            entity_commands.insert((Position(position), rotation, LinearVelocity(linear_velocity)));
            if !local_mode.0 {
                entity_commands
                    .insert(lightyear::prelude::Predicted)
                    .remove::<lightyear::prelude::Interpolated>();
            }
            entity_commands.remove::<NearbyCollisionProxy>();
            continue;
        }

        if controlled.is_some() {
            commands.entity(entity).remove::<ControlledEntity>();
        }

        let is_root_entity = mounted_on.is_none()
            && hardpoint.is_none()
            && player_tag.is_none()
            && _guid.is_some()
            && !has_controlled_entity_guid;
        if !is_root_entity {
            continue;
        }
        if is_suppressed {
            commands.entity(entity).remove::<NearbyCollisionProxy>();
            commands.entity(entity).remove::<(
                ActionQueue,
                FlightComputer,
                RigidBody,
                Collider,
                Mass,
                AngularInertia,
                LockedAxes,
                LinearDamping,
                AngularDamping,
            )>();
            if !local_mode.0 {
                commands
                    .entity(entity)
                    .insert(lightyear::prelude::Interpolated)
                    .remove::<lightyear::prelude::Predicted>();
            }
            continue;
        }

        let keep_nearby_proxy = nearby_proxy_entities.contains(&entity);
        if keep_nearby_proxy {
            if let Some(size) = size_m {
                let mut entity_commands = commands.entity(entity);
                if !has_rigidbody {
                    entity_commands.insert(RigidBody::Kinematic);
                }
                entity_commands.insert(Collider::rectangle(size.width, size.length));
                entity_commands.insert(NearbyCollisionProxy);
                // Kinematic collision proxy should not carry local dynamic mass/inertia writers.
                entity_commands.remove::<(Mass, AngularInertia)>();
            }
            // No SizeM: physics does not apply; leave entity without RigidBody/Collider.
        } else {
            // Remote/non-controlled ships must remain receive-only on client every tick.
            // Replication may re-add these components after initial adoption.
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
        commands.entity(entity).remove::<ActionQueue>();
        if !local_mode.0 {
            commands
                .entity(entity)
                .insert(lightyear::prelude::Interpolated)
                .remove::<lightyear::prelude::Predicted>();
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
            Has<FlightComputer>,
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
        has_flight_computer,
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
            || has_flight_computer
            || (has_rigidbody && !has_nearby_proxy)
        {
            anomalies.push(format!(
                "{} remote writers present predicted={} action_queue={} flight_computer={} rb={} nearby_proxy={}",
                entity_name,
                is_predicted,
                has_action_queue,
                has_flight_computer,
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
        let mut heading = if let Some(rot) = rotation.as_ref() {
            if rot.is_finite() {
                rot.as_radians()
            } else {
                0.0
            }
        } else if transform.rotation.is_finite() {
            transform.rotation.to_euler(EulerRot::ZYX).2
        } else {
            0.0
        };
        if !heading.is_finite() {
            heading = 0.0;
        }
        let planar_rot = Quat::from_rotation_z(heading);
        if let Some(mut rot) = rotation {
            *rot = Rotation::radians(heading);
        }
        transform.translation.z = 0.0;
        transform.rotation = planar_rot;
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn reconcile_controlled_prediction_with_confirmed(
    time: Res<'_, Time>,
    mut log_state: Local<'_, PredictionReconcileLogState>,
    mut controlled_query: Query<
        '_,
        '_,
        (
            Option<&EntityGuid>,
            &mut Position,
            &mut Rotation,
            Option<&mut LinearVelocity>,
            Option<&mut Transform>,
            Option<&lightyear::prelude::Confirmed<Position>>,
            Option<&lightyear::prelude::Confirmed<Rotation>>,
            Option<&lightyear::prelude::Confirmed<LinearVelocity>>,
        ),
        (With<ControlledEntity>, With<lightyear::prelude::Predicted>),
    >,
) {
    const SNAP_POS_ERROR_M: f32 = 64.0;
    const SMOOTH_POS_ERROR_M: f32 = 2.0;
    const SMOOTH_FACTOR: f32 = 0.25;
    const SNAP_ROT_ERROR_RAD: f32 = 0.8;
    const SMOOTH_ROT_ERROR_RAD: f32 = 0.08;

    for (
        guid,
        mut position,
        mut rotation,
        mut linear_velocity,
        transform,
        confirmed_position,
        confirmed_rotation,
        confirmed_linear_velocity,
    ) in &mut controlled_query
    {
        let Some(confirmed_position) = confirmed_position else {
            continue;
        };

        let confirmed_pos = confirmed_position.0.0;
        let pos_error = confirmed_pos - position.0;
        let pos_error_len = pos_error.length();
        let mut pos_mode = "none";
        if pos_error_len >= SNAP_POS_ERROR_M {
            position.0 = confirmed_pos;
            pos_mode = "snap";
            if let Some(velocity) = linear_velocity.as_mut() {
                velocity.0 = confirmed_linear_velocity.map_or(Vec2::ZERO, |v| v.0.0);
            }
        } else if pos_error_len >= SMOOTH_POS_ERROR_M {
            position.0 += pos_error * SMOOTH_FACTOR;
            pos_mode = "smooth";
        }

        let mut vel_error_len = 0.0_f32;
        let mut vel_mode = "none";
        if let Some(velocity) = linear_velocity.as_mut()
            && let Some(confirmed_vel) = confirmed_linear_velocity
        {
            let confirmed = confirmed_vel.0.0;
            let vel_error = (confirmed - velocity.0).length();
            vel_error_len = vel_error;
            if pos_error_len >= SNAP_POS_ERROR_M || vel_error >= 2.0 {
                velocity.0 = confirmed;
                vel_mode = "snap";
            } else {
                velocity.0 = velocity.0.lerp(confirmed, 0.35);
                vel_mode = "smooth";
            }
            if confirmed.length_squared() <= 1.0e-6 && velocity.0.length_squared() <= 1.0e-4 {
                velocity.0 = Vec2::ZERO;
            }
        }

        let mut rot_error = 0.0_f32;
        let mut rot_mode = "none";
        if let Some(confirmed_rotation) = confirmed_rotation {
            let confirmed_rot = confirmed_rotation.0;
            rot_error = rotation.angle_between(confirmed_rot);
            let rot_error_abs = rot_error.abs();
            if rot_error_abs >= SNAP_ROT_ERROR_RAD {
                *rotation = confirmed_rot;
                rot_mode = "snap";
            } else if rot_error_abs >= SMOOTH_ROT_ERROR_RAD {
                *rotation = rotation.slerp(confirmed_rot, SMOOTH_FACTOR);
                rot_mode = "smooth";
            }
        }

        if let Some(mut transform) = transform {
            transform.translation.x = position.0.x;
            transform.translation.y = position.0.y;
            transform.rotation = (*rotation).into();
            transform.translation.z = 0.0;
        }

        if prediction_reconcile_debug_logging_enabled() {
            let now_s = time.elapsed_secs_f64();
            let correction_applied = pos_mode != "none" || vel_mode != "none" || rot_mode != "none";
            let should_log = correction_applied || now_s - log_state.last_logged_at_s >= 0.5;
            if should_log {
                let guid_str = guid
                    .map(|g| g.0.to_string())
                    .unwrap_or_else(|| "unknown-guid".to_string());
                info!(
                    entity_guid = %guid_str,
                    pos_error_m = pos_error_len,
                    rot_error_rad = rot_error,
                    vel_error_mps = vel_error_len,
                    pos_mode = %pos_mode,
                    rot_mode = %rot_mode,
                    vel_mode = %vel_mode,
                    predicted_pos = ?position.0,
                    confirmed_pos = ?confirmed_pos,
                    "prediction reconcile sample"
                );
                log_state.last_logged_at_s = now_s;
            }
        }
    }
}
