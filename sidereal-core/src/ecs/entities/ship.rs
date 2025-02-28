use bevy::prelude::*;
use crate::ecs::components::{Hull, Position, Velocity, Rotation, AngularRotation};
#[derive(Bundle)]
struct ShipBundle {
    name: Name,
    position: Position, 
    velocity: Velocity,
    rotation: Rotation,
    angular_rotation: AngularRotation,
    hull: Hull,
}
