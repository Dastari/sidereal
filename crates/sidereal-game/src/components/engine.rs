use bevy::prelude::*;
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};

use crate::{EntityGuid, MountedOn};

#[sidereal_component_macros::sidereal_component(kind = "engine", persist = true, replicate = true, visibility = [OwnerOnly])]
#[derive(Debug, Clone, Component, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Component, Serialize, Deserialize)]
#[require(EntityGuid, MountedOn)]
pub struct Engine {
    #[serde(alias = "thrust_n")]
    pub thrust: f32,
    #[serde(default, alias = "reverse_thrust_n")]
    pub reverse_thrust: f32,
    #[serde(default, alias = "torque_thrust_nm")]
    pub torque_thrust: f32,
    pub burn_rate_kg_s: f32,
}
