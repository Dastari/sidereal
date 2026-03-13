use bevy::ecs::component::ComponentId;
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
use std::hash::{Hash, Hasher};

use super::components::{ResolvedRuntimeRenderLayer, WorldEntity};
use super::resources::{
    CachedRuntimeRenderLayerAssignment, CompiledRuntimeRenderLayerRule, RenderLayerPerfCounters,
    RuntimeRenderLayerAssignmentCache, RuntimeRenderLayerRegistry, RuntimeRenderLayerRegistryState,
};

pub(super) fn sync_runtime_render_layer_registry_system(world: &mut World) {
    let mut perf = world
        .remove_resource::<RenderLayerPerfCounters>()
        .unwrap_or_default();
    perf.registry_sync_runs = perf.registry_sync_runs.saturating_add(1);

    let Some(generated_registry) = world.get_resource::<GeneratedComponentRegistry>().cloned()
    else {
        world.insert_resource(perf);
        return;
    };
    let Some(app_type_registry) = world.get_resource::<AppTypeRegistry>().cloned() else {
        world.insert_resource(perf);
        return;
    };
    let mut registry_state = world
        .remove_resource::<RuntimeRenderLayerRegistryState>()
        .unwrap_or_default();
    let generated_registry_signature = hash_generated_registry(&generated_registry);
    let definition_count = count_components::<RuntimeRenderLayerDefinition>(world);
    let rule_count = count_components::<RuntimeRenderLayerRule>(world);
    let post_process_stack_count = count_components::<RuntimePostProcessStack>(world);
    let authored_state_changed = has_any_authored_render_layer_changes(world, &mut registry_state)
        || registry_state.generated_registry_signature != generated_registry_signature
        || registry_state.definition_count != definition_count
        || registry_state.rule_count != rule_count
        || registry_state.post_process_stack_count != post_process_stack_count;
    if !authored_state_changed {
        registry_state.generated_registry_signature = generated_registry_signature;
        registry_state.definition_count = definition_count;
        registry_state.rule_count = rule_count;
        registry_state.post_process_stack_count = post_process_stack_count;
        world.insert_resource(registry_state);
        world.insert_resource(perf);
        return;
    }

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

    let watched_component_ids = collect_watched_component_ids(&compiled_rules);
    let new_registry = RuntimeRenderLayerRegistry {
        definitions_by_id,
        world_rules: compiled_rules,
        watched_component_ids,
        default_world_layer: default_main_world_render_layer(),
    };

    let should_replace = world
        .get_resource::<RuntimeRenderLayerRegistry>()
        .is_none_or(|existing| {
            existing.definitions_by_id != new_registry.definitions_by_id
                || existing.world_rules.len() != new_registry.world_rules.len()
                || existing.watched_component_ids != new_registry.watched_component_ids
                || existing
                    .world_rules
                    .iter()
                    .zip(&new_registry.world_rules)
                    .any(|(a, b)| a.rule_id != b.rule_id || a.target_layer_id != b.target_layer_id)
        });
    if should_replace {
        world.insert_resource(new_registry);
        registry_state.generation = registry_state.generation.saturating_add(1);
        perf.registry_rebuilds = perf.registry_rebuilds.saturating_add(1);
    }
    registry_state.generated_registry_signature = generated_registry_signature;
    registry_state.definition_count = definition_count;
    registry_state.rule_count = rule_count;
    registry_state.post_process_stack_count = post_process_stack_count;
    world.insert_resource(registry_state);
    world.insert_resource(perf);
}

