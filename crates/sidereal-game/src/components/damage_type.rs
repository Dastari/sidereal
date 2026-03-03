use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Reflect, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum DamageType {
    #[default]
    Ballistic,
}
