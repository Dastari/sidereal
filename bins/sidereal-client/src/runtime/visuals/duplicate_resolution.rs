pub(super) fn suppress_duplicate_predicted_interpolated_visuals_system(world: &mut World) {
    let mut state = world
        .remove_resource::<DuplicateVisualResolutionState>()
        .unwrap_or_default();

    collect_duplicate_visual_membership_changes(world, &mut state);
    collect_duplicate_visual_dirty_guid_changes(world, &mut state);

    let dirty_guids = if state.dirty_all {
        state.entities_by_guid.keys().copied().collect::<Vec<_>>()
    } else {
        state.dirty_guids.iter().copied().collect::<Vec<_>>()
    };

    for guid in dirty_guids {
        recompute_duplicate_visual_group(world, &mut state, guid);
    }

    state.dirty_guids.clear();
    state.dirty_all = false;
    state.duplicate_guid_groups = state
        .entities_by_guid
        .values()
        .filter(|entities| entities.len() > 1)
        .count();
    world.insert_resource(state);
}

fn collect_duplicate_visual_membership_changes(
    world: &mut World,
    state: &mut DuplicateVisualResolutionState,
) {
    if state.dirty_all {
        state.guid_by_entity.clear();
        state.entities_by_guid.clear();
        let mut query = world.query_filtered::<(Entity, &EntityGuid), With<WorldEntity>>();
        for (entity, guid) in query.iter(world) {
            state.guid_by_entity.insert(entity, guid.0);
            state
                .entities_by_guid
                .entry(guid.0)
                .or_default()
                .insert(entity);
            state.dirty_guids.insert(guid.0);
        }
        return;
    }

    let mut added_or_changed_guid = world.query_filtered::<(Entity, &EntityGuid), (
        With<WorldEntity>,
        Or<(Added<WorldEntity>, Added<EntityGuid>, Changed<EntityGuid>)>,
    )>();
    for (entity, guid) in added_or_changed_guid.iter(world) {
        let new_guid = guid.0;
        if let Some(previous_guid) = state.guid_by_entity.insert(entity, new_guid)
            && previous_guid != new_guid
        {
            if let Some(entities) = state.entities_by_guid.get_mut(&previous_guid) {
                entities.remove(&entity);
                if entities.is_empty() {
                    state.entities_by_guid.remove(&previous_guid);
                }
            }
            state.dirty_guids.insert(previous_guid);
        }
        state
            .entities_by_guid
            .entry(new_guid)
            .or_default()
            .insert(entity);
        state.dirty_guids.insert(new_guid);
    }

    let removed_entity_guid_entities = read_removed_duplicate_visual_entities::<EntityGuid>(
        world,
        &mut state.entity_guid_removal_cursor,
    );
    for entity in removed_entity_guid_entities {
        remove_duplicate_visual_membership_for_entity(state, entity);
    }
    let removed_world_entities = read_removed_duplicate_visual_entities::<WorldEntity>(
        world,
        &mut state.world_entity_removal_cursor,
    );
    for entity in removed_world_entities {
        remove_duplicate_visual_membership_for_entity(state, entity);
    }
}

fn remove_duplicate_visual_membership_for_entity(
    state: &mut DuplicateVisualResolutionState,
    entity: Entity,
) {
    if let Some(previous_guid) = state.guid_by_entity.remove(&entity) {
        if let Some(entities) = state.entities_by_guid.get_mut(&previous_guid) {
            entities.remove(&entity);
            if entities.is_empty() {
                state.entities_by_guid.remove(&previous_guid);
            }
        }
        state.dirty_guids.insert(previous_guid);
    }
}

