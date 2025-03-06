use bevy::prelude::*;
use bevy::reflect::Reflect;
use bevy_rapier2d::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Component, Serialize, Deserialize, Clone, Debug, Reflect, Default)]
#[reflect(Component, Serialize, Deserialize)]
pub enum RigidBody {
    #[default]
    Dynamic,
    Static,
    Kinematic,
}

#[derive(Component, Serialize, Deserialize, Clone, Debug, Reflect, Default)]
#[require(
    RigidBody,
    Velocity,
    Collider,
    Transform,
    Sleeping,
    Damping,
    GlobalTransform
)]
pub struct PhysicsBody;
