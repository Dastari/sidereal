//! World entity transform sync, interpolation, and player/camera lock.

use avian2d::prelude::{Position, Rotation};
use bevy::prelude::*;
use lightyear::interpolation::interpolation_history::ConfirmedHistory;
use lightyear::prelude::Confirmed;
use sidereal_game::FullscreenLayer;

use super::components::{PendingInitialVisualReady, PendingVisibilityFadeIn, WorldEntity};

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
            .map(|p| p.0.0)
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

/// Keep newly adopted entities hidden until we can render an authoritative pose.
///
/// This prevents transient origin flashes when relevance is gained but interpolation
/// history is not yet ready on the first render frame.
#[allow(clippy::type_complexity)]
pub(crate) fn reveal_world_entities_when_initial_transform_ready(
    mut commands: Commands<'_, '_>,
    mut entities: Query<
        '_,
        '_,
        (
            Entity,
            Has<lightyear::prelude::Interpolated>,
            Option<&'_ Position>,
            Option<&'_ Rotation>,
            Option<&'_ Confirmed<Position>>,
            Option<&'_ Confirmed<Rotation>>,
            Option<&'_ ConfirmedHistory<Position>>,
            Option<&'_ ConfirmedHistory<Rotation>>,
            Option<&'_ FullscreenLayer>,
            &'_ mut Transform,
            &'_ mut Visibility,
        ),
        (With<WorldEntity>, With<PendingInitialVisualReady>),
    >,
) {
    for (
        entity,
        is_interpolated,
        position,
        rotation,
        confirmed_position,
        confirmed_rotation,
        position_history,
        rotation_history,
        fullscreen_layer,
        mut transform,
        mut visibility,
    ) in &mut entities
    {
        let mut ready = false;
        let mut source_position: Option<Vec2> = None;
        let mut source_heading: Option<f32> = None;

        if fullscreen_layer.is_some() {
            // Fullscreen layers are non-spatial overlay entities: they have no physics
            // transform history but should render as soon as adopted.
            ready = true;
        } else if is_interpolated {
            let history_ready = position_history.and_then(|h| h.end()).is_some()
                && rotation_history.and_then(|h| h.end()).is_some();
            if history_ready {
                ready = true;
            } else if let (Some(cp), Some(cr)) = (confirmed_position, confirmed_rotation) {
                source_position = Some(cp.0.0);
                source_heading = Some(cr.0.as_radians());
                ready = true;
            }
        } else if let (Some(p), Some(r)) = (position, rotation) {
            source_position = Some(p.0);
            source_heading = Some(r.as_radians());
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
            transform.translation.x = planar_position.x;
            transform.translation.y = planar_position.y;
            transform.translation.z = 0.0;
            transform.rotation = Quat::from_rotation_z(heading);
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
