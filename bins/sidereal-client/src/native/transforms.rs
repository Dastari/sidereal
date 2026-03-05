//! World entity transform sync, interpolation, and player/camera lock.

use avian2d::prelude::{Position, Rotation};
use bevy::prelude::*;
use lightyear::interpolation::interpolation_history::ConfirmedHistory;
use lightyear::prelude::Confirmed;

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

/// Bootstrap for interpolated entities that just became relevant but do not yet
/// have interpolation history samples. Without this, they can render at default
/// Transform (0,0) until the next server delta arrives.
#[allow(clippy::type_complexity)]
pub(crate) fn sync_interpolated_world_entity_transforms_without_history(
    mut entities: Query<
        '_,
        '_,
        (
            Option<&'_ Position>,
            Option<&'_ Rotation>,
            Option<&'_ Confirmed<Position>>,
            Option<&'_ Confirmed<Rotation>>,
            &'_ mut Transform,
            Option<&'_ ConfirmedHistory<Position>>,
            Option<&'_ ConfirmedHistory<Rotation>>,
        ),
        (With<WorldEntity>, With<lightyear::prelude::Interpolated>),
    >,
) {
    for (
        position,
        rotation,
        confirmed_position,
        confirmed_rotation,
        mut transform,
        position_history,
        rotation_history,
    ) in &mut entities
    {
        // Interpolation needs at least 2 samples. With only one (or zero), preserve
        // authoritative spawn pose from Confirmed values so entities don't render at origin.
        let history_ready = position_history.and_then(|h| h.end()).is_some()
            && rotation_history.and_then(|h| h.end()).is_some();
        if history_ready {
            continue;
        }
        let source_position = confirmed_position
            .map(|p| p.0 .0)
            .or_else(|| position.map(|p| p.0));
        let source_heading = confirmed_rotation
            .map(|r| r.0.as_radians())
            .or_else(|| rotation.map(|r| r.as_radians()));
        let planar_position = if source_position.is_some_and(|p| p.is_finite()) {
            source_position.unwrap_or(Vec2::ZERO)
        } else {
            Vec2::ZERO
        };
        let heading = if source_heading.is_some_and(|r| r.is_finite()) {
            source_heading.unwrap_or(0.0)
        } else {
            0.0
        };
        transform.translation.x = planar_position.x;
        transform.translation.y = planar_position.y;
        transform.translation.z = 0.0;
        transform.rotation = Quat::from_rotation_z(heading);
    }
}
