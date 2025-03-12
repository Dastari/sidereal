use crate::ecs::systems::sectors::SectorCoord;
use bevy::prelude::*;
use bevy_reflect::Reflect;
use serde::{Deserialize, Serialize};
#[derive(Component, Clone, Debug, PartialEq, Serialize, Deserialize, Reflect)]
#[reflect(Component, Serialize, Deserialize)]
pub struct InSector(pub SectorCoord);