pub(super) fn resolve_runtime_render_layer_assignments_system(world: &mut World) {
    let Some(registry) = world.get_resource::<RuntimeRenderLayerRegistry>().cloned() else {
        return;
    };
    let registry_generation = world
        .get_resource::<RuntimeRenderLayerRegistryState>()
        .map(|state| state.generation)
        .unwrap_or_default();
    let mut perf = world
        .remove_resource::<RenderLayerPerfCounters>()
        .unwrap_or_default();
    perf.assignment_sync_runs = perf.assignment_sync_runs.saturating_add(1);
    let mut cache = world
        .remove_resource::<RuntimeRenderLayerAssignmentCache>()
        .unwrap_or_default();
    let requires_full_scan = registry_generation_changed(&cache, registry_generation);
    let entities_to_process = if requires_full_scan {
        perf.assignment_full_scans = perf.assignment_full_scans.saturating_add(1);
        collect_all_render_layer_assignment_entities(world)
    } else {
        perf.assignment_targeted_scans = perf.assignment_targeted_scans.saturating_add(1);
        collect_dirty_render_layer_assignment_entities(world, &registry, &mut cache)
    };
    let mut pending_resolved_updates = Vec::<(Entity, ResolvedRuntimeRenderLayer)>::new();
    let mut seen_entities = HashSet::<Entity>::new();

    for entity in entities_to_process {
        seen_entities.insert(entity);
        perf.assignment_entities_considered = perf.assignment_entities_considered.saturating_add(1);
        let Some(entity_ref) = world.get_entity(entity).ok() else {
            continue;
        };
        if !is_runtime_render_layer_assignment_target(&entity_ref) {
            continue;
        }
        let labels = entity_ref.get::<EntityLabels>();
        let override_layer = entity_ref.get::<RuntimeRenderLayerOverride>();
        let current = entity_ref.get::<ResolvedRuntimeRenderLayer>();
        let input_hash = assignment_input_hash(
            labels.map(|value| &value.0),
            override_layer,
            &entity_ref,
            &registry.watched_component_ids,
        );
        let should_recompute = current.is_none()
            || cache.by_entity.get(&entity).is_none_or(|cached| {
                cached.registry_generation != registry_generation || cached.input_hash != input_hash
            });
        if !should_recompute {
            perf.assignment_skips = perf.assignment_skips.saturating_add(1);
            continue;
        }
        let labels = labels.map(|value| value.0.as_slice()).unwrap_or(&[]);
        let desired_definition =
            resolve_world_layer_definition(&registry, &entity_ref, labels, override_layer);
        let desired = ResolvedRuntimeRenderLayer {
            layer_id: desired_definition.layer_id.clone(),
            definition: desired_definition,
        };
        if Some(&desired) != current {
            pending_resolved_updates.push((entity, desired));
        }
        cache.by_entity.insert(
            entity,
            CachedRuntimeRenderLayerAssignment {
                registry_generation,
                input_hash,
            },
        );
        perf.assignment_recomputes = perf.assignment_recomputes.saturating_add(1);
    }
    for (entity, desired) in pending_resolved_updates {
        world.entity_mut(entity).insert(desired);
    }
    cache
        .by_entity
        .retain(|entity, _| seen_entities.contains(entity) || world.get_entity(*entity).is_ok());
    cache
        .watched_component_removal_cursors
        .retain(|component_id, _| registry.watched_component_ids.contains(component_id));
    cache.last_world_entity_count = count_world_render_layer_entities(world);
    world.insert_resource(cache);
    world.insert_resource(perf);
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

fn count_components<T: Component>(world: &mut World) -> usize {
    let mut query = world.query::<&T>();
    query.iter(world).count()
}

fn count_world_render_layer_entities(world: &mut World) -> usize {
    let mut query = world.query_filtered::<Entity, With<WorldEntity>>();
    query.iter(world).count()
}

fn has_any_authored_render_layer_changes(
    world: &mut World,
    registry_state: &mut RuntimeRenderLayerRegistryState,
) -> bool {
    has_any_component_changes::<RuntimeRenderLayerDefinition>(world)
        || has_any_component_changes::<RuntimeRenderLayerRule>(world)
        || has_any_component_changes::<RuntimePostProcessStack>(world)
        || has_any_removed_components::<RuntimeRenderLayerDefinition>(
            world,
            &mut registry_state.definition_removal_cursor,
        )
        || has_any_removed_components::<RuntimeRenderLayerRule>(
            world,
            &mut registry_state.rule_removal_cursor,
        )
        || has_any_removed_components::<RuntimePostProcessStack>(
            world,
            &mut registry_state.post_process_stack_removal_cursor,
        )
}

fn has_any_component_changes<T: Component>(world: &mut World) -> bool {
    let mut query = world.query_filtered::<Entity, Or<(Added<T>, Changed<T>)>>();
    query.iter(world).next().is_some()
}

fn has_any_removed_components<T: Component>(
    world: &mut World,
    cursor: &mut Option<
        bevy::ecs::message::MessageCursor<bevy::ecs::lifecycle::RemovedComponentEntity>,
    >,
) -> bool {
    let Some(component_id) = world.component_id::<T>() else {
        return false;
    };
    let Some(events) = world.removed_components().get(component_id) else {
        return false;
    };
    let reader = cursor.get_or_insert_with(Default::default);
    reader.read(events).next().is_some()
}

fn hash_generated_registry(registry: &GeneratedComponentRegistry) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    registry.entries.len().hash(&mut hasher);
    for entry in &registry.entries {
        entry.component_kind.hash(&mut hasher);
        entry.type_path.hash(&mut hasher);
    }
    hasher.finish()
}

