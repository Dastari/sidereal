use bevy::prelude::*;
use bevy::reflect::Reflect;
use serde::{Serialize, Deserialize};

/// Component to mark entities that should not move due to physics systems
/// These entities will still exert gravitational forces but won't move themselves
#[derive(Component, Clone, Debug, Reflect)]
pub struct Fixed;

#[derive(Component, Clone, Debug, PartialEq, Serialize, Deserialize, Reflect)]
pub struct Mass (pub f32);

impl Default for Mass {
    fn default() -> Self {
        Self(1.0)
    }
}

impl Mass {
    pub fn new(value: f32) -> Self {
        Self(value)
    }
}


#[derive(Component, Clone, Debug, PartialEq,  Serialize, Deserialize, Reflect)]
pub struct Velocity {
    pub x: f32,
    pub y: f32,
}

impl Default for Velocity {
    fn default() -> Self {
        Self { x: 0.0, y: 0.0 }
    }
}   

impl Velocity {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

#[derive(Component, Clone, Debug, PartialEq, Serialize, Deserialize, Reflect)]
pub struct AngularVelocity(pub f32);

impl Default for AngularVelocity {
    fn default() -> Self {
        Self(0.0)
    }
}

impl AngularVelocity {
    pub fn new(value: f32) -> Self {
        Self(value)
    }
}

