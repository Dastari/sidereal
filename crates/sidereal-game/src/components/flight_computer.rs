use bevy::prelude::*;
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};

use crate::EntityGuid;

#[sidereal_component_macros::sidereal_component(kind = "flight_computer", persist = true, replicate = true, predict = true, visibility = [OwnerOnly])]
#[derive(Debug, Clone, Component, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Component, Serialize, Deserialize)]
#[require(EntityGuid)]
pub struct FlightComputer {
    pub profile: String,
    pub throttle: f32,
    pub yaw_input: f32,
    #[serde(default)]
    pub brake_active: bool,
    pub turn_rate_deg_s: f32,
}
