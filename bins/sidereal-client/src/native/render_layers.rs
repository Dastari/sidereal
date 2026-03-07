use bevy::log::warn;
use bevy::prelude::*;
use sidereal_game::{
    DEFAULT_MAIN_WORLD_LAYER_ID, EntityLabels, FullscreenLayer, GeneratedComponentRegistry,
    RENDER_PHASE_WORLD, RuntimePostProcessStack, RuntimeRenderLayerDefinition,
    RuntimeRenderLayerOverride, RuntimeRenderLayerRule, default_main_world_render_layer,
    validate_runtime_post_process_stack, validate_runtime_render_layer_definition,
    validate_runtime_render_layer_rule,
};
use std::collections::{HashMap, HashSet};

use super::components::{ResolvedRuntimeRenderLayer, WorldEntity};
use super::resources::{CompiledRuntimeRenderLayerRule, RuntimeRenderLayerRegistry};

pub(super) fn sync_runtime_render_layer_registry_system(world: &mut World) {
    let Some(generated_registry) = world.get_resource::<GeneratedComponentRegistry>().cloned()
    else {
        return;
    };
    let Some(app_type_registry) = world.get_resource::<AppTypeRegistry>().cloned() else {
        return;
    };

    let component_ids_by_kind = {
        let registry = app_type_registry.read();
        let mut ids = HashMap::new();
        for entry in &generated_registry.entries {
            let Some(type_registration) = registry.get_with_type_path(entry.type_path) else {
                continue;
            };
            let Some(component_id) = world.components().get_id(type_registration.type_id()) else {
                continue;
            };
            ids.insert(entry.component_kind.to_string(), component_id);
        }
        ids
    };

    let definitions = {
        let mut query = world.query::<&RuntimeRenderLayerDefinition>();
        query.iter(world).cloned().collect::<Vec<_>>()
    };
    let rules = {
        let mut query = world.query::<&RuntimeRenderLayerRule>();
        query.iter(world).cloned().collect::<Vec<_>>()
    };
    let post_stacks = {
        let mut query = world.query::<&RuntimePostProcessStack>();
        query.iter(world).cloned().collect::<Vec<_>>()
    };

    let mut definitions_by_id = HashMap::<String, RuntimeRenderLayerDefinition>::new();
    definitions_by_id.insert(
        DEFAULT_MAIN_WORLD_LAYER_ID.to_string(),
        default_main_world_render_layer(),
    );

    for definition in definitions {
        if !definition.enabled {
            continue;
        }
        if let Err(err) = validate_runtime_render_layer_definition(&definition) {
            warn!(
                "ignoring invalid runtime render layer definition layer_id={} error={}",
                definition.layer_id, err
            );
            continue;
        }
        definitions_by_id.insert(definition.layer_id.clone(), definition);
    }

    let known_layer_ids = definitions_by_id.keys().cloned().collect::<HashSet<_>>();
    let known_component_kinds = sidereal_game::known_component_kinds(&generated_registry);
    let mut compiled_rules = Vec::<CompiledRuntimeRenderLayerRule>::new();
    for rule in rules {
        if !rule.enabled {
            continue;
        }
        if let Err(err) =
            validate_runtime_render_layer_rule(&rule, &known_layer_ids, &known_component_kinds)
        {
            warn!(
                "ignoring invalid runtime render layer rule rule_id={} error={}",
                rule.rule_id, err
            );
            continue;
        }
        let Some(target_definition) = definitions_by_id.get(&rule.target_layer_id) else {
            continue;
        };
        if target_definition.phase != RENDER_PHASE_WORLD {
            warn!(
                "ignoring runtime render layer rule rule_id={} target_layer_id={} because target phase is not world",
                rule.rule_id, rule.target_layer_id
            );
            continue;
        }
        compiled_rules.push(CompiledRuntimeRenderLayerRule {
            rule_id: rule.rule_id,
            target_layer_id: rule.target_layer_id,
            priority: rule.priority,
            labels_any: rule.labels_any,
            labels_all: rule.labels_all,
            archetypes_any: rule.archetypes_any,
            components_all: rule
                .components_all
                .iter()
                .filter_map(|kind| component_ids_by_kind.get(kind).copied())
                .collect(),
            components_any: rule
                .components_any
                .iter()
                .filter_map(|kind| component_ids_by_kind.get(kind).copied())
                .collect(),
        });
    }
    compiled_rules.sort_by(|a, b| {
        b.priority
            .cmp(&a.priority)
            .then_with(|| a.rule_id.cmp(&b.rule_id))
    });

    for stack in post_stacks {
        if let Err(err) = validate_runtime_post_process_stack(&stack) {
            warn!("invalid runtime post-process stack ignored for validation only: {err}");
        }
    }

    let new_registry = RuntimeRenderLayerRegistry {
        definitions_by_id,
        world_rules: compiled_rules,
        default_world_layer: default_main_world_render_layer(),
    };

    let should_replace = world
        .get_resource::<RuntimeRenderLayerRegistry>()
        .is_none_or(|existing| {
            existing.definitions_by_id != new_registry.definitions_by_id
                || existing.world_rules.len() != new_registry.world_rules.len()
                || existing
                    .world_rules
                    .iter()
                    .zip(&new_registry.world_rules)
                    .any(|(a, b)| a.rule_id != b.rule_id || a.target_layer_id != b.target_layer_id)
        });
    if should_replace {
        world.insert_resource(new_registry);
    }
}

