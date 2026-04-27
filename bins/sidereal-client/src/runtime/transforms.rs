//! World entity transform sync, interpolation, and player/camera lock.

use avian2d::prelude::{
    AngularInertia, AngularVelocity, LinearVelocity, Mass, Position, RigidBody, Rotation,
};
use bevy::{math::DVec2, prelude::*};
use lightyear::frame_interpolation::FrameInterpolate;
use lightyear::interpolation::interpolation_history::ConfirmedHistory;
use lightyear::prelude::Confirmed;
use lightyear::prelude::input::native::{ActionState, InputMarker};
use sidereal_game::{
    EntityGuid, FlightControlAuthority, FullscreenLayer, RENDER_DOMAIN_FULLSCREEN,
    RENDER_PHASE_FULLSCREEN_BACKGROUND, RENDER_PHASE_FULLSCREEN_FOREGROUND,
    RuntimeRenderLayerDefinition, SimulationMotionWriter, WorldPosition, WorldRotation,
    resolve_world_position, resolve_world_rotation_rad,
};
use sidereal_net::PlayerInput;
use sidereal_runtime_sync::RuntimeEntityHierarchy;
use std::collections::HashMap;
use std::sync::OnceLock;

use super::app_state::{ClientSession, LocalPlayerViewState};
use super::components::{
    ControlledEntity, PendingInitialVisualReady, PendingVisibilityFadeIn, WorldEntity,
};

fn motion_replication_diagnostics_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("SIDEREAL_DEBUG_MOTION_REPLICATION")
            .is_ok_and(|value| value == "1" || value.eq_ignore_ascii_case("true"))
    })
}

#[derive(Default)]
pub(crate) struct ClientMotionReplicationDiagnosticsState {
    last_logged_at_s: f64,
    last_transform_by_guid: HashMap<uuid::Uuid, Vec3>,
}

#[allow(clippy::type_complexity)]
pub(crate) fn log_motion_replication_diagnostics(
    time: Res<'_, Time>,
    session: Res<'_, ClientSession>,
    player_view_state: Res<'_, LocalPlayerViewState>,
    mut state: Local<'_, ClientMotionReplicationDiagnosticsState>,
    entities: Query<
        '_,
        '_,
        (
            Entity,
            &'_ EntityGuid,
            Option<&'_ Confirmed<Position>>,
            Option<&'_ ConfirmedHistory<Position>>,
            Option<&'_ Position>,
            Option<&'_ Rotation>,
            Option<&'_ LinearVelocity>,
            Option<&'_ AngularVelocity>,
            &'_ Transform,
            Has<lightyear::prelude::Predicted>,
            Has<lightyear::prelude::Interpolated>,
            Has<ControlledEntity>,
            Has<SimulationMotionWriter>,
            Has<FlightControlAuthority>,
        ),
        With<WorldEntity>,
    >,
    extras: Query<
        '_,
        '_,
        (
            Option<&'_ Confirmed<Rotation>>,
            Option<&'_ Mass>,
            Option<&'_ AngularInertia>,
            Has<InputMarker<PlayerInput>>,
            Option<&'_ ActionState<PlayerInput>>,
            Has<RigidBody>,
        ),
    >,
) {
    if !motion_replication_diagnostics_enabled() {
        return;
    }
    const LOG_INTERVAL_S: f64 = 1.0;
    let now_s = time.elapsed_secs_f64();
    if now_s - state.last_logged_at_s < LOG_INTERVAL_S {
        return;
    }
    state.last_logged_at_s = now_s;

    let player_guid = session
        .player_entity_id
        .as_deref()
        .and_then(sidereal_runtime_sync::parse_guid_from_entity_id)
        .or_else(|| {
            session
                .player_entity_id
                .as_deref()
                .and_then(|id| uuid::Uuid::parse_str(id).ok())
        });
    let controlled_guid = player_view_state
        .controlled_entity_id
        .as_deref()
        .and_then(sidereal_runtime_sync::parse_guid_from_entity_id)
        .or_else(|| {
            player_view_state
                .controlled_entity_id
                .as_deref()
                .and_then(|id| uuid::Uuid::parse_str(id).ok())
        });

    for (
        entity,
        guid,
        confirmed_position,
        position_history,
        position,
        rotation,
        velocity,
        angular_velocity,
        transform,
        is_predicted,
        is_interpolated,
        is_controlled,
        has_motion_writer,
        has_flight_authority,
    ) in &entities
    {
        let (
            confirmed_rotation,
            mass,
            angular_inertia,
            has_input_marker,
            action_state,
            has_rigidbody,
        ) = extras
            .get(entity)
            .unwrap_or((None, None, None, false, None, false));
        let previous = state
            .last_transform_by_guid
            .insert(guid.0, transform.translation);
        let transform_changed_since_last = previous
            .is_some_and(|previous| previous.distance_squared(transform.translation) > 0.0001);
        let history_len = position_history.map(ConfirmedHistory::len).unwrap_or(0);
        let current = position.map(|value| value.0);
        let relevant_to_local_control =
            Some(guid.0) == controlled_guid || Some(guid.0) == player_guid;
        if !relevant_to_local_control
            && !is_controlled
            && !is_predicted
            && confirmed_position.is_none()
            && current.is_none()
            && history_len == 0
        {
            continue;
        }
        let history_newest_tick = position_history
            .and_then(ConfirmedHistory::newest)
            .map(|(tick, _)| tick.0);
        let confirmed = confirmed_position.map(|value| value.0.0);
        let confirmed_rotation_rad = confirmed_rotation.map(|value| value.0.as_radians());
        let linear_velocity = velocity.map(|value| value.0);
        let rotation_rad = rotation.map(|value| value.as_radians());
        let angular_velocity_rad_s = angular_velocity.map(|value| value.0);
        let mass_kg = mass.map(|value| value.0);
        let angular_inertia_kg_m2 = angular_inertia.map(|value| value.0);
        let actions = action_state.map(|state| state.0.actions.as_slice());
        info!(
            ?entity,
            guid = %guid.0,
            is_predicted,
            is_interpolated,
            is_controlled,
            has_motion_writer,
            has_flight_authority,
            has_input_marker,
            has_rigidbody,
            confirmed_position = ?confirmed,
            confirmed_rotation_rad,
            current_position = ?current,
            rotation_rad,
            linear_velocity = ?linear_velocity,
            angular_velocity_rad_s,
            mass_kg,
            angular_inertia_kg_m2,
            history_len,
            history_newest_tick,
            transform_x = transform.translation.x,
            transform_y = transform.translation.y,
            transform_rotation_rad = transform.rotation.to_euler(EulerRot::XYZ).2,
            actions = ?actions,
            transform_changed_since_last,
            "client motion replication diagnostic"
        );
    }
}

