use bevy::prelude::*;
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};

#[derive(Component, Clone, Debug, PartialEq, Serialize, Deserialize, Reflect)]
pub struct Name(pub String);

impl Default for Name {
    fn default() -> Self {
        Self(String::from("Unnamed"))
    }
}

impl Name {
    pub fn new(name: &str) -> Self {
        Self(String::from(name))
    }
}
