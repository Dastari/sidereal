use avian2d::prelude::Position;
use bevy::prelude::*;
use lightyear::prelude::client::Connected;
use lightyear::prelude::server::{ClientOf, RawServer};
use lightyear::prelude::{NetworkTarget, RemoteId, Server, ServerMultiMessageSender};
use sidereal_game::{
    ControlledEntityGuid, DisplayName, EntityGuid, EntityLabels, HealthPool, OwnerId, PlayerTag,
};
use sidereal_net::{
    ManifestChannel, OwnedAssetEntry, PlayerEntityId, ServerOwnerAssetManifestDeltaMessage,
    ServerOwnerAssetManifestSnapshotMessage,
};
use std::collections::{HashMap, HashSet};

use crate::replication::SimulatedControlledEntity;
use crate::replication::auth::AuthenticatedClientBindings;

const SNAPSHOT_RESYNC_INTERVAL_S: f64 = 1.0;

#[derive(Debug, Default)]
struct PlayerManifestState {
    sequence: u64,
    assets_by_entity_id: HashMap<String, OwnedAssetEntry>,
}

#[derive(Debug, Resource, Default)]
pub struct OwnerManifestStreamState {
    by_player_entity_id: HashMap<String, PlayerManifestState>,
    last_snapshot_player_by_client: HashMap<Entity, String>,
    last_snapshot_at_s_by_client: HashMap<Entity, f64>,
    tick: u64,
}

pub fn init_resources(app: &mut App) {
    app.insert_resource(OwnerManifestStreamState::default());
}

fn kind_from_labels(labels: Option<&EntityLabels>) -> String {
    if let Some(labels) = labels {
        if labels.0.iter().any(|label| label == "Ship") {
            return "ship".to_string();
        }
        if let Some(first) = labels.0.first() {
            return first.to_ascii_lowercase();
        }
    }
    "entity".to_string()
}

fn compute_manifest_diff(
    previous_assets: &HashMap<String, OwnedAssetEntry>,
    current_assets: &HashMap<String, OwnedAssetEntry>,
) -> (Vec<OwnedAssetEntry>, Vec<String>) {
    let mut upserts = current_assets
        .iter()
        .filter_map(
            |(entity_id, current)| match previous_assets.get(entity_id) {
                Some(previous) if previous == current => None,
                _ => Some(current.clone()),
            },
        )
        .collect::<Vec<_>>();
    upserts.sort_by(|a, b| a.entity_id.cmp(&b.entity_id));

    let mut removals = previous_assets
        .keys()
        .filter(|entity_id| !current_assets.contains_key(*entity_id))
        .cloned()
        .collect::<Vec<_>>();
    removals.sort();

    (upserts, removals)
}

