use bevy::log::warn;
use bevy::prelude::*;
use lightyear::prelude::MessageReceiver;
use lightyear::prelude::MessageSender;
use lightyear::prelude::client::{Client, Connected};
use sidereal_net::{
    ClientTacticalResnapshotRequestMessage, PlayerEntityId, ServerTacticalContactsDeltaMessage,
    ServerTacticalContactsSnapshotMessage, ServerTacticalFogDeltaMessage,
    ServerTacticalFogSnapshotMessage, TacticalSnapshotChannel,
};
use std::collections::HashSet;

use super::app_state::ClientSession;
use super::resources::{TacticalContactsCache, TacticalFogCache, TacticalResnapshotRequestState};

const MISMATCH_LOG_INTERVAL_S: f64 = 1.0;
const SNAPSHOT_TIMEOUT_S: f64 = 3.0;
const RESNAPSHOT_REQUEST_INTERVAL_S: f64 = 1.0;

fn local_player_canonical_id(session: &ClientSession) -> Option<String> {
    session
        .player_entity_id
        .as_deref()
        .and_then(PlayerEntityId::parse)
        .map(|id| id.canonical_wire_id())
}

fn sort_dedup_cells(cells: &mut Vec<sidereal_net::GridCell>) {
    cells.sort_by_key(|cell| (cell.x, cell.y));
    cells.dedup();
}

fn apply_tactical_fog_snapshot(
    cache: &mut TacticalFogCache,
    local_player_id: &str,
    message: &ServerTacticalFogSnapshotMessage,
) {
    if message.player_entity_id != local_player_id || message.sequence < cache.sequence {
        return;
    }
    cache.sequence = message.sequence;
    cache.generated_at_tick = message.generated_at_tick;
    cache.cell_size_m = message.cell_size_m;
    cache.explored_cells = message.explored_cells.clone();
    cache.live_cells = message.live_cells.clone();
    sort_dedup_cells(&mut cache.explored_cells);
    sort_dedup_cells(&mut cache.live_cells);
}

fn apply_tactical_fog_delta(
    cache: &mut TacticalFogCache,
    local_player_id: &str,
    message: &ServerTacticalFogDeltaMessage,
    now_s: f64,
) -> bool {
    if message.player_entity_id != local_player_id {
        return false;
    }
    if message.base_sequence != cache.sequence {
        if now_s - cache.last_sequence_mismatch_log_at_s >= MISMATCH_LOG_INTERVAL_S {
            warn!(
                "tactical fog delta sequence mismatch: local={} base={} incoming={}; waiting for snapshot",
                cache.sequence, message.base_sequence, message.sequence
            );
            cache.last_sequence_mismatch_log_at_s = now_s;
        }
        return true;
    }
    if message.sequence <= cache.sequence {
        return false;
    }

    let mut explored = cache
        .explored_cells
        .iter()
        .copied()
        .collect::<HashSet<sidereal_net::GridCell>>();
    explored.extend(message.explored_cells_added.iter().copied());

    let mut live = cache
        .live_cells
        .iter()
        .copied()
        .collect::<HashSet<sidereal_net::GridCell>>();
    live.extend(message.live_cells_added.iter().copied());
    for cell in &message.live_cells_removed {
        live.remove(cell);
    }

    cache.explored_cells = explored.into_iter().collect();
    cache.live_cells = live.into_iter().collect();
    sort_dedup_cells(&mut cache.explored_cells);
    sort_dedup_cells(&mut cache.live_cells);
    cache.sequence = message.sequence;
    cache.generated_at_tick = message.generated_at_tick;
    false
}

fn apply_tactical_contacts_snapshot(
    cache: &mut TacticalContactsCache,
    local_player_id: &str,
    message: &ServerTacticalContactsSnapshotMessage,
) {
    if message.player_entity_id != local_player_id || message.sequence < cache.sequence {
        return;
    }
    cache.sequence = message.sequence;
    cache.generated_at_tick = message.generated_at_tick;
    cache.contacts_by_entity_id.clear();
    for contact in &message.contacts {
        cache
            .contacts_by_entity_id
            .insert(contact.entity_id.clone(), contact.clone());
    }
}

fn apply_tactical_contacts_delta(
    cache: &mut TacticalContactsCache,
    local_player_id: &str,
    message: &ServerTacticalContactsDeltaMessage,
    now_s: f64,
) -> bool {
    if message.player_entity_id != local_player_id {
        return false;
    }
    if message.base_sequence != cache.sequence {
        if now_s - cache.last_sequence_mismatch_log_at_s >= MISMATCH_LOG_INTERVAL_S {
            warn!(
                "tactical contacts delta sequence mismatch: local={} base={} incoming={}; waiting for snapshot",
                cache.sequence, message.base_sequence, message.sequence
            );
            cache.last_sequence_mismatch_log_at_s = now_s;
        }
        return true;
    }
    if message.sequence <= cache.sequence {
        return false;
    }
    for entity_id in &message.removals {
        cache.contacts_by_entity_id.remove(entity_id);
    }
    for contact in &message.upserts {
        cache
            .contacts_by_entity_id
            .insert(contact.entity_id.clone(), contact.clone());
    }
    cache.sequence = message.sequence;
    cache.generated_at_tick = message.generated_at_tick;
    false
}

