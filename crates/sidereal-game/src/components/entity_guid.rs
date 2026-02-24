use bevy::prelude::*;
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[sidereal_component_macros::sidereal_component(kind = "entity_guid", persist = true, replicate = true, visibility = [Public])]
#[derive(
    Debug, Clone, Copy, Default, Component, Reflect, Serialize, Deserialize, PartialEq, Eq, Hash,
)]
#[reflect(Component, Serialize, Deserialize)]
pub struct EntityGuid(pub Uuid);