fn apply_planar_transform(transform: &mut Transform, planar_position: DVec2, heading: f64) {
    transform.translation.x = planar_position.x as f32;
    transform.translation.y = planar_position.y as f32;
    transform.translation.z = 0.0;
    transform.rotation = Quat::from_rotation_z(heading as f32);
}

fn resolve_current_planar_pose(
    position: Option<&Position>,
    rotation: Option<&Rotation>,
    world_position: Option<&WorldPosition>,
    world_rotation: Option<&WorldRotation>,
) -> Option<(DVec2, f64)> {
    let planar_position = resolve_world_position(position, world_position)?;
    let heading = resolve_world_rotation_rad(rotation, world_rotation)?;
    if !planar_position.is_finite() || !heading.is_finite() {
        return None;
    }
    Some((planar_position, heading))
}

fn resolve_confirmed_planar_pose(
    confirmed_position: Option<&Confirmed<Position>>,
    confirmed_rotation: Option<&Confirmed<Rotation>>,
) -> Option<(DVec2, f64)> {
    let planar_position = confirmed_position.map(|value| value.0.0)?;
    let heading = confirmed_rotation.map(|value| value.0.as_radians())?;
    if !planar_position.is_finite() || !heading.is_finite() {
        return None;
    }
    Some((planar_position, heading))
}

