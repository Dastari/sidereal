#[allow(clippy::type_complexity)]
pub(crate) fn ensure_hierarchy_parent_spatial_components(
    mut commands: Commands<'_, '_>,
    children_with_parent: Query<'_, '_, &'_ ChildOf>,
    parent_components: Query<'_, '_, ParentSpatialQueryItem<'_>>,
) {
    let mut visited_parents = HashSet::<Entity>::new();
    for child_of in &children_with_parent {
        let entity = child_of.parent();
        if !visited_parents.insert(entity) {
            continue;
        }
        let Ok((
            has_transform,
            has_global_transform,
            has_visibility,
            position,
            rotation,
            world_position,
            world_rotation,
        )) = parent_components.get(entity)
        else {
            continue;
        };
        if has_transform && has_global_transform && has_visibility {
            continue;
        }
        let mut transform = Transform::default();
        if let (Some(planar_position), Some(heading)) = (
            resolve_world_position(position, world_position),
            bootstrap_planar_heading(rotation, world_rotation),
        ) && planar_position.is_finite()
            && heading.is_finite()
        {
            transform.translation.x = planar_position.x as f32;
            transform.translation.y = planar_position.y as f32;
            transform.translation.z = 0.0;
            transform.rotation = Quat::from_rotation_z(heading);
        }
        let mut entity_commands = commands.entity(entity);
        if !has_transform {
            entity_commands.insert(transform);
        }
        if !has_global_transform {
            entity_commands.insert(GlobalTransform::from(transform));
        }
        if !has_visibility {
            entity_commands.insert(Visibility::default());
        }
    }
}

pub(crate) fn ensure_parent_spatial_components_on_children_added(
    trigger: On<Add, Children>,
    mut commands: Commands<'_, '_>,
    parent_components: Query<'_, '_, ParentSpatialQueryItem<'_>>,
) {
    let entity = trigger.entity;
    let Ok((
        has_transform,
        has_global_transform,
        has_visibility,
        position,
        rotation,
        world_position,
        world_rotation,
    )) = parent_components.get(entity)
    else {
        return;
    };
    if has_transform && has_global_transform && has_visibility {
        return;
    }
    let mut transform = Transform::default();
    if let (Some(planar_position), Some(heading)) = (
        resolve_world_position(position, world_position),
        bootstrap_planar_heading(rotation, world_rotation),
    ) && planar_position.is_finite()
        && heading.is_finite()
    {
        transform.translation.x = planar_position.x as f32;
        transform.translation.y = planar_position.y as f32;
        transform.translation.z = 0.0;
        transform.rotation = Quat::from_rotation_z(heading);
    }
    let mut entity_commands = commands.entity(entity);
    if !has_transform {
        entity_commands.insert(transform);
    }
    if !has_global_transform {
        entity_commands.insert(GlobalTransform::from(transform));
    }
    if !has_visibility {
        entity_commands.insert(Visibility::default());
    }
}

/// Defensive guard against malformed replicated hierarchy links.
///
/// Server should not replicate cyclic Bevy hierarchy links, but if bad data slips
/// through (for example from migration/script bugs), transform propagation can stack-overflow.
/// This system breaks invalid `ChildOf` links before `TransformSystems::Propagate`.
pub(crate) fn sanitize_invalid_childof_hierarchy_links(
    mut commands: Commands<'_, '_>,
    child_of_query: Query<'_, '_, (Entity, &'_ ChildOf)>,
) {
    if child_of_query.is_empty() {
        return;
    }

    let mut parent_by_child = HashMap::<Entity, Entity>::new();
    for (child, child_of) in &child_of_query {
        parent_by_child.insert(child, child_of.parent());
    }

    const MAX_DEPTH: usize = 256;
    for (child, parent) in parent_by_child.clone() {
        if child == parent {
            bevy::log::warn!(
                "detected self-parent hierarchy link; removing ChildOf child={:?} parent={:?}",
                child,
                parent
            );
            commands.entity(child).remove::<ChildOf>();
            continue;
        }
        let mut seen = HashSet::<Entity>::new();
        let mut cursor = parent;
        let mut cycle = false;
        for _ in 0..MAX_DEPTH {
            if !seen.insert(cursor) {
                cycle = true;
                break;
            }
            let Some(next) = parent_by_child.get(&cursor).copied() else {
                break;
            };
            if next == child {
                cycle = true;
                break;
            }
            cursor = next;
        }
        if cycle {
            bevy::log::warn!(
                "detected cyclic replicated hierarchy; removing ChildOf for child={:?}",
                child
            );
            commands.entity(child).remove::<ChildOf>();
        }
    }
}

