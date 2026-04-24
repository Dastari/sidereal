use avian2d::prelude::{Position, Rotation};
use bevy::math::DVec2;

use crate::{WorldPosition, WorldRotation};

pub fn resolve_world_position(
    avian_position: Option<&Position>,
    world_position: Option<&WorldPosition>,
) -> Option<DVec2> {
    avian_position
        .map(|value| value.0)
        .or_else(|| world_position.map(|value| value.0))
        .filter(|value| value.is_finite())
}

pub fn resolve_world_rotation_rad(
    avian_rotation: Option<&Rotation>,
    world_rotation: Option<&WorldRotation>,
) -> Option<f64> {
    avian_rotation
        .map(|value| value.as_radians())
        .or_else(|| world_rotation.map(|value| value.0))
        .filter(|value| value.is_finite())
}
