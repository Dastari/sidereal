use bevy::prelude::*;
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};

use crate::EntityGuid;

pub const DEFAULT_DESTRUCTION_PROFILE_ID: &str = "explosion_burst";

fn default_destroy_delay_s() -> f32 {
    0.18
}

#[sidereal_component_macros::sidereal_component(
    kind = "destructible",
    persist = true,
    replicate = false
)]
#[derive(Debug, Clone, Component, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Component, Serialize, Deserialize)]
#[require(EntityGuid)]
pub struct Destructible {
    #[serde(default = "default_profile_id")]
    pub destruction_profile_id: String,
    #[serde(default = "default_destroy_delay_s")]
    pub destroy_delay_s: f32,
}

fn default_profile_id() -> String {
    DEFAULT_DESTRUCTION_PROFILE_ID.to_string()
}

impl Default for Destructible {
    fn default() -> Self {
        Self {
            destruction_profile_id: default_profile_id(),
            destroy_delay_s: default_destroy_delay_s(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingDestructionPhase {
    EffectDelay,
    AwaitDestroyedEventDispatch,
}

#[derive(Debug, Clone, Component, PartialEq)]
pub struct PendingDestruction {
    pub destruction_profile_id: String,
    pub remaining_delay_s: f32,
    pub phase: PendingDestructionPhase,
}
