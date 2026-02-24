use bevy::prelude::*;
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::EntityGuid;

#[sidereal_component_macros::sidereal_component(kind = "mounted_on", persist = true, replicate = true, visibility = [Public])]
#[derive(Debug, Clone, Default, Component, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Component, Serialize, Deserialize)]
#[require(EntityGuid)]
pub struct MountedOn {
    pub parent_entity_id: Uuid,
    pub hardpoint_id: String,
}
