use bevy::prelude::*;
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};

use crate::EntityGuid;

pub const SPACE_BACKGROUND_LAYER_KIND: &str = "space_background";
pub const STARFIELD_LAYER_KIND: &str = "starfield";

#[sidereal_component_macros::sidereal_component(kind = "fullscreen_layer", persist = true, replicate = true, visibility = [Public])]
#[derive(Debug, Clone, Component, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Component, Serialize, Deserialize)]
#[require(EntityGuid)]
pub struct FullscreenLayer {
    pub layer_kind: String,
    pub shader_asset_id: String,
    pub layer_order: i32,
}
