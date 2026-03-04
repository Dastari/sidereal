//! World entity transform sync, interpolation, and player/camera lock.

use avian2d::prelude::{Position, Rotation};
use bevy::prelude::*;

use super::components::WorldEntity;

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
            Vec2::ZERO
        };
        let heading = if rotation.is_finite() {
            rotation.as_radians()
        } else {
            0.0
        };
        transform.translation.x = planar_position.x;
        transform.translation.y = planar_position.y;
        transform.translation.z = 0.0;
        transform.rotation = Quat::from_rotation_z(heading);
    }
}
