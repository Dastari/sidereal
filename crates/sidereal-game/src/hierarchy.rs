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

use bevy::prelude::*;
use std::collections::HashMap;
use uuid::Uuid;

use crate::{EntityGuid, Hardpoint, MountedOn, ParentGuid};

/// Establishes Bevy parent-child hierarchy from `ParentGuid` (preferred)
/// and `MountedOn` (fallback for legacy/module-only links).
///
/// For each entity without a Bevy parent (no `ChildOf`), looks up a parent
/// by stable GUID and calls `add_child`.
///
/// When only legacy `MountedOn` is available and the child is attached
/// directly under ship root, a hardpoint offset fallback is applied.
///
/// Entities whose parents have not yet spawned are silently skipped and
/// retried on subsequent frames (the `Without<ChildOf>` filter re-includes
/// them automatically).
#[allow(clippy::type_complexity)]
pub fn sync_mounted_hierarchy(
    mut commands: Commands<'_, '_>,
    unmounted: Query<
        '_,
        '_,
        (
            Entity,
            Option<&'_ ParentGuid>,
            Option<&'_ MountedOn>,
            Option<&'_ Hardpoint>,
        ),
        (Without<ChildOf>, Or<(With<ParentGuid>, With<MountedOn>)>),
    >,
    guid_entities: Query<'_, '_, (Entity, &'_ EntityGuid)>,
    hardpoints: Query<'_, '_, (&'_ EntityGuid, &'_ Hardpoint, Option<&'_ ParentGuid>)>,
) {
    if unmounted.is_empty() {
        return;
    }

    let entity_by_guid: HashMap<Uuid, Entity> = guid_entities
        .iter()
        .map(|(entity, guid)| (guid.0, entity))
        .collect();

    let mut hardpoint_transforms: HashMap<(Uuid, &str), (Vec3, Quat)> = HashMap::new();
    for (_hardpoint_guid, hp, parent_guid) in &hardpoints {
        let Some(parent_guid) = parent_guid else {
            continue;
        };
        hardpoint_transforms.insert(
            (parent_guid.0, hp.hardpoint_id.as_str()),
            (hp.offset_m, hp.local_rotation),
        );
    }

    for (child_entity, parent_guid, mounted_on, hardpoint) in &unmounted {
        let parent_guid = parent_guid
            .map(|v| v.0)
            .or_else(|| mounted_on.map(|v| v.parent_entity_id));
        let Some(parent_guid) = parent_guid else {
            continue;
        };
        let Some(&parent_entity) = entity_by_guid.get(&parent_guid) else {
            continue;
        };

        commands.entity(parent_entity).add_child(child_entity);

        if let Some(hardpoint) = hardpoint {
            commands.entity(child_entity).insert(
                Transform::from_translation(hardpoint.offset_m)
                    .with_rotation(hardpoint.local_rotation),
            );
            continue;
        }

        // Legacy fallback: if entity is attached by MountedOn directly to root ship
        // and not to a dedicated hardpoint entity, apply hardpoint offset manually.
        if mounted_on.is_some()
            && mounted_on.is_some_and(|v| parent_guid == v.parent_entity_id)
            && let Some(mounted_on) = mounted_on
            && let Some(&(offset, rotation)) = hardpoint_transforms.get(&(
                mounted_on.parent_entity_id,
                mounted_on.hardpoint_id.as_str(),
            ))
        {
            commands
                .entity(child_entity)
                .insert(Transform::from_translation(offset).with_rotation(rotation));
        }
    }
}
