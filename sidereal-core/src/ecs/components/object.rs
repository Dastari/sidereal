use crate::ecs::components::Id;
use bevy::prelude::*;
use bevy_reflect::Reflect;
use serde::{Deserialize, Serialize};

#[derive(Component, Reflect, Serialize, Deserialize, Default)]
#[require(Name, Id)]
#[reflect(Component, Serialize, Deserialize)]
pub enum Object {
    #[default]
    Debris, // Debris from a destroyed object
    Ship,
    Asteroid,
    Projectile,
}
