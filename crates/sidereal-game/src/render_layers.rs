use crate::{
    GeneratedComponentRegistry, RENDER_DOMAIN_FULLSCREEN, RENDER_DOMAIN_POST_PROCESS,
    RENDER_DOMAIN_WORLD_POLYGON, RENDER_DOMAIN_WORLD_SPRITE, RENDER_PHASE_FULLSCREEN_BACKGROUND,
    RENDER_PHASE_FULLSCREEN_FOREGROUND, RENDER_PHASE_POST_PROCESS, RENDER_PHASE_WORLD,
    RuntimePostProcessStack, RuntimeRenderLayerDefinition, RuntimeRenderLayerRule,
    RuntimeWorldVisualStack, is_valid_world_visual_material_domain,
};
use std::collections::HashSet;

pub const DEFAULT_MAIN_WORLD_LAYER_ID: &str = "main_world";

pub fn default_main_world_render_layer() -> RuntimeRenderLayerDefinition {
    RuntimeRenderLayerDefinition {
        layer_id: DEFAULT_MAIN_WORLD_LAYER_ID.to_string(),
        phase: RENDER_PHASE_WORLD.to_string(),
        material_domain: RENDER_DOMAIN_WORLD_SPRITE.to_string(),
        shader_asset_id: String::new(),
        params_asset_id: None,
        texture_bindings: Vec::new(),
        order: 0,
        parallax_factor: Some(1.0),
        screen_scale_factor: Some(1.0),
        depth_bias_z: Some(0.0),
        enabled: true,
    }
}

pub fn is_valid_render_phase(value: &str) -> bool {
    matches!(
        value,
        RENDER_PHASE_FULLSCREEN_BACKGROUND
            | RENDER_PHASE_WORLD
            | RENDER_PHASE_FULLSCREEN_FOREGROUND
            | RENDER_PHASE_POST_PROCESS
    )
}

pub fn is_valid_render_domain(value: &str) -> bool {
    matches!(
        value,
        RENDER_DOMAIN_WORLD_SPRITE
            | RENDER_DOMAIN_WORLD_POLYGON
            | RENDER_DOMAIN_FULLSCREEN
            | RENDER_DOMAIN_POST_PROCESS
    )
}

pub fn is_valid_phase_domain_pair(phase: &str, domain: &str) -> bool {
    match phase {
        RENDER_PHASE_WORLD => matches!(
            domain,
            RENDER_DOMAIN_WORLD_SPRITE | RENDER_DOMAIN_WORLD_POLYGON
        ),
        RENDER_PHASE_FULLSCREEN_BACKGROUND | RENDER_PHASE_FULLSCREEN_FOREGROUND => {
            domain == RENDER_DOMAIN_FULLSCREEN
        }
        RENDER_PHASE_POST_PROCESS => domain == RENDER_DOMAIN_POST_PROCESS,
        _ => false,
    }
}

pub fn validate_runtime_render_layer_definition(
    definition: &RuntimeRenderLayerDefinition,
) -> Result<(), String> {
    if definition.layer_id.trim().is_empty() {
        return Err("layer_id must not be empty".to_string());
    }
    if !is_valid_render_phase(&definition.phase) {
        return Err(format!("unknown render phase '{}'", definition.phase));
    }
    if !is_valid_render_domain(&definition.material_domain) {
        return Err(format!(
            "unknown render material_domain '{}'",
            definition.material_domain
        ));
    }
    if !is_valid_phase_domain_pair(&definition.phase, &definition.material_domain) {
        return Err(format!(
            "render phase '{}' is not compatible with material_domain '{}'",
            definition.phase, definition.material_domain
        ));
    }
    if definition.material_domain != RENDER_DOMAIN_WORLD_SPRITE
        && definition.material_domain != RENDER_DOMAIN_WORLD_POLYGON
        && definition.shader_asset_id.trim().is_empty()
    {
        return Err("shader_asset_id must not be empty for non-world layers".to_string());
    }
    if let Some(parallax_factor) = definition.parallax_factor
        && (!parallax_factor.is_finite() || parallax_factor <= 0.0 || parallax_factor > 4.0)
    {
        return Err(format!(
            "parallax_factor must be finite and within (0, 4], got {parallax_factor}"
        ));
    }
    if let Some(screen_scale_factor) = definition.screen_scale_factor
        && (!screen_scale_factor.is_finite()
            || screen_scale_factor <= 0.0
            || screen_scale_factor > 64.0)
    {
        return Err(format!(
            "screen_scale_factor must be finite and within (0, 64], got {screen_scale_factor}"
        ));
    }
    if let Some(depth_bias_z) = definition.depth_bias_z
        && !depth_bias_z.is_finite()
    {
        return Err("depth_bias_z must be finite when present".to_string());
    }
    if definition
        .params_asset_id
        .as_ref()
        .is_some_and(|v| v.trim().is_empty())
    {
        return Err("params_asset_id must not be blank when present".to_string());
    }
    for binding in &definition.texture_bindings {
        if binding.asset_id.trim().is_empty() {
            return Err("texture_bindings entries must have non-empty asset_id".to_string());
        }
    }
    Ok(())
}