fn collect_duplicate_visual_dirty_guid_changes(
    world: &mut World,
    state: &mut DuplicateVisualResolutionState,
) {
    mark_dirty_duplicate_visual_guids_for_changes::<ControlledEntityGuid>(world, state);
    mark_dirty_duplicate_visual_guids_for_changes::<PlayerTag>(world, state);
    mark_dirty_duplicate_visual_guids_for_changes::<ControlledEntity>(world, state);
    mark_dirty_duplicate_visual_guids_for_changes::<lightyear::prelude::Interpolated>(world, state);
    mark_dirty_duplicate_visual_guids_for_changes::<lightyear::prelude::Predicted>(world, state);
    mark_dirty_duplicate_visual_guids_for_changes::<Position>(world, state);
    mark_dirty_duplicate_visual_guids_for_changes::<Rotation>(world, state);
    mark_dirty_duplicate_visual_guids_for_changes::<WorldPosition>(world, state);
    mark_dirty_duplicate_visual_guids_for_changes::<WorldRotation>(world, state);
    mark_dirty_duplicate_visual_guids_for_changes::<Confirmed<Position>>(world, state);
    mark_dirty_duplicate_visual_guids_for_changes::<Confirmed<Rotation>>(world, state);
    mark_dirty_duplicate_visual_guids_for_additions::<ConfirmedHistory<avian2d::prelude::Position>>(
        world, state,
    );
    mark_dirty_duplicate_visual_guids_for_additions::<ConfirmedHistory<avian2d::prelude::Rotation>>(
        world, state,
    );

    for entity in read_removed_duplicate_visual_entities::<ControlledEntityGuid>(
        world,
        &mut state.controlled_entity_guid_removal_cursor,
    ) {
        mark_duplicate_visual_entity_guid_dirty(state, entity);
    }
    for entity in read_removed_duplicate_visual_entities::<PlayerTag>(
        world,
        &mut state.player_tag_removal_cursor,
    ) {
        mark_duplicate_visual_entity_guid_dirty(state, entity);
    }
    for entity in read_removed_duplicate_visual_entities::<ControlledEntity>(
        world,
        &mut state.controlled_entity_removal_cursor,
    ) {
        mark_duplicate_visual_entity_guid_dirty(state, entity);
    }
    for entity in read_removed_duplicate_visual_entities::<lightyear::prelude::Interpolated>(
        world,
        &mut state.interpolated_removal_cursor,
    ) {
        mark_duplicate_visual_entity_guid_dirty(state, entity);
    }
    for entity in read_removed_duplicate_visual_entities::<lightyear::prelude::Predicted>(
        world,
        &mut state.predicted_removal_cursor,
    ) {
        mark_duplicate_visual_entity_guid_dirty(state, entity);
    }
    for entity in read_removed_duplicate_visual_entities::<
        ConfirmedHistory<avian2d::prelude::Position>,
    >(world, &mut state.position_history_removal_cursor)
    {
        mark_duplicate_visual_entity_guid_dirty(state, entity);
    }
    for entity in read_removed_duplicate_visual_entities::<
        ConfirmedHistory<avian2d::prelude::Rotation>,
    >(world, &mut state.rotation_history_removal_cursor)
    {
        mark_duplicate_visual_entity_guid_dirty(state, entity);
    }
}

fn mark_dirty_duplicate_visual_guids_for_changes<T: Component>(
    world: &mut World,
    state: &mut DuplicateVisualResolutionState,
) {
    let mut query =
        world.query_filtered::<Entity, (With<WorldEntity>, Or<(Added<T>, Changed<T>)>)>();
    for entity in query.iter(world) {
        if let Some(guid) = state.guid_by_entity.get(&entity).copied() {
            state.dirty_guids.insert(guid);
        }
    }
}

fn mark_dirty_duplicate_visual_guids_for_additions<T: Component>(
    world: &mut World,
    state: &mut DuplicateVisualResolutionState,
) {
    let mut query = world.query_filtered::<Entity, (With<WorldEntity>, Added<T>)>();
    for entity in query.iter(world) {
        if let Some(guid) = state.guid_by_entity.get(&entity).copied() {
            state.dirty_guids.insert(guid);
        }
    }
}

fn mark_duplicate_visual_entity_guid_dirty(
    state: &mut DuplicateVisualResolutionState,
    entity: Entity,
) {
    if let Some(guid) = state.guid_by_entity.get(&entity).copied() {
        state.dirty_guids.insert(guid);
    }
}

