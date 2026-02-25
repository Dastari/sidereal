use avian3d::prelude::Position;
use bevy::prelude::*;
use lightyear::prelude::server::ClientOf;
use lightyear::prelude::{Replicate, ReplicationState};
use sidereal_game::{
    EntityGuid, FactionId, FactionVisibility, MountedOn, OwnerId, PlayerTag, PublicVisibility,
    ScannerRangeM,
};
use std::collections::{HashMap, HashSet};

use crate::visibility::{
    ClientObserverAnchorPositionMap, ClientVisibilityRegistry, DEFAULT_VIEW_RANGE_M,
};

#[derive(Debug, Clone)]
struct PlayerVisibilityContext {
    player_entity_id: String,
    observer_anchor_position: Option<Vec3>,
    scanner_sources: Vec<(Vec3, f32)>,
    player_faction_id: Option<String>,
}

#[derive(Resource, Default)]
pub struct VisibilityScratch {
    live_clients: Vec<Entity>,
    live_client_set: HashSet<Entity>,
    registered_clients: Vec<(Entity, String)>,
    root_entity_by_guid: HashMap<uuid::Uuid, Entity>,
    root_position_by_entity: HashMap<Entity, Vec3>,
    root_public_by_entity: HashMap<Entity, bool>,
    root_owner_by_entity: HashMap<Entity, String>,
    root_faction_by_entity: HashMap<Entity, String>,
    scanner_sources_by_owner: HashMap<String, Vec<(Vec3, f32)>>,
    player_faction_by_owner: HashMap<String, String>,
    context_by_client: HashMap<Entity, PlayerVisibilityContext>,
}

impl VisibilityScratch {
    fn clear(&mut self) {
        self.live_clients.clear();
        self.live_client_set.clear();
        self.registered_clients.clear();
        self.root_entity_by_guid.clear();
        self.root_position_by_entity.clear();
        self.root_public_by_entity.clear();
        self.root_owner_by_entity.clear();
        self.root_faction_by_entity.clear();
        self.scanner_sources_by_owner.clear();
        self.player_faction_by_owner.clear();
        self.context_by_client.clear();
    }
}

