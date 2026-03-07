use bevy::prelude::*;
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};

use super::runtime_render_layer_definition::RuntimeTextureBinding;

fn default_enabled() -> bool {
    true
}

#[derive(Debug, Clone, Reflect, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct RuntimePostProcessPass {
    pub pass_id: String,
    pub shader_asset_id: String,
    #[serde(default)]
    pub params_asset_id: Option<String>,
    #[serde(default)]
    pub texture_bindings: Vec<RuntimeTextureBinding>,
    #[serde(default)]
    pub order: i32,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

#[sidereal_component_macros::sidereal_component(
    kind = "runtime_post_process_stack",
    persist = true,
    replicate = true,
    visibility = [Public]
)]
#[derive(Debug, Clone, Component, Reflect, Serialize, Deserialize, PartialEq, Default)]
#[reflect(Component, Serialize, Deserialize)]
pub struct RuntimePostProcessStack {
    #[serde(default)]
    pub passes: Vec<RuntimePostProcessPass>,
}