fn collect_watched_component_ids(
    compiled_rules: &[CompiledRuntimeRenderLayerRule],
) -> Vec<ComponentId> {
    let mut watched = Vec::<ComponentId>::new();
    for rule in compiled_rules {
        for component_id in rule
            .components_all
            .iter()
            .chain(rule.components_any.iter())
            .copied()
        {
            if !watched.contains(&component_id) {
                watched.push(component_id);
            }
        }
    }
    watched.sort_by_key(|component_id| format!("{component_id:?}"));
    watched.dedup();
    watched
}

fn registry_generation_changed(
    cache: &RuntimeRenderLayerAssignmentCache,
    registry_generation: u64,
) -> bool {
    cache
        .by_entity
        .values()
        .next()
        .is_some_and(|cached| cached.registry_generation != registry_generation)
}

fn collect_all_render_layer_assignment_entities(world: &mut World) -> Vec<Entity> {
    let mut query = world.query_filtered::<Entity, (
        With<WorldEntity>,
        Without<RuntimeRenderLayerDefinition>,
        Without<RuntimeRenderLayerRule>,
        Without<RuntimePostProcessStack>,
        Without<FullscreenLayer>,
    )>();
    query.iter(world).collect()
}

fn collect_dirty_render_layer_assignment_entities(
    world: &mut World,
    registry: &RuntimeRenderLayerRegistry,
    cache: &mut RuntimeRenderLayerAssignmentCache,
) -> Vec<Entity> {
    let mut dirty = HashSet::<Entity>::new();

    let mut added_world_entities = world.query_filtered::<Entity, (
        Added<WorldEntity>,
        Without<RuntimeRenderLayerDefinition>,
        Without<RuntimeRenderLayerRule>,
        Without<RuntimePostProcessStack>,
        Without<FullscreenLayer>,
    )>();
    dirty.extend(added_world_entities.iter(world));

    let mut changed_labels = world.query_filtered::<Entity, (
        Or<(Added<EntityLabels>, Changed<EntityLabels>)>,
        With<WorldEntity>,
        Without<RuntimeRenderLayerDefinition>,
        Without<RuntimeRenderLayerRule>,
        Without<RuntimePostProcessStack>,
        Without<FullscreenLayer>,
    )>();
    dirty.extend(changed_labels.iter(world));

    let mut changed_overrides = world.query_filtered::<Entity, (
        Or<(
            Added<RuntimeRenderLayerOverride>,
            Changed<RuntimeRenderLayerOverride>,
        )>,
        With<WorldEntity>,
        Without<RuntimeRenderLayerDefinition>,
        Without<RuntimeRenderLayerRule>,
        Without<RuntimePostProcessStack>,
        Without<FullscreenLayer>,
    )>();
    dirty.extend(changed_overrides.iter(world));

    let mut unresolved = world.query_filtered::<Entity, (
        With<WorldEntity>,
        Without<ResolvedRuntimeRenderLayer>,
        Without<RuntimeRenderLayerDefinition>,
        Without<RuntimeRenderLayerRule>,
        Without<RuntimePostProcessStack>,
        Without<FullscreenLayer>,
    )>();
    dirty.extend(unresolved.iter(world));
    collect_removed_assignment_input_entities::<EntityLabels>(
        world,
        &mut cache.label_removal_cursor,
        &mut dirty,
    );
    collect_removed_assignment_input_entities::<RuntimeRenderLayerOverride>(
        world,
        &mut cache.override_removal_cursor,
        &mut dirty,
    );
    collect_watched_component_dirty_entities(world, registry, cache, &mut dirty);

    dirty.into_iter().collect()
}

