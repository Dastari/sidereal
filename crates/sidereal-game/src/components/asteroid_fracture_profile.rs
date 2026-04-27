use bevy::prelude::*;
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};

#[sidereal_component_macros::sidereal_component(
    kind = "asteroid_fracture_profile",
    persist = true,
    replicate = false
)]
#[derive(Debug, Clone, Component, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Component, Serialize, Deserialize)]
pub struct AsteroidFractureProfile {
    pub break_massive_into_large_min: u8,
    pub break_massive_into_large_max: u8,
    pub break_large_into_medium_min: u8,
    pub break_large_into_medium_max: u8,
    pub break_medium_into_small_min: u8,
    pub break_medium_into_small_max: u8,
    pub child_impulse_min_mps: f32,
    pub child_impulse_max_mps: f32,
    pub mass_retention_ratio: f32,
    pub terminal_debris_loss_ratio: f32,
}
