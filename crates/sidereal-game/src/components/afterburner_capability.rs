use bevy::prelude::*;
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};

use crate::EntityGuid;

#[sidereal_component_macros::sidereal_component(
    kind = "afterburner_capability",
    persist = true,
    replicate = true,
    visibility = [OwnerOnly]
)]
#[derive(Debug, Clone, Component, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Component, Serialize, Deserialize)]
#[require(EntityGuid)]
pub struct AfterburnerCapability {
    pub enabled: bool,
    pub multiplier: f32,
    pub fuel_burn_multiplier: f32,
    #[serde(default)]
    pub max_afterburner_velocity_mps: Option<f32>,
}