fn collect_removed_assignment_input_entities<T: Component>(
    world: &mut World,
    cursor: &mut Option<
        bevy::ecs::message::MessageCursor<bevy::ecs::lifecycle::RemovedComponentEntity>,
    >,
    dirty: &mut HashSet<Entity>,
) {
    let Some(component_id) = world.component_id::<T>() else {
        return;
    };
    let Some(events) = world.removed_components().get(component_id) else {
        return;
    };
    let reader = cursor.get_or_insert_with(Default::default);
    for event in reader.read(events) {
        dirty.insert(Entity::from(event.clone()));
    }
}

fn collect_watched_component_dirty_entities(
    world: &mut World,
    registry: &RuntimeRenderLayerRegistry,
    cache: &mut RuntimeRenderLayerAssignmentCache,
    dirty: &mut HashSet<Entity>,
) {
    if registry.watched_component_ids.is_empty() {
        return;
    }

    let last_change_tick = world.last_change_tick();
    let read_change_tick = world.read_change_tick();
    for archetype in world.archetypes().iter() {
        let watched_components = registry
            .watched_component_ids
            .iter()
            .copied()
            .filter(|component_id| archetype.contains(*component_id))
            .collect::<Vec<_>>();
        if watched_components.is_empty() {
            continue;
        }

        for archetype_entity in archetype.entities() {
            let entity = archetype_entity.id();
            let Some(entity_ref) = world.get_entity(entity).ok() else {
                continue;
            };
            if !is_runtime_render_layer_assignment_target(&entity_ref) {
                continue;
            }
            if watched_components.iter().any(|component_id| {
                entity_ref
                    .get_change_ticks_by_id(*component_id)
                    .is_some_and(|ticks| ticks.is_changed(last_change_tick, read_change_tick))
            }) {
                dirty.insert(entity);
            }
        }
    }

    let removed_components = world.removed_components();
    for component_id in &registry.watched_component_ids {
        let Some(events) = removed_components.get(*component_id) else {
            continue;
        };
        let cursor = cache
            .watched_component_removal_cursors
            .entry(*component_id)
            .or_default();
        for event in cursor.read(events) {
            dirty.insert(Entity::from(event.clone()));
        }
    }
}

fn is_runtime_render_layer_assignment_target(entity_ref: &EntityRef<'_>) -> bool {
    entity_ref.contains::<WorldEntity>()
        && !entity_ref.contains::<RuntimeRenderLayerDefinition>()
        && !entity_ref.contains::<RuntimeRenderLayerRule>()
        && !entity_ref.contains::<RuntimePostProcessStack>()
        && !entity_ref.contains::<FullscreenLayer>()
}

