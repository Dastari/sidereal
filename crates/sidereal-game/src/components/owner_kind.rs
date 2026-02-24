use bevy::prelude::{ReflectDeserialize, ReflectSerialize};
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, Reflect, Serialize, Deserialize, PartialEq, Eq)]
#[reflect(Serialize, Deserialize)]
pub enum OwnerKind {
    Player,
    Faction,
    World,
    #[default]
    Unowned,
}