#[allow(clippy::too_many_arguments)]
pub fn receive_tactical_snapshot_messages(
    session: Res<'_, ClientSession>,
    time: Res<'_, Time<Real>>,
    mut fog_cache: ResMut<'_, TacticalFogCache>,
    mut contacts_cache: ResMut<'_, TacticalContactsCache>,
    mut resnapshot_state: ResMut<'_, TacticalResnapshotRequestState>,
    mut request_senders: Query<
        '_,
        '_,
        &mut MessageSender<ClientTacticalResnapshotRequestMessage>,
        (With<Client>, With<Connected>),
    >,
    mut fog_snapshot_receivers: Query<
        '_,
        '_,
        &mut MessageReceiver<ServerTacticalFogSnapshotMessage>,
        (With<Client>, With<Connected>),
    >,
    mut contacts_snapshot_receivers: Query<
        '_,
        '_,
        &mut MessageReceiver<ServerTacticalContactsSnapshotMessage>,
        (With<Client>, With<Connected>),
    >,
    mut fog_delta_receivers: Query<
        '_,
        '_,
        &mut MessageReceiver<ServerTacticalFogDeltaMessage>,
        (With<Client>, With<Connected>),
    >,
    mut contacts_delta_receivers: Query<
        '_,
        '_,
        &mut MessageReceiver<ServerTacticalContactsDeltaMessage>,
        (With<Client>, With<Connected>),
    >,
) {
    let now_s = time.elapsed_secs_f64();
    let Some(local_player_id) = local_player_canonical_id(&session) else {
        return;
    };

    if fog_cache.player_entity_id.as_deref() != Some(local_player_id.as_str()) {
        *fog_cache = TacticalFogCache {
            player_entity_id: Some(local_player_id.clone()),
            ..default()
        };
    }
    if contacts_cache.player_entity_id.as_deref() != Some(local_player_id.as_str()) {
        *contacts_cache = TacticalContactsCache {
            player_entity_id: Some(local_player_id.clone()),
            ..default()
        };
    }
    if resnapshot_state.player_entity_id.as_deref() != Some(local_player_id.as_str()) {
        *resnapshot_state = TacticalResnapshotRequestState {
            player_entity_id: Some(local_player_id.clone()),
            ..Default::default()
        };
    }

    for mut receiver in &mut fog_snapshot_receivers {
        for message in receiver.receive() {
            apply_tactical_fog_snapshot(&mut fog_cache, &local_player_id, &message);
            if message.player_entity_id == local_player_id {
                resnapshot_state.pending_fog = false;
                resnapshot_state.last_fog_snapshot_received_at_s = now_s;
            }
        }
    }

    for mut receiver in &mut contacts_snapshot_receivers {
        for message in receiver.receive() {
            apply_tactical_contacts_snapshot(&mut contacts_cache, &local_player_id, &message);
            if message.player_entity_id == local_player_id {
                resnapshot_state.pending_contacts = false;
                resnapshot_state.last_contacts_snapshot_received_at_s = now_s;
            }
        }
    }

    for mut receiver in &mut fog_delta_receivers {
        for message in receiver.receive() {
            if apply_tactical_fog_delta(&mut fog_cache, &local_player_id, &message, now_s) {
                resnapshot_state.pending_fog = true;
            }
        }
    }
    for mut receiver in &mut contacts_delta_receivers {
        for message in receiver.receive() {
            if apply_tactical_contacts_delta(&mut contacts_cache, &local_player_id, &message, now_s)
            {
                resnapshot_state.pending_contacts = true;
            }
        }
    }

    if fog_cache.sequence > 0
        && now_s - resnapshot_state.last_fog_snapshot_received_at_s >= SNAPSHOT_TIMEOUT_S
    {
        resnapshot_state.pending_fog = true;
    }
    if contacts_cache.sequence > 0
        && now_s - resnapshot_state.last_contacts_snapshot_received_at_s >= SNAPSHOT_TIMEOUT_S
    {
        resnapshot_state.pending_contacts = true;
    }

    if (resnapshot_state.pending_fog || resnapshot_state.pending_contacts)
        && now_s - resnapshot_state.last_request_at_s >= RESNAPSHOT_REQUEST_INTERVAL_S
    {
        let request = ClientTacticalResnapshotRequestMessage {
            player_entity_id: local_player_id,
            request_fog_snapshot: resnapshot_state.pending_fog,
            request_contacts_snapshot: resnapshot_state.pending_contacts,
        };
        for mut sender in &mut request_senders {
            sender.send::<TacticalSnapshotChannel>(request.clone());
        }
        resnapshot_state.last_request_at_s = now_s;
    }
}

