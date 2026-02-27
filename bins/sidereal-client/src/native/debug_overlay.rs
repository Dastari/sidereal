//! F3 debug overlay: toggle and draw (AABB, velocity arrows, visibility circle).

use avian2d::prelude::{LinearVelocity, Position, Rotation};
use bevy::ecs::query::Has;
use bevy::prelude::*;
use sidereal_game::{EntityGuid, Hardpoint, MountedOn, ScannerRangeM, SizeM};
use sidereal_runtime_sync::RuntimeEntityHierarchy;

use super::components::{ControlledEntity, WorldEntity};
use super::resources::DebugOverlayEnabled;
use super::state::{ClientSession, LocalPlayerViewState};

pub(crate) fn toggle_debug_overlay_system(
    input: Res<'_, ButtonInput<KeyCode>>,
    mut debug_overlay: ResMut<'_, DebugOverlayEnabled>,
) {
    if input.just_pressed(KeyCode::F3) {
        debug_overlay.enabled = !debug_overlay.enabled;
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
            &'_ Transform,
            Option<&'_ SizeM>,
            Option<&'_ LinearVelocity>,
            Option<&'_ MountedOn>,
            Option<&'_ Hardpoint>,
            Option<&'_ ControlledEntity>,
            Option<&'_ ScannerRangeM>,
            Option<&'_ EntityGuid>,
            Option<&'_ lightyear::prelude::Confirmed<Position>>,
            Option<&'_ lightyear::prelude::Confirmed<Rotation>>,
            Has<lightyear::prelude::Predicted>,
            Has<lightyear::prelude::Replicated>,
            Has<lightyear::prelude::Interpolated>,
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
        transform,
        size_m,
        linear_velocity,
        mounted_on,
        hardpoint,
        controlled_marker,
        scanner_range,
        _entity_guid,
        confirmed_position,
        confirmed_rotation,
        _is_predicted,
        _is_replicated,
        _is_interpolated,
    ) in &entities
    {
        let pos = transform.translation;
        let rot = transform.rotation;
        let half_extents =
            size_m.map(|size| Vec3::new(size.width * 0.5, size.length * 0.5, size.height * 0.5));

        let is_local_controlled = (mounted_on.is_none()
            && hardpoint.is_none()
            && Some(entity) == local_controlled_entity)
            || controlled_marker.is_some_and(|controlled| {
                session
                    .player_entity_id
                    .as_deref()
                    .is_some_and(|player_id| controlled.player_entity_id == player_id)
            });

        if let Some(half_extents) = half_extents {
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