pub fn validate_runtime_render_layer_rule(
    rule: &RuntimeRenderLayerRule,
    known_layer_ids: &HashSet<String>,
    known_component_kinds: &HashSet<String>,
) -> Result<(), String> {
    if rule.rule_id.trim().is_empty() {
        return Err("rule_id must not be empty".to_string());
    }
    if rule.target_layer_id.trim().is_empty() {
        return Err("target_layer_id must not be empty".to_string());
    }
    if !known_layer_ids.contains(&rule.target_layer_id) {
        return Err(format!(
            "target_layer_id '{}' does not reference a known layer",
            rule.target_layer_id
        ));
    }
    let has_matchers = !rule.labels_any.is_empty()
        || !rule.labels_all.is_empty()
        || !rule.archetypes_any.is_empty()
        || !rule.components_all.is_empty()
        || !rule.components_any.is_empty();
    if !has_matchers {
        return Err(format!(
            "render layer rule '{}' must define at least one matcher",
            rule.rule_id
        ));
    }
    for component_kind in rule.components_all.iter().chain(rule.components_any.iter()) {
        if !known_component_kinds.contains(component_kind) {
            return Err(format!(
                "render layer rule '{}' references unknown component kind '{}'",
                rule.rule_id, component_kind
            ));
        }
    }
    Ok(())
}

pub fn validate_runtime_post_process_stack(stack: &RuntimePostProcessStack) -> Result<(), String> {
    let mut pass_ids = HashSet::<String>::new();
    for pass in &stack.passes {
        if pass.pass_id.trim().is_empty() {
            return Err("post-process pass_id must not be empty".to_string());
        }
        if !pass_ids.insert(pass.pass_id.clone()) {
            return Err(format!("duplicate post-process pass_id '{}'", pass.pass_id));
        }
        if pass.shader_asset_id.trim().is_empty() {
            return Err(format!(
                "post-process pass '{}' must define shader_asset_id",
                pass.pass_id
            ));
        }
        if pass
            .params_asset_id
            .as_ref()
            .is_some_and(|v| v.trim().is_empty())
        {
            return Err(format!(
                "post-process pass '{}' has blank params_asset_id",
                pass.pass_id
            ));
        }
        for binding in &pass.texture_bindings {
            if binding.asset_id.trim().is_empty() {
                return Err(format!(
                    "post-process pass '{}' has texture binding with blank asset_id",
                    pass.pass_id
                ));
            }
        }
    }
    Ok(())
}

pub fn validate_runtime_world_visual_stack(stack: &RuntimeWorldVisualStack) -> Result<(), String> {
    let mut pass_ids = HashSet::<String>::new();
    for pass in &stack.passes {
        if pass.pass_id.trim().is_empty() {
            return Err("world visual pass_id must not be empty".to_string());
        }
        if !pass_ids.insert(pass.pass_id.clone()) {
            return Err(format!("duplicate world visual pass_id '{}'", pass.pass_id));
        }
        if pass.visual_family.trim().is_empty() {
            return Err(format!(
                "world visual pass '{}' must define visual_family",
                pass.pass_id
            ));
        }
        if pass.visual_kind.trim().is_empty() {
            return Err(format!(
                "world visual pass '{}' must define visual_kind",
                pass.pass_id
            ));
        }
        if !is_valid_world_visual_material_domain(&pass.material_domain) {
            return Err(format!(
                "world visual pass '{}' has incompatible material_domain '{}'",
                pass.pass_id, pass.material_domain
            ));
        }
        if pass.shader_asset_id.trim().is_empty() {
            return Err(format!(
                "world visual pass '{}' must define shader_asset_id",
                pass.pass_id
            ));
        }
        if pass
            .params_asset_id
            .as_ref()
            .is_some_and(|v| v.trim().is_empty())
        {
            return Err(format!(
                "world visual pass '{}' has blank params_asset_id",
                pass.pass_id
            ));
        }
        if let Some(scale_multiplier) = pass.scale_multiplier
            && (!scale_multiplier.is_finite() || scale_multiplier <= 0.0)
        {
            return Err(format!(
                "world visual pass '{}' has invalid scale_multiplier {}",
                pass.pass_id, scale_multiplier
            ));
        }
        if let Some(depth_bias_z) = pass.depth_bias_z
            && !depth_bias_z.is_finite()
        {
            return Err(format!(
                "world visual pass '{}' has non-finite depth_bias_z",
                pass.pass_id
            ));
        }
        for binding in &pass.texture_bindings {
            if binding.asset_id.trim().is_empty() {
                return Err(format!(
                    "world visual pass '{}' has texture binding with blank asset_id",
                    pass.pass_id
                ));
            }
        }
    }
    Ok(())
}

