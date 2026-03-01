use bevy::prelude::*;
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};

#[sidereal_component_macros::sidereal_component(kind = "entity_labels", persist = true, replicate = true, visibility = [Public])]
#[derive(Debug, Clone, Component, Reflect, Serialize, Deserialize, PartialEq, Eq, Default)]
#[reflect(Component, Serialize, Deserialize)]
pub struct EntityLabels(pub Vec<String>);
