use crate::ecs::components::{Id, Sector};
use avian2d::prelude::*;
use bevy::prelude::*;
use bevy_reflect::Reflect;
use serde::{Deserialize, Serialize};

#[derive(Component, Reflect, Serialize, Deserialize, Default)]
#[require(
    Name,
    Id,
    Transform,
    LinearVelocity,
    AngularVelocity,
    RigidBody,
    Sector
)]
#[reflect(Component, Serialize, Deserialize)]
pub enum Object {
    #[default]
    Debris, // Debris from a destroyed object
    Ship,
    Asteroid,
    Projectile,
}
