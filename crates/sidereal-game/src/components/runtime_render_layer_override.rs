use bevy::prelude::*;
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};

#[sidereal_component_macros::sidereal_component(
    kind = "runtime_render_layer_override",
    persist = true,
    replicate = true,
    visibility = [Public]
)]
#[derive(Debug, Clone, Component, Reflect, Serialize, Deserialize, PartialEq, Eq, Default)]
#[reflect(Component, Serialize, Deserialize)]
pub struct RuntimeRenderLayerOverride {
    pub layer_id: String,
}
