use avian2d::prelude::{LinearVelocity, Position, Rotation};
use bevy::prelude::*;
use lightyear::prelude::client::Connected;
use lightyear::prelude::server::{ClientOf, RawServer};
use lightyear::prelude::{
    NetworkTarget, RemoteId, ReplicationState, Server, ServerMultiMessageSender,
};
use sidereal_game::{
    EntityGuid, EntityLabels, FactionId, PlayerTag, VisibilityGridCell, VisibilitySpatialGrid,
};
use sidereal_net::{
    ClientTacticalResnapshotRequestMessage, GridCell, PlayerEntityId,
    ServerTacticalContactsDeltaMessage, ServerTacticalContactsSnapshotMessage,
    ServerTacticalFogDeltaMessage, ServerTacticalFogSnapshotMessage, TacticalContact,
    TacticalDeltaChannel, TacticalSnapshotChannel,
};
use std::collections::{HashMap, HashSet};

use crate::replication::PlayerRuntimeEntityMap;
use crate::replication::auth::AuthenticatedClientBindings;
use lightyear::prelude::MessageReceiver;

const UPDATE_INTERVAL_S: f64 = 0.5;
const SNAPSHOT_RESYNC_INTERVAL_S: f64 = 2.0;

#[derive(Debug, Default)]
struct PlayerTacticalStreamState {
    fog_sequence: u64,
    contacts_sequence: u64,
    last_sent_at_s: f64,
    last_snapshot_at_s: f64,
    initialized: bool,
    explored_cells: HashSet<GridCell>,
    live_cells: HashSet<GridCell>,
    contacts_by_entity_id: HashMap<String, TacticalContact>,
}

#[derive(Debug, Resource, Default)]
pub struct TacticalStreamState {
    by_player_entity_id: HashMap<String, PlayerTacticalStreamState>,
    forced_snapshot_by_player: HashSet<String>,
    tick: u64,
}

pub fn init_resources(app: &mut App) {
    app.insert_resource(TacticalStreamState::default());
}