#[allow(clippy::type_complexity)]
#[allow(clippy::too_many_arguments)]
pub fn stream_owner_asset_manifest_messages(
    server_query: Query<'_, '_, &'_ Server, With<RawServer>>,
    mut sender: ServerMultiMessageSender<'_, '_, With<Connected>>,
    time: Res<'_, Time<Real>>,
    bindings: Res<'_, AuthenticatedClientBindings>,
    client_remotes: Query<'_, '_, &'_ RemoteId, With<ClientOf>>,
    mut stream_state: ResMut<'_, OwnerManifestStreamState>,
    player_controlled: Query<
        '_,
        '_,
        (&'_ EntityGuid, Option<&'_ ControlledEntityGuid>),
        With<PlayerTag>,
    >,
    entities: Query<
        '_,
        '_,
        (
            &'_ EntityGuid,
            &'_ OwnerId,
            Option<&'_ EntityLabels>,
            Option<&'_ DisplayName>,
            Option<&'_ Position>,
            Option<&'_ HealthPool>,
            Has<SimulatedControlledEntity>,
        ),
    >,
) {
    let Ok(server) = server_query.single() else {
        return;
    };
    let now_s = time.elapsed_secs_f64();
    stream_state.tick = stream_state.tick.saturating_add(1);
    let generated_at_tick = stream_state.tick;

    let mut controlled_guid_by_player = HashMap::<String, String>::new();
    for (player_guid, controlled_guid) in &player_controlled {
        let player_entity_id = player_guid.0.to_string();
        let resolved_controlled = controlled_guid
            .and_then(|controlled| controlled.0.clone())
            .unwrap_or_else(|| player_entity_id.clone());
        controlled_guid_by_player.insert(player_entity_id, resolved_controlled);
    }

    let mut assets_by_owner = HashMap::<String, HashMap<String, OwnedAssetEntry>>::new();
    for (guid, owner_id, labels, display_name, position, health, is_controllable) in &entities {
        if !is_controllable {
            continue;
        }
        let Some(owner_player_id) =
            PlayerEntityId::parse(owner_id.0.as_str()).map(|id| id.canonical_wire_id())
        else {
            continue;
        };
        let entity_id = guid.0.to_string();
        let health_ratio = health.and_then(|pool| {
            (pool.maximum > 0.0).then_some((pool.current / pool.maximum).clamp(0.0, 1.0))
        });
        let controlled_by_owner = controlled_guid_by_player
            .get(owner_player_id.as_str())
            .is_some_and(|controlled| controlled == &entity_id);
        let entry = OwnedAssetEntry {
            entity_id: entity_id.clone(),
            display_name: display_name
                .map(|name| name.0.clone())
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| entity_id.clone()),
            kind: kind_from_labels(labels),
            status: "active".to_string(),
            controlled_by_owner,
            last_known_position_xy: position.map(|p| [p.0.x, p.0.y]),
            health_ratio,
            fuel_ratio: None,
            updated_at_tick: generated_at_tick,
        };
        assets_by_owner
            .entry(owner_player_id)
            .or_default()
            .insert(entity_id, entry);
    }

    let mut active_players = HashSet::<String>::new();
    let mut active_clients = HashSet::<Entity>::new();
    for (client_entity, bound_player_id) in &bindings.by_client_entity {
        active_clients.insert(*client_entity);
        let Some(player_entity_id) =
            PlayerEntityId::parse(bound_player_id.as_str()).map(|id| id.canonical_wire_id())
        else {
            continue;
        };
        let Ok(remote_id) = client_remotes.get(*client_entity) else {
            continue;
        };
        active_players.insert(player_entity_id.clone());

        let current_assets = assets_by_owner
            .remove(player_entity_id.as_str())
            .unwrap_or_default();
        let target = NetworkTarget::Single(remote_id.0);
        let snapshot_player_changed = stream_state
            .last_snapshot_player_by_client
            .get(client_entity)
            .is_none_or(|last_player| last_player != &player_entity_id);
        let snapshot_stale = stream_state
            .last_snapshot_at_s_by_client
            .get(client_entity)
            .is_none_or(|last_at| now_s - *last_at >= SNAPSHOT_RESYNC_INTERVAL_S);
        let client_needs_snapshot = snapshot_player_changed || snapshot_stale;
        let mut emitted_snapshot = false;
        {
            let state = stream_state
                .by_player_entity_id
                .entry(player_entity_id.clone())
                .or_default();

            if state.sequence == 0 {
                state.sequence = 1;
                state.assets_by_entity_id = current_assets.clone();
            }

            let (upserts, removals) =
                compute_manifest_diff(&state.assets_by_entity_id, &current_assets);
            let mut previous_sequence = state.sequence;
            if !upserts.is_empty() || !removals.is_empty() {
                previous_sequence = state.sequence;
                state.sequence = state.sequence.saturating_add(1);
                state.assets_by_entity_id = current_assets;
            }

            if client_needs_snapshot {
                let mut assets = state
                    .assets_by_entity_id
                    .values()
                    .cloned()
                    .collect::<Vec<_>>();
                assets.sort_by(|a, b| a.entity_id.cmp(&b.entity_id));
                let message = ServerOwnerAssetManifestSnapshotMessage {
                    player_entity_id: player_entity_id.clone(),
                    sequence: state.sequence,
                    assets,
                    generated_at_tick,
                };
                let _ = sender.send::<ServerOwnerAssetManifestSnapshotMessage, ManifestChannel>(
                    &message, server, &target,
                );
                emitted_snapshot = true;
            } else if !upserts.is_empty() || !removals.is_empty() {
                let message = ServerOwnerAssetManifestDeltaMessage {
                    player_entity_id: player_entity_id.clone(),
                    sequence: state.sequence,
                    base_sequence: previous_sequence,
                    upserts,
                    removals,
                    generated_at_tick,
                };
                let _ = sender.send::<ServerOwnerAssetManifestDeltaMessage, ManifestChannel>(
                    &message, server, &target,
                );
            }
        }
        if emitted_snapshot {
            stream_state
                .last_snapshot_player_by_client
                .insert(*client_entity, player_entity_id.clone());
            stream_state
                .last_snapshot_at_s_by_client
                .insert(*client_entity, now_s);
        }
    }

    stream_state
        .by_player_entity_id
        .retain(|player_entity_id, _| active_players.contains(player_entity_id));
    stream_state
        .last_snapshot_player_by_client
        .retain(|client_entity, _| active_clients.contains(client_entity));
    stream_state
        .last_snapshot_at_s_by_client
        .retain(|client_entity, _| active_clients.contains(client_entity));
}

#[cfg(test)]
mod tests {
    use super::compute_manifest_diff;
    use sidereal_net::OwnedAssetEntry;
    use std::collections::HashMap;

    fn entry(entity_id: &str, name: &str) -> OwnedAssetEntry {
        OwnedAssetEntry {
            entity_id: entity_id.to_string(),
            display_name: name.to_string(),
            kind: "ship".to_string(),
            status: "active".to_string(),
            controlled_by_owner: false,
            last_known_position_xy: None,
            health_ratio: None,
            fuel_ratio: None,
            updated_at_tick: 1,
        }
    }

    #[test]
    fn manifest_diff_reports_upserts_and_removals() {
        let mut previous = HashMap::new();
        previous.insert("a".to_string(), entry("a", "A"));
        previous.insert("b".to_string(), entry("b", "B"));

        let mut current = HashMap::new();
        current.insert("a".to_string(), entry("a", "A2"));
        current.insert("c".to_string(), entry("c", "C"));

        let (upserts, removals) = compute_manifest_diff(&previous, &current);
        assert_eq!(upserts.len(), 2);
        assert_eq!(upserts[0].entity_id, "a");
        assert_eq!(upserts[1].entity_id, "c");
        assert_eq!(removals, vec!["b".to_string()]);
    }
}
