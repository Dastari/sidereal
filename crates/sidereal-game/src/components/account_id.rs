use bevy::prelude::*;
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};

use crate::{EntityGuid, PlayerTag};

#[sidereal_component_macros::sidereal_component(kind = "account_id", persist = true, replicate = true, visibility = [OwnerOnly])]
#[derive(Debug, Clone, Component, Reflect, Serialize, Deserialize, PartialEq, Eq)]
#[reflect(Component, Serialize, Deserialize)]
#[require(EntityGuid, PlayerTag)]
pub struct AccountId(pub String);
