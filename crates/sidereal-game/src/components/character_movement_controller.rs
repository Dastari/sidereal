use bevy::prelude::*;
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};

use crate::{EntityGuid, PlayerTag};

#[sidereal_component_macros::sidereal_component(
    kind = "character_movement_controller",
    persist = true,
    replicate = true,
    visibility = [OwnerOnly]
)]
#[derive(Debug, Clone, Component, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Component, Serialize, Deserialize)]
#[require(EntityGuid, PlayerTag)]
#[serde(default)]
pub struct CharacterMovementController {
    /// Maximum planar movement speed in meters/second.
    pub speed_mps: f32,
    /// Maximum planar acceleration in meters/second^2.
    pub max_accel_mps2: f32,
    /// Exponential damping coefficient applied when no input is present.
    pub damping_per_s: f32,
}

impl Default for CharacterMovementController {
    fn default() -> Self {
        Self {
            speed_mps: 220.0,
            max_accel_mps2: 880.0,
            damping_per_s: 8.0,
        }
    }
}
