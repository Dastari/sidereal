use bevy::prelude::*;
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};

use crate::EntityGuid;

#[sidereal_component_macros::sidereal_component(kind = "cargo_mass_kg", persist = true, replicate = true, visibility = [OwnerOnly])]
#[derive(Debug, Clone, Copy, Component, Reflect, Serialize, Deserialize, PartialEq, Default)]
#[reflect(Component, Serialize, Deserialize)]
#[require(EntityGuid)]
pub struct CargoMassKg(pub f32);
