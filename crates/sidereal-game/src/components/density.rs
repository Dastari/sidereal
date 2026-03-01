use bevy::prelude::*;
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};

#[sidereal_component_macros::sidereal_component(
    kind = "density",
    persist = false,
    replicate = false,
    visibility = [OwnerOnly]
)]
#[derive(Debug, Clone, Copy, Component, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Component, Serialize, Deserialize)]
pub struct Density(pub f32);

impl Default for Density {
    fn default() -> Self {
        Self(1.0)
    }
}
