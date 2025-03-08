use crate::ecs::components::{Hull, Object };
use avian2d::prelude::*;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Component, Reflect, Default, Serialize, Deserialize, Clone)]
#[require(Hull, Object(|| Object::Ship), RigidBody(|| RigidBody::Dynamic), Collider(|| Circle::new(1.0)))]
pub struct Ship;

impl Ship {
    pub fn new() -> Self {
        Self
    }
}
