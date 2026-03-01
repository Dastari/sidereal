use bevy::prelude::*;
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};

use crate::EntityGuid;

#[sidereal_component_macros::sidereal_component(kind = "flight_tuning", persist = true, replicate = true, predict = true, visibility = [OwnerOnly])]
#[derive(Debug, Clone, Copy, Component, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Component, Serialize, Deserialize)]
#[require(EntityGuid)]
pub struct FlightTuning {
    pub max_linear_accel_mps2: f32,
    pub passive_brake_accel_mps2: f32,
    pub active_brake_accel_mps2: f32,
    pub drag_per_s: f32,
}
