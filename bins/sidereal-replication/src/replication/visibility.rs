use avian3d::prelude::Position;
use bevy::prelude::*;
use lightyear::prelude::server::ClientOf;
use lightyear::prelude::{Replicate, ReplicationState};
use sidereal_game::{
    EntityGuid, FactionId, FactionVisibility, MountedOn, OwnerId, PublicVisibility, ScannerRangeM,
};
use std::collections::{HashMap, HashSet};

use crate::visibility::{
    ClientControlledEntityPositionMap, ClientVisibilityRegistry, DEFAULT_VIEW_RANGE_M,
};

#[derive(Debug, Clone)]
struct PlayerVisibilityContext {
    player_entity_id: String,
    camera_position: Option<Vec3>,
    scanner_sources: Vec<(Vec3, f32)>,
    player_faction_id: Option<String>,
}

#[allow(clippy::type_complexity)]
#[allow(clippy::too_many_arguments)]
pub fn update_network_visibility(
    clients: Query<'_, '_, Entity, With<ClientOf>>,
    visibility_registry: Res<'_, ClientVisibilityRegistry>,
    controlled_positions: Res<'_, ClientControlledEntityPositionMap>,
    root_entities: Query<
        '_,
        '_,
        (
            Entity,
            &'_ Position,
            Option<&'_ EntityGuid>,
            Option<&'_ OwnerId>,
            Option<&'_ ScannerRangeM>,
            Option<&'_ PublicVisibility>,
            Option<&'_ FactionId>,
        ),
        (With<Replicate>, Without<MountedOn>, Without<ChildOf>),
    >,
    position_by_entity: Query<'_, '_, &'_ Position>,
    guid_entities: Query<
        '_,
        '_,
        (Entity, &'_ EntityGuid),
        (With<Replicate>, Without<MountedOn>, Without<ChildOf>),
    >,
    mut replicated_entities: Query<
        '_,
        '_,
        (
            Entity,
            &'_ mut ReplicationState,
            Option<&'_ OwnerId>,
            Option<&'_ PublicVisibility>,
            Option<&'_ FactionVisibility>,
            Option<&'_ FactionId>,
            Option<&'_ Position>,
            Option<&'_ ChildOf>,
            Option<&'_ MountedOn>,
        ),
        With<Replicate>,
    >,
) {
    let live_clients = clients.iter().collect::<Vec<_>>();
    let live_client_set = live_clients.iter().copied().collect::<HashSet<_>>();

    // Drop stale registry entries for clients that have disconnected but have not yet
    // been cleaned by auth cleanup pass in this frame.
    let registered_clients = visibility_registry
        .player_entity_id_by_client
        .iter()
        .filter_map(|(client, player_id)| {
            if live_client_set.contains(client) {
                Some((*client, player_id.clone()))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    let root_entity_by_guid = guid_entities
        .iter()
        .map(|(entity, guid)| (guid.0, entity))
        .collect::<HashMap<_, _>>();
    let root_position_by_entity = root_entities
        .iter()
        .map(|(entity, position, _, _, _, _, _)| (entity, position.0))
        .collect::<HashMap<_, _>>();
    let root_public_by_entity = root_entities
        .iter()
        .map(|(entity, _, _, _, _, public_visibility, _)| (entity, public_visibility.is_some()))
        .collect::<HashMap<_, _>>();
    let root_owner_by_entity = root_entities
        .iter()
        .filter_map(|(entity, _, _, owner_id, _, _, _)| {
            owner_id.map(|owner| (entity, owner.0.clone()))
        })
        .collect::<HashMap<_, _>>();
    let root_faction_by_entity = root_entities
        .iter()
        .filter_map(|(entity, _, _, _, _, _, faction_id)| {
            faction_id.map(|faction| (entity, faction.0.clone()))
        })
        .collect::<HashMap<_, _>>();
    let context_by_client = registered_clients
        .iter()
        .map(|(client_entity, player_entity_id)| {
            (
                *client_entity,
                build_player_visibility_context(
                    player_entity_id,
                    controlled_positions.as_ref(),
                    &root_entities,
                ),
            )
        })
        .collect::<HashMap<_, _>>();

    for (
        entity,
        mut replication_state,
        owner_id,
        public_visibility,
        faction_visibility,
        faction_id,
        own_position,
        child_of,
        mounted_on,
    ) in &mut replicated_entities
    {
        let root_entity = child_of
            .map(|p| p.parent())
            .or_else(|| {
                mounted_on
                    .and_then(|mounted| root_entity_by_guid.get(&mounted.parent_entity_id).copied())
            })
            .unwrap_or(entity);

        let entity_position = root_position_by_entity
            .get(&root_entity)
            .copied()
            .or_else(|| own_position.map(|position| position.0))
            .or_else(|| {
                child_of
                    .and_then(|parent| position_by_entity.get(parent.parent()).ok())
                    .map(|position| position.0)
            });
        let is_public = public_visibility.is_some()
            || root_public_by_entity
                .get(&root_entity)
                .copied()
                .unwrap_or(false);
        let owner_player_id = owner_id
            .map(|owner| owner.0.as_str())
            .or_else(|| root_owner_by_entity.get(&root_entity).map(String::as_str));
        let entity_faction_id = faction_id
            .map(|faction| faction.0.as_str())
            .or_else(|| root_faction_by_entity.get(&root_entity).map(String::as_str));
        let is_faction_visible = faction_visibility.is_some();

        for client_entity in &live_clients {
            let Some(player_entity_id) = visibility_registry
                .player_entity_id_by_client
                .get(client_entity)
            else {
                // Ignore unauthenticated/unregistered clients entirely.
                // They should not receive world replication, and sending repeated
                // lose-visibility updates here causes noisy "despawn unknown entity" logs.
                continue;
            };
            let Some(visibility_context) = context_by_client.get(client_entity) else {
                continue;
            };
            let should_be_visible = is_entity_visible_to_player(
                player_entity_id,
                owner_player_id,
                is_public,
                is_faction_visible,
                entity_faction_id,
                entity_position,
                visibility_context,
            );
            if should_be_visible {
                replication_state.gain_visibility(*client_entity);
            } else if replication_state.is_visible(*client_entity) {
                replication_state.lose_visibility(*client_entity);
            }
        }
    }
}

fn build_player_visibility_context(
    player_entity_id: &str,
    controlled_positions: &ClientControlledEntityPositionMap,
    root_entities: &Query<
        '_,
        '_,
        (
            Entity,
            &'_ Position,
            Option<&'_ EntityGuid>,
            Option<&'_ OwnerId>,
            Option<&'_ ScannerRangeM>,
            Option<&'_ PublicVisibility>,
            Option<&'_ FactionId>,
        ),
        (With<Replicate>, Without<MountedOn>, Without<ChildOf>),
    >,
) -> PlayerVisibilityContext {
    let scanner_sources = root_entities
        .iter()
        .filter_map(|(_, position, _, owner_id, scanner_range, _, _)| {
            if owner_id.is_some_and(|owner| owner.0 == player_entity_id) {
                let range = scanner_range
                    .map(|r| r.0.max(0.0))
                    .unwrap_or(DEFAULT_VIEW_RANGE_M);
                Some((position.0, range))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    // Bootstrap-safe camera position: if the player camera hasn't replicated yet,
    // fallback to first owned scanner source.
    let camera_position = controlled_positions
        .get_position(player_entity_id)
        .or_else(|| scanner_sources.first().map(|(pos, _)| *pos));
    let player_faction_id = root_entities
        .iter()
        .find_map(|(_, _, _, owner_id, _, _, faction_id)| {
            if owner_id.is_some_and(|owner| owner.0 == player_entity_id) {
                faction_id.map(|faction| faction.0.clone())
            } else {
                None
            }
        });

    PlayerVisibilityContext {
        player_entity_id: player_entity_id.to_string(),
        camera_position,
        scanner_sources,
        player_faction_id,
    }
}

fn is_entity_visible_to_player(
    player_entity_id: &str,
    owner_player_id: Option<&str>,
    is_public_visibility: bool,
    is_faction_visibility: bool,
    entity_faction_id: Option<&str>,
    entity_position: Option<Vec3>,
    visibility_context: &PlayerVisibilityContext,
) -> bool {
    // Safety check for mismatched context call-site.
    if visibility_context.player_entity_id != player_entity_id {
        return false;
    }

    // Owned entities/modules are always visible to their owner.
    if owner_player_id.is_some_and(|owner| owner == player_entity_id) {
        return true;
    }

    let (Some(camera_position), Some(target_position)) =
        (visibility_context.camera_position, entity_position)
    else {
        return false;
    };

    // Pass A: camera bubble coarse cull.
    let in_camera_bubble = (target_position - camera_position).length() <= DEFAULT_VIEW_RANGE_M;
    if !in_camera_bubble {
        return false;
    }

    // Public visibility: camera line-of-sight only (no scanner gate).
    if is_public_visibility {
        return true;
    }

    if is_faction_visibility
        && visibility_context
            .player_faction_id
            .as_deref()
            .zip(entity_faction_id)
            .is_some_and(|(player_faction, entity_faction)| player_faction == entity_faction)
    {
        return true;
    }

    // Pass B: scanner coverage union from all owned scanner-bearing roots.
    visibility_context
        .scanner_sources
        .iter()
        .any(|(scanner_pos, scanner_range_m)| {
            (target_position - *scanner_pos).length() <= *scanner_range_m
        })
}
