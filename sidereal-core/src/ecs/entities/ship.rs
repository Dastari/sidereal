use crate::ecs::components::{Hull, Object, PhysicsBody};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Component, Reflect, Default, Serialize, Deserialize, Clone)]
#[require(Hull, PhysicsBody, Object(|| Object::Ship))]
pub struct Ship;

impl Ship {
    pub fn new() -> Self {
        Self
    }
}
