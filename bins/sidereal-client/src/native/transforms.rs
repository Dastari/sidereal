//! World entity transform sync, interpolation, and player/camera lock.

use avian2d::prelude::{Position, Rotation};
use bevy::prelude::*;
use sidereal_game::PlayerTag;

use super::components::{
    ControlledEntity, GameplayCamera, InterpolatedVisualSmoothing,
    SuppressedPredictedDuplicateVisual, TopDownCamera, WorldEntity,
};
use super::state::{ClientSession, LocalPlayerViewState};
use avian2d::prelude::{AngularVelocity, LinearVelocity};
use sidereal_runtime_sync::RuntimeEntityHierarchy;

#[allow(clippy::type_complexity)]
pub(crate) fn sync_world_entity_transforms_from_physics(
    mut entities: Query<
        '_,
        '_,
        (&mut Transform, Option<&Position>, Option<&Rotation>),
        (
            With<WorldEntity>,
            Or<(With<Position>, With<Rotation>)>,
            Without<Camera>,
            Without<lightyear::prelude::Interpolated>,
        ),
    >,
) {
    for (mut transform, position, rotation) in &mut entities {
        if let Some(position) = position {
            transform.translation.x = position.0.x;
            transform.translation.y = position.0.y;
        }
        if let Some(rotation) = rotation {
            transform.rotation = (*rotation).into();
        }
        transform.translation.z = 0.0;
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn refresh_interpolated_visual_targets_system(
    time: Res<'_, Time>,
    mut commands: Commands<'_, '_>,
    mut entities: Query<
        '_,
        '_,
        (
            Entity,
            &Position,
            Option<&Rotation>,
            &mut Transform,
            Option<&mut InterpolatedVisualSmoothing>,
        ),
        (
            With<WorldEntity>,
            With<lightyear::prelude::Interpolated>,
            Without<SuppressedPredictedDuplicateVisual>,
            Or<(Changed<Position>, Changed<Rotation>)>,
        ),
    >,
) {
    let now_s = time.elapsed_secs_f64();
    for (entity, position, rotation, mut transform, smoothing) in &mut entities {
        let target_pos = position.0;
        let target_rot: Quat = rotation
            .copied()
            .map(Quat::from)
            .unwrap_or(transform.rotation);

        if let Some(mut smoothing) = smoothing {
            let interval_s = (now_s - smoothing.last_snapshot_at_s) as f32;
            let duration_s = interval_s.clamp(1.0 / 120.0, 0.25);
            smoothing.from_pos = transform.translation.truncate();
            smoothing.to_pos = target_pos;
            smoothing.from_rot = transform.rotation;
            smoothing.to_rot = target_rot;
            smoothing.elapsed_s = 0.0;
            smoothing.duration_s = duration_s;
            smoothing.last_snapshot_at_s = now_s;
        } else {
            transform.translation.x = target_pos.x;
            transform.translation.y = target_pos.y;
            transform.translation.z = 0.0;
            transform.rotation = target_rot;
            commands.entity(entity).insert(InterpolatedVisualSmoothing {
                from_pos: target_pos,
                to_pos: target_pos,
                from_rot: target_rot,
                to_rot: target_rot,
                elapsed_s: 1.0 / 30.0,
                duration_s: 1.0 / 30.0,
                last_snapshot_at_s: now_s,
            });
        }
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn apply_interpolated_visual_smoothing_system(
    time: Res<'_, Time>,
    mut entities: Query<
        '_,
        '_,
        (&mut Transform, &mut InterpolatedVisualSmoothing),
        (
            With<WorldEntity>,
            With<lightyear::prelude::Interpolated>,
            Without<SuppressedPredictedDuplicateVisual>,
        ),
    >,
) {
    let dt = time.delta_secs().max(0.0);
    for (mut transform, mut smoothing) in &mut entities {
        smoothing.elapsed_s = (smoothing.elapsed_s + dt).max(0.0);
        let alpha = if smoothing.duration_s <= 0.0 {
            1.0
        } else {
            (smoothing.elapsed_s / smoothing.duration_s).clamp(0.0, 1.0)
        };
        let pos = smoothing.from_pos.lerp(smoothing.to_pos, alpha);
        transform.translation.x = pos.x;
        transform.translation.y = pos.y;
        transform.translation.z = 0.0;
        transform.rotation = smoothing.from_rot.slerp(smoothing.to_rot, alpha);
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn lock_player_entity_to_controlled_entity_end_of_frame(
    session: Res<'_, ClientSession>,
    player_view_state: Res<'_, LocalPlayerViewState>,
    entity_registry: Res<'_, RuntimeEntityHierarchy>,
    mut queries: ParamSet<
        '_,
        '_,
        (
            Query<
                '_,
                '_,
                (
                    &'_ Transform,
                    Option<&'_ Position>,
                    Option<&'_ Rotation>,
                    Option<&'_ LinearVelocity>,
                    Option<&'_ AngularVelocity>,
                ),
                Without<Camera>,
            >,
            Query<
                '_,
                '_,
                (
                    &'_ mut Transform,
                    Option<&'_ mut Position>,
                    Option<&'_ mut Rotation>,
                    Option<&'_ mut LinearVelocity>,
                    Option<&'_ mut AngularVelocity>,
                ),
                (With<PlayerTag>, Without<Camera>),
            >,
        ),
    >,
) {
    let Some(player_runtime_id) = session.player_entity_id.as_ref() else {
        return;
    };
    let Some(&player_entity) = entity_registry.by_entity_id.get(player_runtime_id.as_str()) else {
        return;
    };
    let controlled_runtime_id = player_view_state
        .controlled_entity_id
        .as_deref()
        .unwrap_or(player_runtime_id.as_str());
    let Some(&controlled_entity) = entity_registry.by_entity_id.get(controlled_runtime_id) else {
        return;
    };
    if player_entity == controlled_entity {
        return;
    }
    let (
        source_xy,
        source_z,
        source_transform_rotation,
        source_rotation,
        source_linear_velocity,
        source_angular_velocity,
    ) = {
        let source_query = queries.p0();
        let Ok((
            source_transform,
            source_position,
            source_rotation,
            source_linear_velocity,
            source_angular_velocity,
        )) = source_query.get(controlled_entity)
        else {
            return;
        };
        (
            source_position
                .map(|position| position.0)
                .unwrap_or_else(|| source_transform.translation.truncate()),
            source_transform.translation.z,
            source_transform.rotation,
            source_rotation.copied(),
            source_linear_velocity.map(|v| v.0),
            source_angular_velocity.map(|v| v.0),
        )
    };

    let mut player_query = queries.p1();
    let Ok((
        mut player_transform,
        player_position,
        player_rotation,
        player_linear_velocity,
        player_angular_velocity,
    )) = player_query.get_mut(player_entity)
    else {
        return;
    };

    player_transform.translation.x = source_xy.x;
    player_transform.translation.y = source_xy.y;
    player_transform.translation.z = source_z;
    player_transform.rotation = source_transform_rotation;

    if let Some(mut player_position) = player_position {
        player_position.0 = source_xy;
    }
    if let (Some(mut player_rotation), Some(source_rotation)) = (player_rotation, source_rotation) {
        *player_rotation = source_rotation;
    }
    if let (Some(mut player_linear_velocity), Some(source_linear_velocity)) =
        (player_linear_velocity, source_linear_velocity)
    {
        player_linear_velocity.0 = source_linear_velocity;
    }
    if let (Some(mut player_angular_velocity), Some(source_angular_velocity)) =
        (player_angular_velocity, source_angular_velocity)
    {
        player_angular_velocity.0 = source_angular_velocity;
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn lock_camera_to_player_entity_end_of_frame(
    session: Res<'_, ClientSession>,
    entity_registry: Res<'_, RuntimeEntityHierarchy>,
    anchor_query: Query<
        '_,
        '_,
        (&'_ Transform, Option<&'_ Position>),
        (Without<Camera>, Without<GameplayCamera>),
    >,
    mut camera_query: Query<
        '_,
        '_,
        (&'_ mut Transform, &'_ mut TopDownCamera),
        (With<GameplayCamera>, Without<ControlledEntity>),
    >,
) {
    let Some(player_runtime_id) = session.player_entity_id.as_ref() else {
        return;
    };
    let Some(&player_entity) = entity_registry.by_entity_id.get(player_runtime_id.as_str()) else {
        return;
    };
    let Ok((anchor_transform, anchor_position)) = anchor_query.get(player_entity) else {
        return;
    };
    let Ok((mut camera_transform, mut camera)) = camera_query.single_mut() else {
        return;
    };
    let anchor_xy = anchor_position
        .map(|p| p.0)
        .unwrap_or_else(|| anchor_transform.translation.truncate());
    camera.look_ahead_offset = Vec2::ZERO;
    camera.filtered_focus_xy = anchor_xy;
    camera.focus_initialized = true;
    camera_transform.translation.x = anchor_xy.x;
    camera_transform.translation.y = anchor_xy.y;
    camera_transform.translation.z = 80.0;
}
