use bevy::prelude::*;
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};

use crate::EntityGuid;

#[sidereal_component_macros::sidereal_component(kind = "owner_id", persist = true, replicate = true, visibility = [OwnerOnly])]
#[derive(Debug, Clone, Component, Reflect, Serialize, Deserialize, PartialEq, Eq)]
#[reflect(Component, Serialize, Deserialize)]
#[require(EntityGuid)]
pub struct OwnerId(pub String);
