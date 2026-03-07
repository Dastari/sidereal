use bevy::prelude::*;
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};

use super::runtime_render_layer_definition::{
    RENDER_DOMAIN_WORLD_POLYGON, RENDER_DOMAIN_WORLD_SPRITE, RuntimeTextureBinding,
};

fn default_enabled() -> bool {
    true
}

#[derive(Debug, Clone, Reflect, Serialize, Deserialize, PartialEq, Default)]
pub struct RuntimeWorldVisualPassDefinition {
    pub pass_id: String,
    pub visual_family: String,
    pub visual_kind: String,
    pub material_domain: String,
    pub shader_asset_id: String,
    #[serde(default)]
    pub params_asset_id: Option<String>,
    #[serde(default)]
    pub texture_bindings: Vec<RuntimeTextureBinding>,
    #[serde(default)]
    pub order: i32,
    #[serde(default)]
    pub scale_multiplier: Option<f32>,
    #[serde(default)]
    pub depth_bias_z: Option<f32>,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

#[sidereal_component_macros::sidereal_component(
    kind = "runtime_world_visual_stack",
    persist = true,
    replicate = true,
    visibility = [Public]
)]
#[derive(Debug, Clone, Component, Reflect, Serialize, Deserialize, PartialEq, Default)]
#[reflect(Component, Serialize, Deserialize)]
pub struct RuntimeWorldVisualStack {
    #[serde(default)]
    pub passes: Vec<RuntimeWorldVisualPassDefinition>,
}

pub fn is_valid_world_visual_material_domain(value: &str) -> bool {
    matches!(
        value,
        RENDER_DOMAIN_WORLD_SPRITE | RENDER_DOMAIN_WORLD_POLYGON
    )
}

#[cfg(test)]
mod tests {
    use super::{RuntimeWorldVisualPassDefinition, RuntimeWorldVisualStack};

    #[test]
    fn runtime_world_visual_stack_deserializes_defaults() {
        let stack = serde_json::from_str::<RuntimeWorldVisualStack>(
            r#"{
                "passes": [
                    {
                        "pass_id":"body",
                        "visual_family":"planet",
                        "visual_kind":"body",
                        "material_domain":"world_polygon",
                        "shader_asset_id":"planet_visual_wgsl"
                    }
                ]
            }"#,
        )
        .expect("visual stack");
        assert_eq!(stack.passes.len(), 1);
        let pass = &stack.passes[0];
        assert!(pass.enabled);
        assert!(pass.texture_bindings.is_empty());
        assert_eq!(pass.visual_family, "planet");
    }

    #[test]
    fn runtime_world_visual_pass_default_is_enabled() {
        let pass = RuntimeWorldVisualPassDefinition {
            pass_id: "body".to_string(),
            visual_family: "planet".to_string(),
            visual_kind: "body".to_string(),
            material_domain: "world_polygon".to_string(),
            shader_asset_id: "planet_visual_wgsl".to_string(),
            ..Default::default()
        };
        assert!(pass.enabled);
    }
}
