use bevy::prelude::*;
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};

use crate::{EntityGuid, PlayerTag};

#[sidereal_component_macros::sidereal_component(kind = "selected_entity_guid", persist = true, replicate = true, visibility = [OwnerOnly])]
#[derive(Debug, Clone, Component, Reflect, Serialize, Deserialize, PartialEq, Eq, Default)]
#[reflect(Component, Serialize, Deserialize)]
#[require(EntityGuid, PlayerTag)]
pub struct SelectedEntityGuid(pub Option<String>);
