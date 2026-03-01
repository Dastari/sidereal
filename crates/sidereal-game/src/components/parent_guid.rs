use bevy::prelude::*;
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::EntityGuid;

#[sidereal_component_macros::sidereal_component(kind = "parent_guid", persist = true, replicate = true, visibility = [Public])]
#[derive(Debug, Clone, Component, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Component, Serialize, Deserialize)]
#[require(EntityGuid)]
pub struct ParentGuid(pub Uuid);
