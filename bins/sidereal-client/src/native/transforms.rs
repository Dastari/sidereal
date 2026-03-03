//! World entity transform sync, interpolation, and player/camera lock.

use avian2d::prelude::{AngularVelocity, LinearVelocity, Position, Rotation};
use bevy::prelude::*;
use sidereal_game::{ControlledEntityGuid, EntityGuid, PlayerTag};
use std::sync::OnceLock;

use super::app_state::{ClientSession, LocalPlayerViewState};
use super::components::{
    ControlledEntity, GameplayCamera, NearbyCollisionProxy, TopDownCamera, WorldEntity,
};
use sidereal_runtime_sync::RuntimeEntityHierarchy;

const REMOTE_VISUAL_SMOOTH_RATE: f32 = 20.0;
const REMOTE_VISUAL_SNAP_THRESHOLD_M: f32 = 64.0;

fn remote_root_anchor_fallback_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("SIDEREAL_CLIENT_REMOTE_ROOT_ANCHOR_FALLBACK")
            .ok()
            .is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"))
    })
}

#[allow(clippy::type_complexity)]
pub(crate) fn sync_world_entity_transforms_from_physics(
    time: Res<'_, Time>,
    mut entities: Query<
        '_,
        '_,
        (
            &mut Transform,
            Option<&Position>,
            Option<&Rotation>,
            Has<lightyear::prelude::Interpolated>,
            Has<avian2d::prelude::RigidBody>,
            Has<NearbyCollisionProxy>,
            Has<ControlledEntity>,
        ),
        (
            With<WorldEntity>,
            Or<(With<Position>, With<Rotation>)>,
            Without<Camera>,
        ),
    >,
) {
    for (
        mut transform,
        position,
        rotation,
        is_interpolated,
        has_rigidbody,
        is_nearby_proxy,
        is_controlled,
    ) in &mut entities
    {
        // Interpolated entities that still carry a local rigid body are synced by the
        // Lightyear/Avian integration path. Nearby collision proxies are a local kinematic
        // exception, so keep fallback transform sync enabled for them.
        if is_interpolated && has_rigidbody && !is_nearby_proxy {
            continue;
        }
        let should_snap = is_controlled || is_nearby_proxy;
        let alpha = 1.0 - (-REMOTE_VISUAL_SMOOTH_RATE * time.delta_secs()).exp();

        if let Some(position) = position {
            if should_snap {
                transform.translation.x = position.0.x;
                transform.translation.y = position.0.y;
            } else {
                let current = transform.translation.truncate();
                let target = position.0;
                if (target - current).length() > REMOTE_VISUAL_SNAP_THRESHOLD_M {
                    transform.translation.x = target.x;
                    transform.translation.y = target.y;
                } else {
                    transform.translation.x += (target.x - current.x) * alpha;
                    transform.translation.y += (target.y - current.y) * alpha;
                }
            }
        }
        if let Some(rotation) = rotation {
            let target: Quat = (*rotation).into();
            if should_snap {
                transform.rotation = target;
            } else {
                transform.rotation = transform.rotation.slerp(target, alpha);
            }
        }
        transform.translation.z = 0.0;
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

#[allow(clippy::type_complexity)]
pub(crate) fn sync_remote_controlled_ship_roots_from_player_anchors(
    session: Res<'_, ClientSession>,
    players: Query<
        '_,
        '_,
        (
            &'_ EntityGuid,
            &'_ ControlledEntityGuid,
            Option<&'_ Position>,
            Option<&'_ Rotation>,
            Option<&'_ LinearVelocity>,
            Option<&'_ AngularVelocity>,
            Option<&'_ Transform>,
        ),
        With<PlayerTag>,
    >,
    mut roots: Query<
        '_,
        '_,
        (
            &'_ EntityGuid,
            Option<&'_ mut Position>,
            Option<&'_ mut Rotation>,
            Option<&'_ mut LinearVelocity>,
            Option<&'_ mut AngularVelocity>,
            Option<&'_ mut Transform>,
        ),
        (
            With<WorldEntity>,
            Without<PlayerTag>,
            Without<ControlledEntity>,
            Without<lightyear::prelude::Predicted>,
        ),
    >,
) {
    if !remote_root_anchor_fallback_enabled() {
        return;
    }

    let local_player_guid = session
        .player_entity_id
        .as_deref()
        .and_then(|raw| uuid::Uuid::parse_str(raw).ok());

    #[derive(Clone, Copy)]
    struct AnchorMotionSample {
        world: Vec2,
        rotation: Option<Rotation>,
        linear_velocity: Option<Vec2>,
        angular_velocity: Option<f32>,
    }

    let mut controlled_motion_by_guid =
        std::collections::HashMap::<uuid::Uuid, AnchorMotionSample>::new();
    for (
        player_guid,
        controlled_guid,
        player_position,
        player_rotation,
        player_linear_velocity,
        player_angular_velocity,
        player_transform,
    ) in &players
    {
        if Some(player_guid.0) == local_player_guid {
            continue;
        }
        let Some(controlled_guid_raw) = controlled_guid.0.as_deref() else {
            continue;
        };
        let Ok(controlled_root_guid) = uuid::Uuid::parse_str(controlled_guid_raw) else {
            continue;
        };
        let world = player_position
            .map(|position| position.0)
            .unwrap_or_else(|| player_transform.map_or(Vec2::ZERO, |t| t.translation.truncate()));
        controlled_motion_by_guid.insert(
            controlled_root_guid,
            AnchorMotionSample {
                world,
                rotation: player_rotation.copied(),
                linear_velocity: player_linear_velocity.map(|velocity| velocity.0),
                angular_velocity: player_angular_velocity.map(|velocity| velocity.0),
            },
        );
    }

    for (
        root_guid,
        root_position,
        root_rotation,
        root_linear_velocity,
        root_angular_velocity,
        root_transform,
    ) in &mut roots
    {
        let Some(sample) = controlled_motion_by_guid.get(&root_guid.0).copied() else {
            continue;
        };
        if let Some(mut position) = root_position {
            position.0 = sample.world;
        }
        if let (Some(mut rotation), Some(source_rotation)) = (root_rotation, sample.rotation) {
            *rotation = source_rotation;
        }
        if let (Some(mut linear_velocity), Some(source_linear_velocity)) =
            (root_linear_velocity, sample.linear_velocity)
        {
            linear_velocity.0 = source_linear_velocity;
        }
        if let (Some(mut angular_velocity), Some(source_angular_velocity)) =
            (root_angular_velocity, sample.angular_velocity)
        {
            angular_velocity.0 = source_angular_velocity;
        }
        if let Some(mut transform) = root_transform {
            transform.translation.x = sample.world.x;
            transform.translation.y = sample.world.y;
            transform.translation.z = 0.0;
            if let Some(source_rotation) = sample.rotation {
                transform.rotation = source_rotation.into();
            }
        }
    }
}
