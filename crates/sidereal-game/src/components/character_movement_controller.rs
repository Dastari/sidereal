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
pub struct CharacterMovementController {
    pub speed_mps: f32,
}