fn assignment_input_hash(
    labels: Option<&Vec<String>>,
    override_layer: Option<&RuntimeRenderLayerOverride>,
    entity_ref: &EntityRef<'_>,
    watched_component_ids: &[ComponentId],
) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    labels
        .map(|value| value.as_slice())
        .unwrap_or(&[])
        .hash(&mut hasher);
    override_layer
        .map(|value| value.layer_id.as_str())
        .unwrap_or("")
        .hash(&mut hasher);
    for component_id in watched_component_ids {
        entity_ref.contains_id(*component_id).hash(&mut hasher);
    }
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::{
        resolve_runtime_render_layer_assignments_system, sync_runtime_render_layer_registry_system,
    };
    use crate::runtime::components::{ResolvedRuntimeRenderLayer, WorldEntity};
    use crate::runtime::resources::{
        CompiledRuntimeRenderLayerRule, RenderLayerPerfCounters, RuntimeRenderLayerAssignmentCache,
        RuntimeRenderLayerRegistry, RuntimeRenderLayerRegistryState,
    };
    use bevy::prelude::*;
    use sidereal_game::{EntityLabels, GeneratedComponentRegistry};
    use std::collections::HashMap;

    #[derive(Component, Reflect, Default)]
    #[reflect(Component)]
    struct TestWatchedComponent;

    #[test]
    fn registry_rebuild_is_skipped_when_authored_state_is_unchanged() {
        let mut app = App::new();
        app.insert_resource(GeneratedComponentRegistry {
            entries: Vec::new(),
            shader_entries: Vec::new(),
        });
        app.init_resource::<RuntimeRenderLayerRegistry>();
        app.init_resource::<RuntimeRenderLayerRegistryState>();
        app.init_resource::<RenderLayerPerfCounters>();
        app.add_systems(Update, sync_runtime_render_layer_registry_system);

        app.update();
        let first_generation = app
            .world()
            .resource::<RuntimeRenderLayerRegistryState>()
            .generation;
        app.update();

        let registry_state = app.world().resource::<RuntimeRenderLayerRegistryState>();
        assert_eq!(first_generation, 1);
        assert_eq!(registry_state.generation, first_generation);
    }

    #[test]
    fn registry_rebuilds_when_authored_rule_is_removed() {
        let mut app = App::new();
        app.insert_resource(GeneratedComponentRegistry {
            entries: Vec::new(),
            shader_entries: Vec::new(),
        });
        app.init_resource::<RuntimeRenderLayerRegistry>();
        app.init_resource::<RuntimeRenderLayerRegistryState>();
        app.init_resource::<RenderLayerPerfCounters>();
        app.add_systems(Update, sync_runtime_render_layer_registry_system);

        let rule_entity = app
            .world_mut()
            .spawn(sidereal_game::RuntimeRenderLayerRule {
                rule_id: "temporary_rule".to_string(),
                target_layer_id: sidereal_game::DEFAULT_MAIN_WORLD_LAYER_ID.to_string(),
                enabled: true,
                labels_any: vec!["temporary".to_string()],
                ..default()
            })
            .id();

        app.update();
        let first_generation = app
            .world()
            .resource::<RuntimeRenderLayerRegistryState>()
            .generation;
        app.world_mut()
            .entity_mut(rule_entity)
            .remove::<sidereal_game::RuntimeRenderLayerRule>();
        app.update();

        let registry_state = app.world().resource::<RuntimeRenderLayerRegistryState>();
        assert_eq!(first_generation, 1);
        assert_eq!(registry_state.generation, first_generation + 1);
    }

    #[test]
    fn label_removal_uses_targeted_reassignment() {
        let mut app = App::new();
        app.insert_resource(GeneratedComponentRegistry {
            entries: Vec::new(),
            shader_entries: Vec::new(),
        });
        app.init_resource::<RuntimeRenderLayerRegistry>();
        app.init_resource::<RuntimeRenderLayerRegistryState>();
        app.init_resource::<RuntimeRenderLayerAssignmentCache>();
        app.init_resource::<RenderLayerPerfCounters>();
        app.add_systems(
            Update,
            (
                sync_runtime_render_layer_registry_system,
                resolve_runtime_render_layer_assignments_system
                    .after(sync_runtime_render_layer_registry_system),
            ),
        );

        app.world_mut().spawn((
            sidereal_game::RuntimeRenderLayerDefinition {
                layer_id: "planet_layer".to_string(),
                enabled: true,
                phase: sidereal_game::RENDER_PHASE_WORLD.to_string(),
                ..default()
            },
            sidereal_game::RuntimeRenderLayerRule {
                rule_id: "planet_rule".to_string(),
                target_layer_id: "planet_layer".to_string(),
                enabled: true,
                priority: 10,
                labels_any: vec!["planet".to_string()],
                ..default()
            },
        ));

        let entity = app
            .world_mut()
            .spawn((WorldEntity, EntityLabels(vec!["planet".to_string()])))
            .id();

        app.update();
        app.world_mut().entity_mut(entity).remove::<EntityLabels>();
        app.update();

        let resolved = app
            .world()
            .entity(entity)
            .get::<ResolvedRuntimeRenderLayer>()
            .expect("resolved render layer should be present");
        assert_eq!(
            resolved.layer_id,
            sidereal_game::DEFAULT_MAIN_WORLD_LAYER_ID
        );
    }

    #[test]
    fn watched_component_changes_use_targeted_dirty_pass() {
        let mut app = App::new();
        let watched_component_id = app.world_mut().register_component::<TestWatchedComponent>();
        let default_world_layer = sidereal_game::default_main_world_render_layer();
        let watched_layer = sidereal_game::RuntimeRenderLayerDefinition {
            layer_id: "watched_layer".to_string(),
            enabled: true,
            phase: sidereal_game::RENDER_PHASE_WORLD.to_string(),
            ..default()
        };
        let mut definitions_by_id = HashMap::new();
        definitions_by_id.insert(
            sidereal_game::DEFAULT_MAIN_WORLD_LAYER_ID.to_string(),
            default_world_layer.clone(),
        );
        definitions_by_id.insert("watched_layer".to_string(), watched_layer);
        app.insert_resource(RuntimeRenderLayerRegistry {
            definitions_by_id,
            world_rules: vec![CompiledRuntimeRenderLayerRule {
                rule_id: "watched_rule".to_string(),
                target_layer_id: "watched_layer".to_string(),
                priority: 10,
                labels_any: Vec::new(),
                labels_all: Vec::new(),
                archetypes_any: Vec::new(),
                components_all: Vec::new(),
                components_any: vec![watched_component_id],
            }],
            watched_component_ids: vec![watched_component_id],
            default_world_layer,
        });
        app.insert_resource(RuntimeRenderLayerRegistryState {
            generation: 1,
            ..default()
        });
        app.init_resource::<RuntimeRenderLayerAssignmentCache>();
        app.init_resource::<RenderLayerPerfCounters>();
        app.add_systems(Update, resolve_runtime_render_layer_assignments_system);

        let entity = app.world_mut().spawn(WorldEntity).id();

        app.update();
        app.world_mut()
            .entity_mut(entity)
            .insert(TestWatchedComponent);
        app.update();

        let resolved = app
            .world()
            .entity(entity)
            .get::<ResolvedRuntimeRenderLayer>()
            .expect("resolved render layer should be present");
        assert_eq!(resolved.layer_id, "watched_layer");
    }

    #[test]
    fn override_removal_uses_targeted_reassignment() {
        let mut app = App::new();
        app.insert_resource(GeneratedComponentRegistry {
            entries: Vec::new(),
            shader_entries: Vec::new(),
        });
        app.init_resource::<RuntimeRenderLayerRegistry>();
        app.init_resource::<RuntimeRenderLayerRegistryState>();
        app.init_resource::<RuntimeRenderLayerAssignmentCache>();
        app.init_resource::<RenderLayerPerfCounters>();
        app.add_systems(
            Update,
            (
                sync_runtime_render_layer_registry_system,
                resolve_runtime_render_layer_assignments_system
                    .after(sync_runtime_render_layer_registry_system),
            ),
        );

        app.world_mut()
            .spawn(sidereal_game::RuntimeRenderLayerDefinition {
                layer_id: "override_layer".to_string(),
                enabled: true,
                phase: sidereal_game::RENDER_PHASE_WORLD.to_string(),
                ..default()
            });

        let entity = app
            .world_mut()
            .spawn((
                WorldEntity,
                sidereal_game::RuntimeRenderLayerOverride {
                    layer_id: "override_layer".to_string(),
                },
            ))
            .id();

        app.update();
        app.world_mut()
            .entity_mut(entity)
            .remove::<sidereal_game::RuntimeRenderLayerOverride>();
        app.update();

        let resolved = app
            .world()
            .entity(entity)
            .get::<ResolvedRuntimeRenderLayer>()
            .expect("resolved render layer should be present");
        assert_eq!(
            resolved.layer_id,
            sidereal_game::DEFAULT_MAIN_WORLD_LAYER_ID
        );
    }
}