pub(super) fn resolve_runtime_render_layer_assignments_system(world: &mut World) {
    let Some(registry) = world.get_resource::<RuntimeRenderLayerRegistry>().cloned() else {
        return;
    };

    let entities = {
        let mut query = world.query_filtered::<(
            Entity,
            Option<&EntityLabels>,
            Option<&RuntimeRenderLayerOverride>,
            Option<&ResolvedRuntimeRenderLayer>,
        ), (
            With<WorldEntity>,
            Without<RuntimeRenderLayerDefinition>,
            Without<RuntimeRenderLayerRule>,
            Without<RuntimePostProcessStack>,
            Without<FullscreenLayer>,
        )>();
        query
            .iter(world)
            .map(|(entity, labels, override_layer, current)| {
                (
                    entity,
                    labels.cloned(),
                    override_layer.cloned(),
                    current.cloned(),
                )
            })
            .collect::<Vec<_>>()
    };

    for (entity, labels, override_layer, current) in entities {
        let Some(entity_ref) = world.get_entity(entity).ok() else {
            continue;
        };
        let labels = labels.map(|value| value.0).unwrap_or_default();
        let desired_definition = resolve_world_layer_definition(
            &registry,
            &entity_ref,
            &labels,
            override_layer.as_ref(),
        );
        let desired = ResolvedRuntimeRenderLayer {
            layer_id: desired_definition.layer_id.clone(),
            definition: desired_definition,
        };
        if current.as_ref() != Some(&desired) {
            world.entity_mut(entity).insert(desired);
        }
    }
}

fn resolve_world_layer_definition(
    registry: &RuntimeRenderLayerRegistry,
    entity_ref: &EntityRef<'_>,
    labels: &[String],
    override_layer: Option<&RuntimeRenderLayerOverride>,
) -> RuntimeRenderLayerDefinition {
    if let Some(override_layer) = override_layer
        && let Some(definition) = registry.definitions_by_id.get(&override_layer.layer_id)
        && definition.enabled
        && definition.phase == RENDER_PHASE_WORLD
    {
        return definition.clone();
    }

    for rule in &registry.world_rules {
        if matches_rule(rule, entity_ref, labels)
            && let Some(definition) = registry.definitions_by_id.get(&rule.target_layer_id)
        {
            return definition.clone();
        }
    }

    registry.default_world_layer.clone()
}

fn matches_rule(
    rule: &CompiledRuntimeRenderLayerRule,
    entity_ref: &EntityRef<'_>,
    labels: &[String],
) -> bool {
    let contains_label = |needle: &str| labels.iter().any(|label| label == needle);
    if !rule.labels_any.is_empty() && !rule.labels_any.iter().any(|value| contains_label(value)) {
        return false;
    }
    if !rule.labels_all.is_empty() && !rule.labels_all.iter().all(|value| contains_label(value)) {
        return false;
    }
    if !rule.archetypes_any.is_empty()
        && !rule
            .archetypes_any
            .iter()
            .any(|value| contains_label(value))
    {
        return false;
    }
    if !rule
        .components_all
        .iter()
        .all(|component_id| entity_ref.contains_id(*component_id))
    {
        return false;
    }
    if !rule.components_any.is_empty()
        && !rule
            .components_any
            .iter()
            .any(|component_id| entity_ref.contains_id(*component_id))
    {
        return false;
    }
    true
}
