//! Local hierarchy reconstruction from replicated UUID relationships.
//!
//! Bevy's ChildOf/Children cannot be replicated through Lightyear because
//! Entity references are local to each Bevy world and entity mapping order
//! is undefined on the receiving side. Instead, UUID-based link components
//! carry parentage and mount intent across network boundaries.
//!
//! This system reconstructs Bevy hierarchy locally on each world (server
//! and client) so Bevy's transform propagation produces correct
//! GlobalTransform for mounted/rendered entities.

use bevy::log::warn;
use bevy::prelude::*;
use std::collections::HashMap;
use uuid::Uuid;

use crate::{EntityGuid, Hardpoint, ParentGuid};

const MAX_PARENT_CHAIN_DEPTH: usize = 256;

fn would_create_parent_cycle(
    child_guid: Uuid,
    parent_guid: Uuid,
    parent_by_guid: &HashMap<Uuid, Uuid>,
) -> bool {
    if child_guid == parent_guid {
        return true;
    }
    let mut cursor = parent_guid;
    for _ in 0..MAX_PARENT_CHAIN_DEPTH {
        let Some(next) = parent_by_guid.get(&cursor).copied() else {
            return false;
        };
        if next == child_guid {
            return true;
        }
        if next == cursor {
            return true;
        }
        cursor = next;
    }
    true
}

/// Establishes Bevy parent-child hierarchy from `ParentGuid`.
///
/// For each entity without a Bevy parent (no `ChildOf`), looks up a parent
/// by stable GUID and calls `add_child`.
///
/// Entities whose parents have not yet spawned are silently skipped and
/// retried on subsequent frames (the `Without<ChildOf>` filter re-includes
/// them automatically).
#[allow(clippy::type_complexity)]
pub fn sync_mounted_hierarchy(
    mut commands: Commands<'_, '_>,
    updated_hardpoints: Query<'_, '_, (Entity, &'_ Hardpoint), (With<ChildOf>, Changed<Hardpoint>)>,
    unmounted: Query<
        '_,
        '_,
        (Entity, &'_ ParentGuid, Option<&'_ Hardpoint>),
        (Without<ChildOf>, With<ParentGuid>),
    >,
    guid_entities: Query<
        '_,
        '_,
        (
            Entity,
            &'_ EntityGuid,
            Option<&'_ ParentGuid>,
            Has<Transform>,
            Has<GlobalTransform>,
            Has<Visibility>,
        ),
    >,
) {
    for (entity, hardpoint) in &updated_hardpoints {
        commands.entity(entity).insert(
            Transform::from_translation(hardpoint.offset_m).with_rotation(hardpoint.local_rotation),
        );
    }

    if unmounted.is_empty() {
        return;
    }

    let mut entity_by_guid = HashMap::<Uuid, Entity>::new();
    let mut parent_by_guid = HashMap::<Uuid, Uuid>::new();
    for (entity, guid, maybe_parent, _, _, _) in guid_entities.iter() {
        entity_by_guid.insert(guid.0, entity);
        if let Some(parent) = maybe_parent {
            parent_by_guid.insert(guid.0, parent.0);
        }
    }

    for (child_entity, parent_guid, hardpoint) in &unmounted {
        let Ok((_, child_entity_guid, _, _, _, _)) = guid_entities.get(child_entity) else {
            continue;
        };
        let child_guid = child_entity_guid.0;
        let target_parent_guid = parent_guid.0;
        if would_create_parent_cycle(child_guid, target_parent_guid, &parent_by_guid) {
            warn!(
                "skipping hierarchy link to avoid cycle child_guid={} parent_guid={}",
                child_guid, target_parent_guid
            );
            continue;
        }

        let Some(&parent_entity) = entity_by_guid.get(&target_parent_guid) else {
            continue;
        };

        let Ok((_, _, _, has_transform, has_global_transform, has_visibility)) =
            guid_entities.get(parent_entity)
        else {
            continue;
        };
        let mut parent_commands = commands.entity(parent_entity);
        if !has_transform {
            parent_commands.insert(Transform::default());
        }
        if !has_global_transform {
            parent_commands.insert(GlobalTransform::default());
        }
        if !has_visibility {
            parent_commands.insert(Visibility::default());
        }
        commands.entity(parent_entity).add_child(child_entity);

        // Track accepted links in this frame so subsequent checks are safe
        // even when multiple links are pending in the same command buffer.
        parent_by_guid.insert(child_guid, target_parent_guid);

        if let Some(hardpoint) = hardpoint {
            commands.entity(child_entity).insert(
                Transform::from_translation(hardpoint.offset_m)
                    .with_rotation(hardpoint.local_rotation),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{sync_mounted_hierarchy, would_create_parent_cycle};
    use crate::{EntityGuid, Hardpoint, ParentGuid};
    use bevy::prelude::*;
    use std::collections::HashMap;
    use uuid::Uuid;

    #[test]
    fn detects_simple_cycle() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let mut parents = HashMap::new();
        parents.insert(b, a);
        assert!(would_create_parent_cycle(a, b, &parents));
    }

    #[test]
    fn allows_acyclic_parent_link() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let c = Uuid::new_v4();
        let mut parents = HashMap::new();
        parents.insert(c, b);
        parents.insert(b, a);
        assert!(!would_create_parent_cycle(c, a, &parents));
    }

    #[test]
    fn updates_hardpoint_local_transform_when_offset_changes() {
        let mut app = App::new();
        app.add_systems(Update, sync_mounted_hierarchy);

        let parent_guid = Uuid::new_v4();
        let child_guid = Uuid::new_v4();
        let parent = app.world_mut().spawn(EntityGuid(parent_guid)).id();
        let child = app
            .world_mut()
            .spawn((
                EntityGuid(child_guid),
                ParentGuid(parent_guid),
                Hardpoint {
                    hardpoint_id: "hp".to_string(),
                    offset_m: Vec3::new(1.0, 2.0, 0.0),
                    local_rotation: Quat::IDENTITY,
                },
            ))
            .id();

        app.update();
        let initial = *app
            .world()
            .entity(child)
            .get::<Transform>()
            .expect("initial transform");
        assert_eq!(initial.translation, Vec3::new(1.0, 2.0, 0.0));
        assert!(app.world().entity(child).contains::<ChildOf>());

        {
            let mut entity_mut = app.world_mut().entity_mut(child);
            let mut hardpoint = entity_mut.get_mut::<Hardpoint>().expect("hardpoint");
            hardpoint.offset_m = Vec3::new(9.0, -3.0, 0.0);
        }

        app.update();
        let updated = *app
            .world()
            .entity(child)
            .get::<Transform>()
            .expect("updated transform");
        assert_eq!(updated.translation, Vec3::new(9.0, -3.0, 0.0));
        assert!(app.world().entities().contains(parent));
    }

    #[test]
    fn inserts_missing_spatial_components_on_parent_before_linking_child() {
        let mut app = App::new();
        app.add_systems(Update, sync_mounted_hierarchy);

        let parent_guid = Uuid::new_v4();
        let child_guid = Uuid::new_v4();
        let parent = app.world_mut().spawn(EntityGuid(parent_guid)).id();
        let child = app
            .world_mut()
            .spawn((EntityGuid(child_guid), ParentGuid(parent_guid)))
            .id();

        app.update();

        let parent_ref = app.world().entity(parent);
        assert!(parent_ref.contains::<Transform>());
        assert!(parent_ref.contains::<GlobalTransform>());
        assert!(parent_ref.contains::<Visibility>());
        assert!(app.world().entity(child).contains::<ChildOf>());
    }
}