#[allow(clippy::type_complexity)]
fn resolve_canonical_confirmed_planar_pose(
    entity_guid: &EntityGuid,
    current_entity: Entity,
    entity_registry: &RuntimeEntityHierarchy,
    confirmed_entities: &Query<
        '_,
        '_,
        (
            Option<&Position>,
            Option<&Rotation>,
            Option<&WorldPosition>,
            Option<&WorldRotation>,
        ),
        (With<WorldEntity>, Without<lightyear::prelude::Interpolated>),
    >,
) -> Option<(DVec2, f64)> {
    let canonical_entity = entity_registry
        .by_entity_id
        .get(entity_guid.0.to_string().as_str())
        .copied()?;
    if canonical_entity == current_entity {
        return None;
    }
    let (position, rotation, world_position, world_rotation) =
        confirmed_entities.get(canonical_entity).ok()?;
    resolve_current_planar_pose(position, rotation, world_position, world_rotation)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn interpolated_presentation_ready(
    position: Option<&Position>,
    rotation: Option<&Rotation>,
    world_position: Option<&WorldPosition>,
    world_rotation: Option<&WorldRotation>,
    confirmed_position: Option<&Confirmed<Position>>,
    confirmed_rotation: Option<&Confirmed<Rotation>>,
    position_history: Option<&ConfirmedHistory<Position>>,
    rotation_history: Option<&ConfirmedHistory<Rotation>>,
) -> bool {
    let history_ready = position_history.and_then(|history| history.end()).is_some()
        && rotation_history.and_then(|history| history.end()).is_some();
    let is_static_world_spatial = position.is_none()
        && rotation.is_none()
        && (world_position.is_some() || world_rotation.is_some());
    history_ready
        || resolve_confirmed_planar_pose(confirmed_position, confirmed_rotation).is_some()
        || (is_static_world_spatial
            && resolve_current_planar_pose(position, rotation, world_position, world_rotation)
                .is_some())
}

/// Fallback sync for Confirmed-only world entities.
///
/// Predicted/Interpolated entities are synced by LightyearAvian when
/// `update_syncs_manually = false`; this path intentionally excludes them.
#[allow(clippy::type_complexity)]
pub(crate) fn sync_confirmed_world_entity_transforms_from_physics(
    mut entities: Query<
        '_,
        '_,
        (&'_ Position, &'_ Rotation, &'_ mut Transform),
        (
            With<WorldEntity>,
            Without<lightyear::prelude::Predicted>,
            Without<lightyear::prelude::Interpolated>,
        ),
    >,
) {
    for (position, rotation, mut transform) in &mut entities {
        let planar_position = if position.0.is_finite() {
            position.0
        } else {
            DVec2::ZERO
        };
        let heading = if rotation.is_finite() {
            rotation.as_radians()
        } else {
            0.0
        };
        transform.translation.x = planar_position.x as f32;
        transform.translation.y = planar_position.y as f32;
        transform.translation.z = 0.0;
        transform.rotation = Quat::from_rotation_z(heading as f32);
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn sync_confirmed_world_entity_transforms_from_world_space(
    mut entities: Query<
        '_,
        '_,
        (
            &'_ WorldPosition,
            Option<&'_ WorldRotation>,
            &'_ mut Transform,
        ),
        (
            With<WorldEntity>,
            Without<Position>,
            Without<Rotation>,
            Without<lightyear::prelude::Predicted>,
            Without<lightyear::prelude::Interpolated>,
        ),
    >,
) {
    for (position, rotation, mut transform) in &mut entities {
        let planar_position = if position.0.is_finite() {
            position.0
        } else {
            DVec2::ZERO
        };
        let heading = rotation
            .map(|value| value.0)
            .filter(|value| value.is_finite())
            .unwrap_or(0.0);
        transform.translation.x = planar_position.x as f32;
        transform.translation.y = planar_position.y as f32;
        transform.translation.z = 0.0;
        transform.rotation = Quat::from_rotation_z(heading as f32);
    }
}

/// Bootstrap for interpolated entities that just became relevant but do not yet
/// have interpolation history samples. Without this, they can render at default
/// Transform (0,0) until the next server delta arrives.
#[allow(clippy::type_complexity)]
pub(crate) fn sync_interpolated_world_entity_transforms_without_history(
    entity_registry: Res<'_, RuntimeEntityHierarchy>,
    mut entities: Query<
        '_,
        '_,
        (
            Entity,
            &'_ EntityGuid,
            Option<&'_ Position>,
            Option<&'_ Rotation>,
            Option<&'_ WorldPosition>,
            Option<&'_ WorldRotation>,
            Option<&'_ Confirmed<Position>>,
            Option<&'_ Confirmed<Rotation>>,
            &'_ mut Transform,
            Option<&'_ ConfirmedHistory<Position>>,
            Option<&'_ ConfirmedHistory<Rotation>>,
        ),
        (With<WorldEntity>, With<lightyear::prelude::Interpolated>),
    >,
    confirmed_entities: Query<
        '_,
        '_,
        (
            Option<&'_ Position>,
            Option<&'_ Rotation>,
            Option<&'_ WorldPosition>,
            Option<&'_ WorldRotation>,
        ),
        (With<WorldEntity>, Without<lightyear::prelude::Interpolated>),
    >,
) {
    for (
        entity,
        entity_guid,
        position,
        rotation,
        world_position,
        world_rotation,
        confirmed_position,
        confirmed_rotation,
        mut transform,
        position_history,
        rotation_history,
    ) in &mut entities
    {
        let is_static_world_spatial = position.is_none()
            && rotation.is_none()
            && (world_position.is_some() || world_rotation.is_some());
        if is_static_world_spatial {
            let (planar_position, heading) =
                resolve_current_planar_pose(position, rotation, world_position, world_rotation)
                    .unwrap_or((DVec2::ZERO, 0.0));
            apply_planar_transform(&mut transform, planar_position, heading);
            continue;
        }
        // Interpolation needs at least 2 samples. With only one (or zero), preserve
        // authoritative spawn pose from Confirmed values so entities don't render at origin.
        let history_ready = position_history.and_then(|h| h.end()).is_some()
            && rotation_history.and_then(|h| h.end()).is_some();
        if history_ready {
            continue;
        }
        let (planar_position, heading) =
            resolve_confirmed_planar_pose(confirmed_position, confirmed_rotation)
                .or_else(|| {
                    resolve_canonical_confirmed_planar_pose(
                        entity_guid,
                        entity,
                        &entity_registry,
                        &confirmed_entities,
                    )
                })
                .or_else(|| {
                    resolve_current_planar_pose(position, rotation, world_position, world_rotation)
                })
                .unwrap_or((DVec2::ZERO, 0.0));
        apply_planar_transform(&mut transform, planar_position, heading);
    }
}

/// Keep `FrameInterpolate<Transform>` aligned with the runtime clone types that Lightyear expects.
///
/// Sidereal intentionally defers native replicated adoption until enough components exist to avoid
/// origin flashes, but dynamic relevance/handoff means a world entity can become spatial later in
/// its lifecycle. Lightyear requires `FrameInterpolate<Transform>` to be present explicitly; if we
/// only decide that once at adoption time, a valid Interpolated/Predicted clone can miss the visual
/// interpolation lane entirely.
#[allow(clippy::type_complexity)]
pub(crate) fn sync_frame_interpolation_markers_for_world_entities(
    mut commands: Commands<'_, '_>,
    entities: Query<
        '_,
        '_,
        (
            Entity,
            Has<Position>,
            Has<Rotation>,
            Has<lightyear::prelude::Predicted>,
            Has<lightyear::prelude::Interpolated>,
            Option<&'_ FrameInterpolate<Transform>>,
        ),
        With<WorldEntity>,
    >,
) {
    for (entity, has_position, has_rotation, is_predicted, is_interpolated, frame_interpolate) in
        &entities
    {
        let should_have_frame_interpolation =
            (is_predicted || is_interpolated) && has_position && has_rotation;
        if should_have_frame_interpolation && frame_interpolate.is_none() {
            commands
                .entity(entity)
                .insert(FrameInterpolate::<Transform>::default());
        } else if !should_have_frame_interpolation && frame_interpolate.is_some() {
            commands
                .entity(entity)
                .remove::<FrameInterpolate<Transform>>();
        }
    }
}

/// Seed observer visual transforms only while frame interpolation is still uninitialized.
///
/// Late-lane bootstrap belongs in Lightyear/Avian. Sidereal keeps only this narrow seeding path so
/// freshly adopted observer entities do not spend a frame at the default transform before
/// interpolation state is ready.
#[allow(clippy::type_complexity)]
pub(crate) fn recover_stalled_interpolated_world_entity_transforms(
    mut entities: Query<
        '_,
        '_,
        (
            Option<&'_ Position>,
            Option<&'_ Rotation>,
            Option<&'_ WorldPosition>,
            Option<&'_ WorldRotation>,
            &'_ mut Transform,
            &'_ mut FrameInterpolate<Transform>,
        ),
        (With<WorldEntity>, With<lightyear::prelude::Interpolated>),
    >,
) {
    for (
        position,
        rotation,
        world_position,
        world_rotation,
        mut transform,
        mut frame_interpolate,
    ) in &mut entities
    {
        let Some((planar_position, heading)) =
            resolve_current_planar_pose(position, rotation, world_position, world_rotation)
        else {
            continue;
        };

        let frame_interpolation_uninitialized =
            frame_interpolate.previous_value.is_none() || frame_interpolate.current_value.is_none();
        if !frame_interpolation_uninitialized {
            continue;
        }

        apply_planar_transform(&mut transform, planar_position, heading);
        let seeded_transform = *transform;
        frame_interpolate.previous_value = Some(seeded_transform);
        frame_interpolate.current_value = Some(seeded_transform);
    }
}

/// Seed predicted visual transforms only while frame interpolation is still uninitialized.
///
/// Once the predicted lane is live, transform ownership should stay with Lightyear/Avian and the
/// frame interpolation pipeline rather than a client-side drift-repair shim.
#[allow(clippy::type_complexity)]
pub(crate) fn recover_stalled_predicted_world_entity_transforms(
    mut entities: Query<
        '_,
        '_,
        (
            &'_ Position,
            &'_ Rotation,
            &'_ mut Transform,
            Option<&'_ mut FrameInterpolate<Transform>>,
        ),
        (
            With<WorldEntity>,
            With<lightyear::prelude::Predicted>,
            Without<lightyear::prelude::Interpolated>,
        ),
    >,
) {
    for (position, rotation, mut transform, frame_interpolate) in &mut entities {
        let planar_position = position.0;
        let heading = rotation.as_radians();
        if !planar_position.is_finite() || !heading.is_finite() {
            continue;
        }

        let frame_interpolation_uninitialized = frame_interpolate
            .as_ref()
            .is_some_and(|frame| frame.previous_value.is_none() || frame.current_value.is_none());
        if !frame_interpolation_uninitialized {
            continue;
        }

        apply_planar_transform(&mut transform, planar_position, heading);
        if let Some(mut frame_interpolate) = frame_interpolate {
            let seeded_transform = *transform;
            frame_interpolate.previous_value = Some(seeded_transform);
            frame_interpolate.current_value = Some(seeded_transform);
        }
    }
}

/// Keep newly adopted entities hidden until we can render an authoritative pose.
///
/// This prevents transient origin flashes when relevance is gained but interpolation
/// history is not yet ready on the first render frame.
#[allow(clippy::type_complexity)]
pub(crate) fn reveal_world_entities_when_initial_transform_ready(
    mut commands: Commands<'_, '_>,
    entity_registry: Res<'_, RuntimeEntityHierarchy>,
    mut entities: Query<
        '_,
        '_,
        (
            Entity,
            &'_ EntityGuid,
            Has<lightyear::prelude::Interpolated>,
            Option<&'_ Position>,
            Option<&'_ Rotation>,
            Option<&'_ WorldPosition>,
            Option<&'_ WorldRotation>,
            Option<&'_ Confirmed<Position>>,
            Option<&'_ Confirmed<Rotation>>,
            Option<&'_ ConfirmedHistory<Position>>,
            Option<&'_ ConfirmedHistory<Rotation>>,
            Option<&'_ FullscreenLayer>,
            Option<&'_ RuntimeRenderLayerDefinition>,
            &'_ mut Transform,
            &'_ mut Visibility,
        ),
        (With<WorldEntity>, With<PendingInitialVisualReady>),
    >,
    confirmed_entities: Query<
        '_,
        '_,
        (
            Option<&'_ Position>,
            Option<&'_ Rotation>,
            Option<&'_ WorldPosition>,
            Option<&'_ WorldRotation>,
        ),
        (With<WorldEntity>, Without<lightyear::prelude::Interpolated>),
    >,
) {
    for (
        entity,
        entity_guid,
        is_interpolated,
        position,
        rotation,
        world_position,
        world_rotation,
        confirmed_position,
        confirmed_rotation,
        position_history,
        rotation_history,
        fullscreen_layer,
        runtime_layer,
        mut transform,
        mut visibility,
    ) in &mut entities
    {
        let mut ready = false;
        let mut source_position: Option<DVec2> = None;
        let mut source_heading: Option<f64> = None;

        let is_runtime_fullscreen_layer = runtime_layer.is_some_and(|layer| {
            layer.enabled
                && layer.material_domain == RENDER_DOMAIN_FULLSCREEN
                && matches!(
                    layer.phase.as_str(),
                    RENDER_PHASE_FULLSCREEN_BACKGROUND | RENDER_PHASE_FULLSCREEN_FOREGROUND
                )
        });
        if fullscreen_layer.is_some() || is_runtime_fullscreen_layer {
            // Fullscreen layers are non-spatial overlay entities: they have no physics
            // transform history but should render as soon as adopted. Sidereal renders
            // directly from these authored entities so they must bypass the normal
            // spatial bootstrap gate instead of falling back to client-local copies.
            ready = true;
        } else if position.is_none()
            && rotation.is_none()
            && (world_position.is_some() || world_rotation.is_some())
        {
            let planar_position = resolve_world_position(position, world_position);
            let heading = world_rotation
                .map(|value| value.0)
                .filter(|value| value.is_finite())
                .or_else(|| {
                    resolve_world_rotation_rad(rotation, world_rotation)
                        .filter(|value| value.is_finite())
                })
                .or(Some(0.0));
            if let (Some(planar_position), Some(heading)) = (planar_position, heading) {
                source_position = Some(planar_position);
                source_heading = Some(heading);
                ready = true;
            } else if let (Some(cp), Some(cr)) = (confirmed_position, confirmed_rotation) {
                source_position = Some(cp.0.0);
                source_heading = Some(cr.0.as_radians());
                ready = true;
            }
        } else if is_interpolated {
            if interpolated_presentation_ready(
                position,
                rotation,
                world_position,
                world_rotation,
                confirmed_position,
                confirmed_rotation,
                position_history,
                rotation_history,
            ) {
                ready = true;
                if let Some((planar_position, heading)) =
                    resolve_confirmed_planar_pose(confirmed_position, confirmed_rotation)
                {
                    source_position = Some(planar_position);
                    source_heading = Some(heading);
                } else if let Some((planar_position, heading)) =
                    resolve_canonical_confirmed_planar_pose(
                        entity_guid,
                        entity,
                        &entity_registry,
                        &confirmed_entities,
                    )
                {
                    source_position = Some(planar_position);
                    source_heading = Some(heading);
                } else if position.is_none()
                    && rotation.is_none()
                    && (world_position.is_some() || world_rotation.is_some())
                    && let Some((planar_position, heading)) = resolve_current_planar_pose(
                        position,
                        rotation,
                        world_position,
                        world_rotation,
                    )
                {
                    source_position = Some(planar_position);
                    source_heading = Some(heading);
                }
            } else if let Some((planar_position, heading)) =
                resolve_confirmed_planar_pose(confirmed_position, confirmed_rotation)
            {
                source_position = Some(planar_position);
                source_heading = Some(heading);
            } else if let Some((planar_position, heading)) = resolve_canonical_confirmed_planar_pose(
                entity_guid,
                entity,
                &entity_registry,
                &confirmed_entities,
            ) {
                source_position = Some(planar_position);
                source_heading = Some(heading);
                ready = true;
            }
        } else if let (Some(planar_position), Some(heading)) = (
            resolve_world_position(position, world_position),
            resolve_world_rotation_rad(rotation, world_rotation),
        ) {
            source_position = Some(planar_position);
            source_heading = Some(heading);
            ready = true;
        }

        if !ready {
            *visibility = Visibility::Hidden;
            continue;
        }

        if let (Some(planar_position), Some(heading)) = (source_position, source_heading)
            && planar_position.is_finite()
            && heading.is_finite()
        {
            apply_planar_transform(&mut transform, planar_position, heading);
        }

        *visibility = Visibility::Visible;
        commands
            .entity(entity)
            .remove::<PendingInitialVisualReady>()
            .insert(PendingVisibilityFadeIn {
                elapsed_s: 0.0,
                duration_s: 0.16,
            });
    }
}

#[cfg(test)]
mod tests {
    use super::{
        interpolated_presentation_ready, reveal_world_entities_when_initial_transform_ready,
        sync_interpolated_world_entity_transforms_without_history,
    };
    use crate::runtime::components::{PendingInitialVisualReady, WorldEntity};
    use avian2d::prelude::{Position, Rotation};
    use bevy::prelude::*;
    use lightyear::prelude::Interpolated;
    use sidereal_game::EntityGuid;
    use sidereal_runtime_sync::RuntimeEntityHierarchy;
    use uuid::Uuid;

    #[test]
    fn interpolated_presentation_ready_rejects_dynamic_current_pose_without_confirmed_or_history() {
        assert!(!interpolated_presentation_ready(
            Some(&Position(Vec2::ZERO.into())),
            Some(&Rotation::IDENTITY),
            None,
            None,
            None,
            None,
            None,
            None,
        ));
    }

    #[test]
    fn reveal_keeps_dynamic_interpolated_entity_hidden_without_authoritative_pose() {
        let mut app = App::new();
        app.init_resource::<RuntimeEntityHierarchy>();
        app.add_systems(Update, reveal_world_entities_when_initial_transform_ready);

        let entity = app
            .world_mut()
            .spawn((
                WorldEntity,
                PendingInitialVisualReady,
                EntityGuid(Uuid::new_v4()),
                Interpolated,
                Position(Vec2::ZERO.into()),
                Rotation::IDENTITY,
                Transform::default(),
                Visibility::Visible,
            ))
            .id();

        app.update();

        let entity_ref = app.world().entity(entity);
        assert_eq!(
            *entity_ref.get::<Visibility>().expect("visibility"),
            Visibility::Hidden
        );
        assert!(
            entity_ref.contains::<PendingInitialVisualReady>(),
            "entity should remain pending until it has confirmed pose or interpolation history"
        );
    }

    #[test]
    fn reveal_uses_canonical_confirmed_pose_when_interpolated_bootstrap_is_missing() {
        let mut app = App::new();
        app.init_resource::<RuntimeEntityHierarchy>();
        app.add_systems(Update, reveal_world_entities_when_initial_transform_ready);

        let guid = Uuid::parse_str("ce9e421c-8b62-458a-803e-51e9ad272908").expect("valid guid");
        let confirmed = app
            .world_mut()
            .spawn((
                WorldEntity,
                EntityGuid(guid),
                Position(Vec2::new(12.0, 34.0).into()),
                Rotation::radians(0.75),
                Transform::default(),
                Visibility::Visible,
            ))
            .id();
        app.world_mut()
            .resource_mut::<RuntimeEntityHierarchy>()
            .by_entity_id
            .insert(guid.to_string(), confirmed);
        let interpolated = app
            .world_mut()
            .spawn((
                WorldEntity,
                PendingInitialVisualReady,
                EntityGuid(guid),
                Interpolated,
                Position(Vec2::ZERO.into()),
                Rotation::IDENTITY,
                Transform::default(),
                Visibility::Hidden,
            ))
            .id();

        app.update();

        let entity_ref = app.world().entity(interpolated);
        let transform = entity_ref.get::<Transform>().expect("transform");
        assert_eq!(transform.translation.x, 12.0);
        assert_eq!(transform.translation.y, 34.0);
        assert_eq!(
            *entity_ref.get::<Visibility>().expect("visibility"),
            Visibility::Visible
        );
        assert!(
            !entity_ref.contains::<PendingInitialVisualReady>(),
            "entity should become renderable once the canonical confirmed clone has a pose"
        );
    }

    #[test]
    fn interpolated_without_history_uses_canonical_confirmed_pose() {
        let mut app = App::new();
        app.init_resource::<RuntimeEntityHierarchy>();
        app.add_systems(
            Update,
            sync_interpolated_world_entity_transforms_without_history,
        );

        let guid = Uuid::parse_str("ce9e421c-8b62-458a-803e-51e9ad272908").expect("valid guid");
        let confirmed = app
            .world_mut()
            .spawn((
                WorldEntity,
                EntityGuid(guid),
                Position(Vec2::new(-20.0, 48.0).into()),
                Rotation::radians(-0.5),
                Transform::default(),
            ))
            .id();
        app.world_mut()
            .resource_mut::<RuntimeEntityHierarchy>()
            .by_entity_id
            .insert(guid.to_string(), confirmed);
        let interpolated = app
            .world_mut()
            .spawn((
                WorldEntity,
                EntityGuid(guid),
                Interpolated,
                Position(Vec2::ZERO.into()),
                Rotation::IDENTITY,
                Transform::default(),
            ))
            .id();

        app.update();

        let transform = app
            .world()
            .entity(interpolated)
            .get::<Transform>()
            .expect("transform");
        assert_eq!(transform.translation.x, -20.0);
        assert_eq!(transform.translation.y, 48.0);
    }
}
