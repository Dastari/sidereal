use bevy::prelude::*;
use bevy::reflect::Reflect;
use serde::{Serialize, Deserialize};

#[derive(Component, Clone, Debug, PartialEq, Serialize, Deserialize, Reflect)]
#[reflect(Component)]
pub struct Rotation (pub f32);

impl Default for Rotation {
    fn default() -> Self {
        Self(0.0)
    }
}

impl Rotation {
    pub fn new(value: f32) -> Self {
        Self(value)
    }
}