#[allow(clippy::type_complexity)]
#[allow(clippy::too_many_arguments)]
pub fn update_network_visibility(
    clients: Query<'_, '_, Entity, With<ClientOf>>,
    visibility_registry: Res<'_, ClientVisibilityRegistry>,
    mut scratch: ResMut<'_, VisibilityScratch>,
    observer_anchor_positions: Res<'_, ClientObserverAnchorPositionMap>,
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
            Option<&'_ EntityGuid>,
            Option<&'_ PlayerTag>,
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
    scratch.clear();
    scratch.live_clients.extend(clients.iter());
    let live_clients_snapshot = scratch.live_clients.clone();
    scratch.live_client_set.extend(live_clients_snapshot);

    // Drop stale registry entries for clients that have disconnected but have not yet
    // been cleaned by auth cleanup pass in this frame.
    let registered_clients = visibility_registry
        .player_entity_id_by_client
        .iter()
        .filter_map(|(client, player_id)| {
            if scratch.live_client_set.contains(client) {
                Some((*client, player_id.clone()))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    scratch.registered_clients.extend(registered_clients);

    scratch
        .root_entity_by_guid
        .extend(guid_entities.iter().map(|(entity, guid)| (guid.0, entity)));

    for (entity, position, _, owner_id, scanner_range, public_visibility, faction_id) in
        &root_entities
    {
        scratch.root_position_by_entity.insert(entity, position.0);
        scratch
            .root_public_by_entity
            .insert(entity, public_visibility.is_some());
        if let Some(faction) = faction_id {
            scratch
                .root_faction_by_entity
                .insert(entity, faction.0.clone());
        }
        if let Some(owner) = owner_id {
            scratch.root_owner_by_entity.insert(entity, owner.0.clone());
            let range = scanner_range
                .map(|r| r.0.max(0.0))
                .unwrap_or(DEFAULT_VIEW_RANGE_M);
            scratch
                .scanner_sources_by_owner
                .entry(owner.0.clone())
                .or_default()
                .push((position.0, range));
            if let Some(faction) = faction_id {
                scratch
                    .player_faction_by_owner
                    .entry(owner.0.clone())
                    .or_insert_with(|| faction.0.clone());
            }
        }
    }

    let registered_clients = scratch.registered_clients.clone();
    for (client_entity, player_entity_id) in &registered_clients {
        let scanner_sources = scratch
            .scanner_sources_by_owner
            .get(player_entity_id.as_str())
            .cloned()
            .unwrap_or_default();
        let observer_anchor_position = observer_anchor_positions
            .get_position(player_entity_id.as_str())
            .or_else(|| scanner_sources.first().map(|(pos, _)| *pos));
        let player_faction_id = scratch
            .player_faction_by_owner
            .get(player_entity_id.as_str())
            .cloned();
        scratch.context_by_client.insert(
            *client_entity,
            PlayerVisibilityContext {
                player_entity_id: player_entity_id.clone(),
                observer_anchor_position,
                scanner_sources,
                player_faction_id,
            },
        );
    }

    for (
        entity,
        mut replication_state,
        entity_guid,
        player_tag,
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
                mounted_on.and_then(|mounted| {
                    scratch
                        .root_entity_by_guid
                        .get(&mounted.parent_entity_id)
                        .copied()
                })
            })
            .unwrap_or(entity);

        let entity_position = scratch
            .root_position_by_entity
            .get(&root_entity)
            .copied()
            .or_else(|| own_position.map(|position| position.0))
            .or_else(|| {
                child_of
                    .and_then(|parent| position_by_entity.get(parent.parent()).ok())
                    .map(|position| position.0)
            });
        let is_public = public_visibility.is_some()
            || scratch
                .root_public_by_entity
                .get(&root_entity)
                .copied()
                .unwrap_or(false);
        let owner_player_id = owner_id.map(|owner| owner.0.as_str()).or_else(|| {
            scratch
                .root_owner_by_entity
                .get(&root_entity)
                .map(String::as_str)
        });
        // Ensure players always receive replication for their own observer/player entity
        // even in valid no-ship states.
        let owner_player_id_owned = if owner_player_id.is_none() && player_tag.is_some() {
            entity_guid.map(|guid| format!("player:{}", guid.0))
        } else {
            None
        };
        let owner_player_id = owner_player_id.or(owner_player_id_owned.as_deref());
        let entity_faction_id = faction_id.map(|faction| faction.0.as_str()).or_else(|| {
            scratch
                .root_faction_by_entity
                .get(&root_entity)
                .map(String::as_str)
        });
        let is_faction_visible = faction_visibility.is_some();

        for client_entity in &scratch.live_clients {
            let Some(player_entity_id) = visibility_registry
                .player_entity_id_by_client
                .get(client_entity)
            else {
                // Ignore unauthenticated/unregistered clients entirely.
                // They should not receive world replication, and sending repeated
                // lose-visibility updates here causes noisy "despawn unknown entity" logs.
                continue;
            };
            let Some(visibility_context) = scratch.context_by_client.get(client_entity) else {
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

    let authorization = authorize_visibility(
        player_entity_id,
        owner_player_id,
        is_public_visibility,
        is_faction_visibility,
        entity_faction_id,
        entity_position,
        visibility_context,
    );
    let Some(authorization) = authorization else {
        return false;
    };

    // Owner visibility is an authorization exception and bypasses delivery narrowing.
    if matches!(authorization, VisibilityAuthorization::Owner) {
        return true;
    }

    passes_delivery_scope(entity_position, visibility_context)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VisibilityAuthorization {
    Owner,
    Public,
    Faction,
    Scanner,
}

fn authorize_visibility(
    player_entity_id: &str,
    owner_player_id: Option<&str>,
    is_public_visibility: bool,
    is_faction_visibility: bool,
    entity_faction_id: Option<&str>,
    entity_position: Option<Vec3>,
    visibility_context: &PlayerVisibilityContext,
) -> Option<VisibilityAuthorization> {
    // Ownership/public/faction are policy exceptions and must be evaluated
    // before any spatial delivery narrowing.
    if owner_player_id.is_some_and(|owner| owner == player_entity_id) {
        return Some(VisibilityAuthorization::Owner);
    }
    if is_faction_visibility
        && visibility_context
            .player_faction_id
            .as_deref()
            .zip(entity_faction_id)
            .is_some_and(|(player_faction, entity_faction)| player_faction == entity_faction)
    {
        return Some(VisibilityAuthorization::Faction);
    }
    if is_public_visibility {
        return Some(VisibilityAuthorization::Public);
    }
    let target_position = entity_position?;
    visibility_context
        .scanner_sources
        .iter()
        .find(|(scanner_pos, scanner_range_m)| {
            (target_position - *scanner_pos).length() <= *scanner_range_m
        })
        .map(|_| VisibilityAuthorization::Scanner)
}

fn passes_delivery_scope(
    entity_position: Option<Vec3>,
    visibility_context: &PlayerVisibilityContext,
) -> bool {
    let (Some(observer_anchor_position), Some(target_position)) =
        (visibility_context.observer_anchor_position, entity_position)
    else {
        return false;
    };
    (target_position - observer_anchor_position).length() <= DEFAULT_VIEW_RANGE_M
}

#[cfg(test)]
mod tests {
    use super::*;

    fn visibility_context(
        player_entity_id: &str,
        observer_anchor_position: Option<Vec3>,
        player_faction_id: Option<&str>,
        scanner_sources: Vec<(Vec3, f32)>,
    ) -> PlayerVisibilityContext {
        PlayerVisibilityContext {
            player_entity_id: player_entity_id.to_string(),
            observer_anchor_position,
            scanner_sources,
            player_faction_id: player_faction_id.map(ToString::to_string),
        }
    }

    #[test]
    fn owner_authorization_bypasses_delivery_scope() {
        let ctx = visibility_context("player-a", None, None, vec![]);
        assert_eq!(
            authorize_visibility("player-a", Some("player-a"), false, false, None, None, &ctx),
            Some(VisibilityAuthorization::Owner)
        );
        assert!(is_entity_visible_to_player(
            "player-a",
            Some("player-a"),
            false,
            false,
            None,
            None,
            &ctx
        ));
    }

    #[test]
    fn public_authorization_is_independent_of_delivery_scope() {
        let ctx = visibility_context("player-a", None, None, vec![]);
        assert_eq!(
            authorize_visibility("player-a", None, true, false, None, None, &ctx),
            Some(VisibilityAuthorization::Public)
        );
        assert!(!is_entity_visible_to_player(
            "player-a",
            None,
            true,
            false,
            None,
            Some(Vec3::new(10.0, 0.0, 0.0)),
            &ctx
        ));
    }

    #[test]
    fn faction_authorization_is_independent_of_delivery_scope() {
        let ctx = visibility_context("player-a", None, Some("faction-1"), vec![]);
        assert_eq!(
            authorize_visibility("player-a", None, false, true, Some("faction-1"), None, &ctx),
            Some(VisibilityAuthorization::Faction)
        );
        assert!(!is_entity_visible_to_player(
            "player-a",
            None,
            false,
            true,
            Some("faction-1"),
            Some(Vec3::ZERO),
            &ctx
        ));
    }

    #[test]
    fn scanner_authorization_requires_scanner_coverage() {
        let ctx = visibility_context(
            "player-a",
            Some(Vec3::ZERO),
            None,
            vec![(Vec3::new(1000.0, 0.0, 0.0), 50.0)],
        );
        assert_eq!(
            authorize_visibility(
                "player-a",
                None,
                false,
                false,
                None,
                Some(Vec3::new(0.0, 0.0, 0.0)),
                &ctx
            ),
            None
        );
    }

    #[test]
    fn scanner_authorization_still_requires_delivery_scope() {
        let ctx = visibility_context(
            "player-a",
            Some(Vec3::ZERO),
            None,
            vec![(Vec3::new(1000.0, 0.0, 0.0), 200.0)],
        );
        let target_position = Vec3::new(1050.0, 0.0, 0.0);
        assert_eq!(
            authorize_visibility(
                "player-a",
                None,
                false,
                false,
                None,
                Some(target_position),
                &ctx
            ),
            Some(VisibilityAuthorization::Scanner)
        );
        assert!(!is_entity_visible_to_player(
            "player-a",
            None,
            false,
            false,
            None,
            Some(target_position),
            &ctx
        ));
    }
}