pub fn known_component_kinds(registry: &GeneratedComponentRegistry) -> HashSet<String> {
    registry
        .entries
        .iter()
        .map(|entry| entry.component_kind.to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        default_main_world_render_layer, validate_runtime_post_process_stack,
        validate_runtime_render_layer_definition, validate_runtime_render_layer_rule,
        validate_runtime_world_visual_stack,
    };
    use crate::{
        RuntimePostProcessPass, RuntimePostProcessStack, RuntimeRenderLayerDefinition,
        RuntimeRenderLayerRule, RuntimeWorldVisualPassDefinition, RuntimeWorldVisualStack,
    };
    use std::collections::HashSet;

    #[test]
    fn default_main_world_layer_is_valid() {
        let layer = default_main_world_render_layer();
        validate_runtime_render_layer_definition(&layer).expect("default main world layer");
    }

    #[test]
    fn reject_invalid_phase_domain_pair() {
        let layer = RuntimeRenderLayerDefinition {
            layer_id: "bad".to_string(),
            phase: "world".to_string(),
            material_domain: "fullscreen".to_string(),
            shader_asset_id: "shader".to_string(),
            ..Default::default()
        };
        let err = validate_runtime_render_layer_definition(&layer).expect_err("invalid layer");
        assert!(err.contains("not compatible"));
    }

    #[test]
    fn reject_unknown_rule_component_kind() {
        let mut known_layer_ids = HashSet::new();
        known_layer_ids.insert("main_world".to_string());
        let mut known_component_kinds = HashSet::new();
        known_component_kinds.insert("size_m".to_string());
        let rule = RuntimeRenderLayerRule {
            rule_id: "ships".to_string(),
            target_layer_id: "main_world".to_string(),
            components_any: vec!["nope".to_string()],
            ..Default::default()
        };
        let err =
            validate_runtime_render_layer_rule(&rule, &known_layer_ids, &known_component_kinds)
                .expect_err("invalid rule");
        assert!(err.contains("unknown component kind"));
    }

    #[test]
    fn reject_duplicate_post_process_pass_ids() {
        let stack = RuntimePostProcessStack {
            passes: vec![
                RuntimePostProcessPass {
                    pass_id: "warp".to_string(),
                    shader_asset_id: "warp_shader".to_string(),
                    ..Default::default()
                },
                RuntimePostProcessPass {
                    pass_id: "warp".to_string(),
                    shader_asset_id: "grade_shader".to_string(),
                    ..Default::default()
                },
            ],
        };
        let err = validate_runtime_post_process_stack(&stack).expect_err("invalid stack");
        assert!(err.contains("duplicate"));
    }

    #[test]
    fn reject_duplicate_world_visual_pass_ids() {
        let stack = RuntimeWorldVisualStack {
            passes: vec![
                RuntimeWorldVisualPassDefinition {
                    pass_id: "body".to_string(),
                    visual_family: "planet".to_string(),
                    visual_kind: "body".to_string(),
                    material_domain: "world_polygon".to_string(),
                    shader_asset_id: "planet_visual_wgsl".to_string(),
                    ..Default::default()
                },
                RuntimeWorldVisualPassDefinition {
                    pass_id: "body".to_string(),
                    visual_family: "planet".to_string(),
                    visual_kind: "cloud_front".to_string(),
                    material_domain: "world_polygon".to_string(),
                    shader_asset_id: "planet_clouds_wgsl".to_string(),
                    ..Default::default()
                },
            ],
        };
        let err = validate_runtime_world_visual_stack(&stack).expect_err("invalid stack");
        assert!(err.contains("duplicate"));
    }
}
