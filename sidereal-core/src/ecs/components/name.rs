use bevy::prelude::*;
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Component, Clone, Debug, PartialEq, Serialize, Deserialize, Reflect)]
#[reflect(Component, Serialize, Deserialize)]
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

// Implement Display trait for Name
impl fmt::Display for Name {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