pub fn receive_tactical_resnapshot_requests(
    mut stream_state: ResMut<'_, TacticalStreamState>,
    bindings: Res<'_, AuthenticatedClientBindings>,
    mut receivers: Query<
        '_,
        '_,
        (Entity, &'_ mut MessageReceiver<ClientTacticalResnapshotRequestMessage>),
        With<ClientOf>,
    >,
) {
    for (client_entity, mut receiver) in &mut receivers {
        let Some(bound_player_id) = bindings.by_client_entity.get(&client_entity) else {
            continue;
        };
        let Some(bound_player_id) =
            PlayerEntityId::parse(bound_player_id.as_str()).map(|id| id.canonical_wire_id())
        else {
            continue;
        };
        for message in receiver.receive() {
            let Some(message_player_id) = PlayerEntityId::parse(message.player_entity_id.as_str())
                .map(|id| id.canonical_wire_id())
            else {
                continue;
            };
            if message_player_id != bound_player_id {
                continue;
            }
            if message.request_fog_snapshot || message.request_contacts_snapshot {
                stream_state.forced_snapshot_by_player.insert(message_player_id);
            }
        }
    }
}

fn grid_cell_from_component(cell: &VisibilityGridCell) -> Option<GridCell> {
    let x = i32::try_from(cell.x).ok()?;
    let y = i32::try_from(cell.y).ok()?;
    Some(GridCell { x, y })
}

fn contact_kind_from_labels(labels: Option<&EntityLabels>) -> String {
    if let Some(labels) = labels {
        if labels.0.iter().any(|label| label == "Ship") {
            return "ship".to_string();
        }
        if labels.0.iter().any(|label| label == "Player") {
            return "player".to_string();
        }
        if let Some(first) = labels.0.first() {
            return first.to_ascii_lowercase();
        }
    }
    "entity".to_string()
}

fn contact_changed_for_delta(previous: &TacticalContact, current: &TacticalContact) -> bool {
    previous.entity_id != current.entity_id
        || previous.kind != current.kind
        || previous.faction_id != current.faction_id
        || previous.position_xy != current.position_xy
        || previous.heading_rad != current.heading_rad
        || previous.velocity_xy != current.velocity_xy
        || previous.is_live_now != current.is_live_now
        || previous.classification != current.classification
        || previous.contact_quality != current.contact_quality
}

fn compute_fog_delta(
    previous_explored_cells: &HashSet<GridCell>,
    previous_live_cells: &HashSet<GridCell>,
    current_explored_cells: &HashSet<GridCell>,
    current_live_cells: &HashSet<GridCell>,
) -> (Vec<GridCell>, Vec<GridCell>, Vec<GridCell>) {
    let mut explored_cells_added = current_explored_cells
        .difference(previous_explored_cells)
        .copied()
        .collect::<Vec<_>>();
    explored_cells_added.sort_by_key(|cell| (cell.x, cell.y));

    let mut live_cells_added = current_live_cells
        .difference(previous_live_cells)
        .copied()
        .collect::<Vec<_>>();
    live_cells_added.sort_by_key(|cell| (cell.x, cell.y));

    let mut live_cells_removed = previous_live_cells
        .difference(current_live_cells)
        .copied()
        .collect::<Vec<_>>();
    live_cells_removed.sort_by_key(|cell| (cell.x, cell.y));

    (explored_cells_added, live_cells_added, live_cells_removed)
}

fn compute_contacts_delta(
    previous_contacts: &HashMap<String, TacticalContact>,
    current_contacts: &HashMap<String, TacticalContact>,
) -> (Vec<TacticalContact>, Vec<String>) {
    let mut upserts = current_contacts
        .iter()
        .filter_map(
            |(entity_id, current)| match previous_contacts.get(entity_id.as_str()) {
                Some(previous) if !contact_changed_for_delta(previous, current) => None,
                _ => Some(current.clone()),
            },
        )
        .collect::<Vec<_>>();
    upserts.sort_by(|a, b| a.entity_id.cmp(&b.entity_id));

    let mut removals = previous_contacts
        .keys()
        .filter(|entity_id| !current_contacts.contains_key(entity_id.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    removals.sort();

    (upserts, removals)
}

#[allow(clippy::type_complexity)]
pub fn stream_tactical_snapshot_messages(
    server_query: Query<'_, '_, &'_ Server, With<RawServer>>,
    mut sender: ServerMultiMessageSender<'_, '_, With<Connected>>,
    time: Res<'_, Time<Real>>,
    mut stream_state: ResMut<'_, TacticalStreamState>,
    bindings: Res<'_, AuthenticatedClientBindings>,
    client_remotes: Query<'_, '_, &'_ RemoteId, With<ClientOf>>,
    player_entities: Res<'_, PlayerRuntimeEntityMap>,
    player_visibility: Query<'_, '_, &'_ VisibilitySpatialGrid, With<PlayerTag>>,
    replicated_entities: Query<
        '_,
        '_,
        (
            Option<&'_ EntityGuid>,
            Option<&'_ EntityLabels>,
            Option<&'_ FactionId>,
            Option<&'_ Position>,
            Option<&'_ GlobalTransform>,
            Option<&'_ Rotation>,
            Option<&'_ LinearVelocity>,
            Option<&'_ PlayerTag>,
            &'_ ReplicationState,
        ),
    >,
) {
    let Ok(server) = server_query.single() else {
        return;
    };
    stream_state.tick = stream_state.tick.saturating_add(1);
    let generated_at_tick = stream_state.tick;
    let now_s = time.elapsed_secs_f64();

    for (client_entity, bound_player_id) in &bindings.by_client_entity {
        let Some(player_entity_id) =
            PlayerEntityId::parse(bound_player_id.as_str()).map(|id| id.canonical_wire_id())
        else {
            continue;
        };
        let Ok(remote_id) = client_remotes.get(*client_entity) else {
            continue;
        };
        let Some(&player_entity) = player_entities
            .by_player_entity_id
            .get(player_entity_id.as_str())
        else {
            continue;
        };
        let Ok(player_grid) = player_visibility.get(player_entity) else {
            continue;
        };

        let force_snapshot = stream_state
            .forced_snapshot_by_player
            .remove(player_entity_id.as_str());

        let state = stream_state
            .by_player_entity_id
            .entry(player_entity_id.clone())
            .or_default();
        if now_s - state.last_sent_at_s < UPDATE_INTERVAL_S {
            continue;
        }
        state.last_sent_at_s = now_s;

        let live_cells = player_grid
            .queried_cells
            .iter()
            .filter_map(grid_cell_from_component)
            .collect::<HashSet<_>>();

        // Iteration 1: explored cells follow current queried visibility footprint.
        // Server-side persistent explored-memory growth will replace this in the fog-of-war phase.
        let explored_cells = live_cells.clone();

        let mut contacts_by_entity_id = HashMap::<String, TacticalContact>::new();
        for (
            guid,
            labels,
            faction_id,
            position,
            global_transform,
            rotation,
            linear_velocity,
            player_tag,
            replication_state,
        ) in &replicated_entities
        {
            if !replication_state.is_visible(*client_entity) {
                continue;
            }
            if player_tag.is_some() {
                continue;
            }
            let Some(guid) = guid else {
                continue;
            };
            let world = global_transform
                .map(GlobalTransform::translation)
                .or_else(|| position.map(|p| p.0.extend(0.0)))
                .unwrap_or(Vec3::ZERO);
            let entity_id = guid.0.to_string();
            contacts_by_entity_id.insert(entity_id.clone(), TacticalContact {
                entity_id: guid.0.to_string(),
                kind: contact_kind_from_labels(labels),
                faction_id: faction_id.map(|id| id.0.clone()),
                position_xy: [world.x, world.y],
                heading_rad: rotation.map_or(0.0, |value| value.as_radians()),
                velocity_xy: linear_velocity.map(|v| [v.0.x, v.0.y]),
                is_live_now: true,
                last_seen_tick: generated_at_tick,
                classification: None,
                contact_quality: None,
            });
        }

        let target = NetworkTarget::Single(remote_id.0);
        let needs_snapshot = force_snapshot
            || !state.initialized
            || now_s - state.last_snapshot_at_s >= SNAPSHOT_RESYNC_INTERVAL_S;

        if needs_snapshot {
            state.fog_sequence = state.fog_sequence.saturating_add(1);
            let mut explored_cells_snapshot = explored_cells.iter().copied().collect::<Vec<_>>();
            explored_cells_snapshot.sort_by_key(|cell| (cell.x, cell.y));
            let mut live_cells_snapshot = live_cells.iter().copied().collect::<Vec<_>>();
            live_cells_snapshot.sort_by_key(|cell| (cell.x, cell.y));
            let fog_message = ServerTacticalFogSnapshotMessage {
                player_entity_id: player_entity_id.clone(),
                sequence: state.fog_sequence,
                cell_size_m: player_grid.cell_size_m,
                explored_cells: explored_cells_snapshot,
                live_cells: live_cells_snapshot,
                generated_at_tick,
            };
            let _ = sender.send::<ServerTacticalFogSnapshotMessage, TacticalSnapshotChannel>(
                &fog_message,
                server,
                &target,
            );

            state.contacts_sequence = state.contacts_sequence.saturating_add(1);
            let mut contacts_snapshot = contacts_by_entity_id.values().cloned().collect::<Vec<_>>();
            contacts_snapshot.sort_by(|a, b| a.entity_id.cmp(&b.entity_id));
            let contacts_message = ServerTacticalContactsSnapshotMessage {
                player_entity_id: player_entity_id.clone(),
                sequence: state.contacts_sequence,
                contacts: contacts_snapshot,
                generated_at_tick,
            };
            let _ = sender.send::<ServerTacticalContactsSnapshotMessage, TacticalSnapshotChannel>(
                &contacts_message,
                server,
                &target,
            );

            state.last_snapshot_at_s = now_s;
            state.initialized = true;
        } else {
            let (explored_cells_added, live_cells_added, live_cells_removed) = compute_fog_delta(
                &state.explored_cells,
                &state.live_cells,
                &explored_cells,
                &live_cells,
            );
            if !explored_cells_added.is_empty()
                || !live_cells_added.is_empty()
                || !live_cells_removed.is_empty()
            {
                let base_sequence = state.fog_sequence;
                state.fog_sequence = state.fog_sequence.saturating_add(1);
                let fog_delta = ServerTacticalFogDeltaMessage {
                    player_entity_id: player_entity_id.clone(),
                    sequence: state.fog_sequence,
                    base_sequence,
                    explored_cells_added,
                    live_cells_added,
                    live_cells_removed,
                    generated_at_tick,
                };
                let _ = sender.send::<ServerTacticalFogDeltaMessage, TacticalDeltaChannel>(
                    &fog_delta, server, &target,
                );
            }

            let (upserts, removals) =
                compute_contacts_delta(&state.contacts_by_entity_id, &contacts_by_entity_id);
            if !upserts.is_empty() || !removals.is_empty() {
                let base_sequence = state.contacts_sequence;
                state.contacts_sequence = state.contacts_sequence.saturating_add(1);
                let contacts_delta = ServerTacticalContactsDeltaMessage {
                    player_entity_id: player_entity_id.clone(),
                    sequence: state.contacts_sequence,
                    base_sequence,
                    upserts,
                    removals,
                    generated_at_tick,
                };
                let _ = sender.send::<ServerTacticalContactsDeltaMessage, TacticalDeltaChannel>(
                    &contacts_delta,
                    server,
                    &target,
                );
            }
        }

        state.explored_cells = explored_cells;
        state.live_cells = live_cells;
        state.contacts_by_entity_id = contacts_by_entity_id;
    }

    let active_players = bindings
        .by_client_entity
        .values()
        .filter_map(|player_id| PlayerEntityId::parse(player_id.as_str()))
        .map(|player_id| player_id.canonical_wire_id())
        .collect::<HashSet<_>>();
    stream_state
        .by_player_entity_id
        .retain(|player_entity_id, _| active_players.contains(player_entity_id));
}

#[cfg(test)]
mod tests {
    use super::{compute_contacts_delta, compute_fog_delta};
    use sidereal_net::{GridCell, TacticalContact};
    use std::collections::{HashMap, HashSet};

    fn contact(entity_id: &str, position_xy: [f32; 2], last_seen_tick: u64) -> TacticalContact {
        TacticalContact {
            entity_id: entity_id.to_string(),
            kind: "ship".to_string(),
            faction_id: None,
            position_xy,
            heading_rad: 0.0,
            velocity_xy: None,
            is_live_now: true,
            last_seen_tick,
            classification: None,
            contact_quality: None,
        }
    }

    #[test]
    fn fog_delta_reports_added_and_removed_cells() {
        let previous_explored = HashSet::from([GridCell { x: 1, y: 1 }]);
        let previous_live = HashSet::from([GridCell { x: 1, y: 1 }, GridCell { x: 2, y: 2 }]);
        let current_explored = HashSet::from([GridCell { x: 1, y: 1 }, GridCell { x: 3, y: 3 }]);
        let current_live = HashSet::from([GridCell { x: 3, y: 3 }, GridCell { x: 2, y: 2 }]);

        let (explored_added, live_added, live_removed) = compute_fog_delta(
            &previous_explored,
            &previous_live,
            &current_explored,
            &current_live,
        );

        assert_eq!(explored_added, vec![GridCell { x: 3, y: 3 }]);
        assert_eq!(live_added, vec![GridCell { x: 3, y: 3 }]);
        assert_eq!(live_removed, vec![GridCell { x: 1, y: 1 }]);
    }

    #[test]
    fn contacts_delta_ignores_last_seen_tick_churn() {
        let mut previous = HashMap::new();
        previous.insert("a".to_string(), contact("a", [1.0, 2.0], 10));

        let mut current = HashMap::new();
        current.insert("a".to_string(), contact("a", [1.0, 2.0], 11));

        let (upserts, removals) = compute_contacts_delta(&previous, &current);
        assert!(upserts.is_empty());
        assert!(removals.is_empty());
    }

    #[test]
    fn contacts_delta_reports_upserts_and_removals() {
        let mut previous = HashMap::new();
        previous.insert("a".to_string(), contact("a", [1.0, 2.0], 10));
        previous.insert("b".to_string(), contact("b", [2.0, 3.0], 10));

        let mut current = HashMap::new();
        current.insert("a".to_string(), contact("a", [9.0, 9.0], 11));
        current.insert("c".to_string(), contact("c", [4.0, 5.0], 11));

        let (upserts, removals) = compute_contacts_delta(&previous, &current);

        assert_eq!(upserts.len(), 2);
        assert_eq!(upserts[0].entity_id, "a");
        assert_eq!(upserts[1].entity_id, "c");
        assert_eq!(removals, vec!["b".to_string()]);
    }
}