fn read_removed_duplicate_visual_entities<T: Component>(
    world: &mut World,
    cursor: &mut Option<
        bevy::ecs::message::MessageCursor<bevy::ecs::lifecycle::RemovedComponentEntity>,
    >,
) -> Vec<Entity> {
    let Some(component_id) = world.component_id::<T>() else {
        return Vec::new();
    };
    let Some(events) = world.removed_components().get(component_id) else {
        return Vec::new();
    };
    let reader = cursor.get_or_insert_with(Default::default);
    reader
        .read(events)
        .map(|event| Entity::from(event.clone()))
        .collect()
}

fn recompute_duplicate_visual_group(
    world: &mut World,
    state: &mut DuplicateVisualResolutionState,
    guid: uuid::Uuid,
) {
    let member_entities = state
        .entities_by_guid
        .get(&guid)
        .cloned()
        .unwrap_or_default();
    let mut best_entity = None::<(Entity, i32)>;
    let mut live_entities = std::collections::HashSet::<Entity>::new();

    for entity in member_entities {
        let Some(entity_ref) = world.get_entity(entity).ok() else {
            continue;
        };
        if !entity_ref.contains::<WorldEntity>() {
            continue;
        }
        live_entities.insert(entity);
        let force_suppress =
            entity_ref.contains::<ControlledEntityGuid>() || entity_ref.contains::<PlayerTag>();
        if force_suppress {
            continue;
        }

        let is_controlled = entity_ref.contains::<ControlledEntity>();
        let is_interpolated = entity_ref.contains::<lightyear::prelude::Interpolated>();
        let is_predicted = entity_ref.contains::<lightyear::prelude::Predicted>();
        let interpolated_ready = interpolated_presentation_ready(
            entity_ref.get::<Position>(),
            entity_ref.get::<Rotation>(),
            entity_ref.get::<WorldPosition>(),
            entity_ref.get::<WorldRotation>(),
            entity_ref.get::<Confirmed<Position>>(),
            entity_ref.get::<Confirmed<Rotation>>(),
            entity_ref.get::<ConfirmedHistory<avian2d::prelude::Position>>(),
            entity_ref.get::<ConfirmedHistory<avian2d::prelude::Rotation>>(),
        );
        let score = if is_controlled {
            3
        } else if is_interpolated && interpolated_ready {
            2
        } else if is_predicted {
            1
        } else if is_interpolated {
            -1
        } else {
            0
        };
        match best_entity {
            Some((winner, winner_score))
                if score < winner_score
                    || (score == winner_score && entity.to_bits() >= winner.to_bits()) => {}
            _ => {
                best_entity = Some((entity, score));
            }
        }
    }

    if live_entities.is_empty() {
        state.entities_by_guid.remove(&guid);
        state.winner_by_guid.remove(&guid);
        return;
    }

    state.entities_by_guid.insert(guid, live_entities.clone());
    let previous_winner = state.winner_by_guid.get(&guid).copied();
    if let Some((winner, _)) = best_entity {
        if previous_winner != Some(winner) {
            state.winner_swap_count = state.winner_swap_count.saturating_add(1);
        }
        state.winner_by_guid.insert(guid, winner);
    } else {
        state.winner_by_guid.remove(&guid);
    }

    for entity in live_entities {
        let Some(entity_ref) = world.get_entity(entity).ok() else {
            continue;
        };
        let should_suppress = entity_ref.contains::<ControlledEntityGuid>()
            || entity_ref.contains::<PlayerTag>()
            || state
                .winner_by_guid
                .get(&guid)
                .is_some_and(|winner| *winner != entity);
        let is_suppressed = entity_ref.contains::<SuppressedPredictedDuplicateVisual>();
        let is_canonical = entity_ref.contains::<CanonicalPresentationEntity>();
        let mut entity_mut = world.entity_mut(entity);
        if should_suppress {
            if !is_suppressed {
                entity_mut.insert(SuppressedPredictedDuplicateVisual);
            }
            if is_canonical {
                entity_mut.remove::<CanonicalPresentationEntity>();
            }
            entity_mut.insert(Visibility::Hidden);
        } else {
            if is_suppressed {
                entity_mut.remove::<SuppressedPredictedDuplicateVisual>();
                entity_mut.insert(Visibility::Visible);
            }
            if !is_canonical {
                entity_mut.insert(CanonicalPresentationEntity);
            }
        }
    }
}