#[cfg(test)]
mod tests {
    use super::{
        apply_tactical_contacts_delta, apply_tactical_contacts_snapshot, apply_tactical_fog_delta,
        apply_tactical_fog_snapshot,
    };
    use crate::native::resources::{TacticalContactsCache, TacticalFogCache};
    use sidereal_net::{
        GridCell, ServerTacticalContactsDeltaMessage, ServerTacticalContactsSnapshotMessage,
        ServerTacticalFogDeltaMessage, ServerTacticalFogSnapshotMessage, TacticalContact,
    };

    fn contact(entity_id: &str, pos: [f32; 2]) -> TacticalContact {
        TacticalContact {
            entity_id: entity_id.to_string(),
            kind: "ship".to_string(),
            map_icon_asset_id: Some("map_icon_ship_svg".to_string()),
            faction_id: None,
            position_xy: pos,
            heading_rad: 0.0,
            velocity_xy: None,
            is_live_now: true,
            last_seen_tick: 10,
            classification: None,
            contact_quality: None,
        }
    }

    #[test]
    fn tactical_fog_delta_applies_on_matching_base_sequence() {
        let player_id = "11111111-1111-1111-1111-111111111111".to_string();
        let mut cache = TacticalFogCache {
            player_entity_id: Some(player_id.clone()),
            ..Default::default()
        };
        apply_tactical_fog_snapshot(
            &mut cache,
            &player_id,
            &ServerTacticalFogSnapshotMessage {
                player_entity_id: player_id.clone(),
                sequence: 1,
                cell_size_m: 2000.0,
                explored_cells: vec![GridCell { x: 1, y: 1 }],
                live_cells: vec![GridCell { x: 1, y: 1 }, GridCell { x: 2, y: 2 }],
                generated_at_tick: 1,
            },
        );
        let mismatch = apply_tactical_fog_delta(
            &mut cache,
            &player_id,
            &ServerTacticalFogDeltaMessage {
                player_entity_id: player_id.clone(),
                sequence: 2,
                base_sequence: 1,
                explored_cells_added: vec![GridCell { x: 3, y: 3 }],
                live_cells_added: vec![GridCell { x: 3, y: 3 }],
                live_cells_removed: vec![GridCell { x: 1, y: 1 }],
                generated_at_tick: 2,
            },
            5.0,
        );
        assert!(!mismatch);

        assert_eq!(cache.sequence, 2);
        assert!(cache.explored_cells.contains(&GridCell { x: 3, y: 3 }));
        assert!(cache.live_cells.contains(&GridCell { x: 3, y: 3 }));
        assert!(!cache.live_cells.contains(&GridCell { x: 1, y: 1 }));
    }

    #[test]
    fn tactical_fog_delta_ignores_mismatched_base_sequence() {
        let player_id = "11111111-1111-1111-1111-111111111111".to_string();
        let mut cache = TacticalFogCache {
            player_entity_id: Some(player_id.clone()),
            sequence: 4,
            explored_cells: vec![GridCell { x: 5, y: 5 }],
            ..Default::default()
        };
        let mismatch = apply_tactical_fog_delta(
            &mut cache,
            &player_id,
            &ServerTacticalFogDeltaMessage {
                player_entity_id: player_id.clone(),
                sequence: 6,
                base_sequence: 3,
                explored_cells_added: vec![GridCell { x: 9, y: 9 }],
                live_cells_added: vec![],
                live_cells_removed: vec![],
                generated_at_tick: 10,
            },
            3.0,
        );
        assert!(mismatch);
        assert_eq!(cache.sequence, 4);
        assert!(!cache.explored_cells.contains(&GridCell { x: 9, y: 9 }));
    }

    #[test]
    fn tactical_contacts_delta_applies_upserts_and_removals() {
        let player_id = "11111111-1111-1111-1111-111111111111".to_string();
        let mut cache = TacticalContactsCache {
            player_entity_id: Some(player_id.clone()),
            ..Default::default()
        };
        apply_tactical_contacts_snapshot(
            &mut cache,
            &player_id,
            &ServerTacticalContactsSnapshotMessage {
                player_entity_id: player_id.clone(),
                sequence: 1,
                contacts: vec![contact("a", [1.0, 1.0]), contact("b", [2.0, 2.0])],
                generated_at_tick: 1,
            },
        );

        let mismatch = apply_tactical_contacts_delta(
            &mut cache,
            &player_id,
            &ServerTacticalContactsDeltaMessage {
                player_entity_id: player_id.clone(),
                sequence: 2,
                base_sequence: 1,
                upserts: vec![contact("a", [9.0, 9.0]), contact("c", [3.0, 3.0])],
                removals: vec!["b".to_string()],
                generated_at_tick: 2,
            },
            6.0,
        );
        assert!(!mismatch);

        assert_eq!(cache.sequence, 2);
        assert_eq!(cache.contacts_by_entity_id.len(), 2);
        assert_eq!(
            cache.contacts_by_entity_id.get("a").map(|c| c.position_xy),
            Some([9.0, 9.0])
        );
        assert!(cache.contacts_by_entity_id.contains_key("c"));
        assert!(!cache.contacts_by_entity_id.contains_key("b"));
    }
}
