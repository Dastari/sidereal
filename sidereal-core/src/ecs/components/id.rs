use bevy::prelude::*;
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

#[derive(Component, Clone, Debug, PartialEq, Serialize, Deserialize, Reflect)]
#[reflect(Component, Serialize, Deserialize)]
pub struct Id(pub Uuid);

impl Default for Id {
    fn default() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Id {
    pub fn new(id: Option<Uuid>) -> Self {
        match id {
            Some(id) => {
                Self(id)
            },
            None => Self(Uuid::new_v4()),
        }
    }
}

// Implement Display trait for EntityId
impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
