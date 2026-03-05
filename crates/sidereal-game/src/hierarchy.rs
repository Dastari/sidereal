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
    unmounted: Query<
        '_,
        '_,
        (Entity, &'_ ParentGuid, Option<&'_ Hardpoint>),
        (Without<ChildOf>, With<ParentGuid>),
    >,
    guid_entities: Query<'_, '_, (Entity, &'_ EntityGuid, Option<&'_ ParentGuid>)>,
) {
    if unmounted.is_empty() {
        return;
    }

    let mut entity_by_guid = HashMap::<Uuid, Entity>::new();
    let mut parent_by_guid = HashMap::<Uuid, Uuid>::new();
    for (entity, guid, maybe_parent) in guid_entities.iter() {
        entity_by_guid.insert(guid.0, entity);
        if let Some(parent) = maybe_parent {
            parent_by_guid.insert(guid.0, parent.0);
        }
    }

    for (child_entity, parent_guid, hardpoint) in &unmounted {
        let Ok((_, child_entity_guid, _)) = guid_entities.get(child_entity) else {
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
    use super::would_create_parent_cycle;
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
}
