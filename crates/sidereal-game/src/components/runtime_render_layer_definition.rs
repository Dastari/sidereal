use bevy::prelude::*;
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};

pub const RENDER_PHASE_FULLSCREEN_BACKGROUND: &str = "fullscreen_background";
pub const RENDER_PHASE_WORLD: &str = "world";
pub const RENDER_PHASE_FULLSCREEN_FOREGROUND: &str = "fullscreen_foreground";
pub const RENDER_PHASE_POST_PROCESS: &str = "post_process";

pub const RENDER_DOMAIN_WORLD_SPRITE: &str = "world_sprite";
pub const RENDER_DOMAIN_WORLD_POLYGON: &str = "world_polygon";
pub const RENDER_DOMAIN_FULLSCREEN: &str = "fullscreen";
pub const RENDER_DOMAIN_POST_PROCESS: &str = "post_process";

fn default_enabled() -> bool {
    true
}

#[derive(Debug, Clone, Reflect, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct RuntimeTextureBinding {
    #[serde(default)]
    pub slot: u32,
    pub asset_id: String,
}

#[sidereal_component_macros::sidereal_component(
    kind = "runtime_render_layer_definition",
    persist = true,
    replicate = true,
    visibility = [Public]
)]
#[derive(Debug, Clone, Component, Reflect, Serialize, Deserialize, PartialEq, Default)]
#[reflect(Component, Serialize, Deserialize)]
pub struct RuntimeRenderLayerDefinition {
    pub layer_id: String,
    pub phase: String,
    pub material_domain: String,
    pub shader_asset_id: String,
    #[serde(default)]
    pub params_asset_id: Option<String>,
    #[serde(default)]
    pub texture_bindings: Vec<RuntimeTextureBinding>,
    #[serde(default)]
    pub order: i32,
    #[serde(default)]
    pub parallax_factor: Option<f32>,
    #[serde(default)]
    pub screen_scale_factor: Option<f32>,
    #[serde(default)]
    pub depth_bias_z: Option<f32>,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

#[cfg(test)]
mod tests {
    use super::{
        RENDER_DOMAIN_FULLSCREEN, RENDER_PHASE_FULLSCREEN_BACKGROUND, RuntimeRenderLayerDefinition,
    };

    #[test]
    fn runtime_render_layer_definition_deserializes_defaults() {
        let settings = serde_json::from_str::<RuntimeRenderLayerDefinition>(
            r#"{
                "layer_id":"bg_space",
                "phase":"fullscreen_background",
                "material_domain":"fullscreen",
                "shader_asset_id":"space_background_wgsl"
            }"#,
        )
        .expect("layer definition");
        assert_eq!(settings.phase, RENDER_PHASE_FULLSCREEN_BACKGROUND);
        assert_eq!(settings.material_domain, RENDER_DOMAIN_FULLSCREEN);
        assert!(settings.enabled);
        assert!(settings.texture_bindings.is_empty());
    }
}
