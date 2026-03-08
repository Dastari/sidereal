use avian2d::prelude::{LinearVelocity, Position, Rotation};
use base64::Engine;
use base64::engine::general_purpose::STANDARD_NO_PAD;
use bevy::prelude::*;
use lightyear::prelude::client::Connected;
use lightyear::prelude::server::{ClientOf, LinkOf};
use lightyear::prelude::{
    NetworkTarget, RemoteId, ReplicationState, Server, ServerMultiMessageSender,
};
use sidereal_game::{
    EntityGuid, EntityLabels, FactionId, MapIcon, PlayerExploredCells, PlayerExploredCellsChunk,
    PlayerExploredCellsChunkEncoding, PlayerTag, VisibilityDisclosure, VisibilityGridCell,
    VisibilityRangeSource, VisibilitySpatialGrid,
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
const FOG_CELL_SIZE_M: f32 = 100.0;

#[derive(Debug, Default)]
struct PlayerTacticalStreamState {
    fog_sequence: u64,
    contacts_sequence: u64,
    last_sent_at_s: f64,
    last_snapshot_at_s: f64,
    initialized: bool,
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
        (
            Entity,
            &'_ mut MessageReceiver<ClientTacticalResnapshotRequestMessage>,
        ),
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
                stream_state
                    .forced_snapshot_by_player
                    .insert(message_player_id);
            }
        }
    }
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
        || previous.map_icon_asset_id != current.map_icon_asset_id
        || previous.faction_id != current.faction_id
        || previous.position_xy != current.position_xy
        || previous.heading_rad != current.heading_rad
        || previous.velocity_xy != current.velocity_xy
        || previous.is_live_now != current.is_live_now
        || previous.classification != current.classification
        || previous.contact_quality != current.contact_quality
}

fn compute_live_cells_delta(
    previous_live_cells: &HashSet<GridCell>,
    current_live_cells: &HashSet<GridCell>,
) -> (Vec<GridCell>, Vec<GridCell>) {
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

    (live_cells_added, live_cells_removed)
}

fn chunk_cell_coordinates(
    cell: GridCell,
    chunk_size_cells: u16,
) -> Option<(i32, i32, u16, u16, usize)> {
    let chunk_size_i32 = i32::from(chunk_size_cells.max(1));
    let chunk_x = cell.x.div_euclid(chunk_size_i32);
    let chunk_y = cell.y.div_euclid(chunk_size_i32);
    let local_x = u16::try_from(cell.x.rem_euclid(chunk_size_i32)).ok()?;
    let local_y = u16::try_from(cell.y.rem_euclid(chunk_size_i32)).ok()?;
    let index = usize::from(local_y) * usize::from(chunk_size_cells) + usize::from(local_x);
    Some((chunk_x, chunk_y, local_x, local_y, index))
}

fn words_len_for_chunk(chunk_size_cells: u16) -> usize {
    let cell_count = usize::from(chunk_size_cells) * usize::from(chunk_size_cells);
    cell_count.div_ceil(64)
}

fn decode_chunk_words(chunk: &PlayerExploredCellsChunk, chunk_size_cells: u16) -> Vec<u64> {
    let mut words = vec![0_u64; words_len_for_chunk(chunk_size_cells)];
    let Some(bytes) = chunk.payload_bytes() else {
        return words;
    };
    match chunk.encoding {
        PlayerExploredCellsChunkEncoding::Bitset => {
            for (word, chunk_bytes) in words.iter_mut().zip(bytes.chunks_exact(8)) {
                let mut raw = [0_u8; 8];
                raw.copy_from_slice(chunk_bytes);
                *word = u64::from_le_bytes(raw);
            }
        }
        PlayerExploredCellsChunkEncoding::SparseDeltaVarint => {
            let mut cursor = 0usize;
            let mut value = 0u32;
            while cursor < bytes.len() {
                let mut shift = 0u32;
                let mut delta = 0u32;
                loop {
                    if cursor >= bytes.len() {
                        break;
                    }
                    let byte = bytes[cursor];
                    cursor += 1;
                    delta |= u32::from(byte & 0x7F) << shift;
                    if byte & 0x80 == 0 {
                        break;
                    }
                    shift += 7;
                    if shift >= 32 {
                        break;
                    }
                }
                value = value.saturating_add(delta);
                let idx = usize::try_from(value).unwrap_or(usize::MAX);
                let word_idx = idx / 64;
                let bit_idx = idx % 64;
                if word_idx < words.len() {
                    words[word_idx] |= 1_u64 << bit_idx;
                }
            }
        }
    }
    words
}

fn count_set_bits(words: &[u64]) -> u16 {
    let total = words.iter().map(|word| word.count_ones()).sum::<u32>();
    u16::try_from(total).unwrap_or(u16::MAX)
}

fn encode_varint_u32(mut value: u32, out: &mut Vec<u8>) {
    loop {
        let mut byte = (value & 0x7F) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if value == 0 {
            break;
        }
    }
}

fn encode_chunk_words(
    chunk_x: i32,
    chunk_y: i32,
    chunk_size_cells: u16,
    words: &[u64],
) -> PlayerExploredCellsChunk {
    let explored_count = count_set_bits(words);
    let mut set_indices = Vec::<u32>::new();
    let cell_count = usize::from(chunk_size_cells) * usize::from(chunk_size_cells);
    for idx in 0..cell_count {
        let word_idx = idx / 64;
        let bit_idx = idx % 64;
        if word_idx < words.len() && (words[word_idx] & (1_u64 << bit_idx)) != 0 {
            set_indices.push(idx as u32);
        }
    }

    let sparse_threshold = cell_count / 16;
    if set_indices.len() <= sparse_threshold {
        let mut sparse_bytes = Vec::<u8>::new();
        let mut prev = 0u32;
        for idx in set_indices {
            let delta = idx.saturating_sub(prev);
            encode_varint_u32(delta, &mut sparse_bytes);
            prev = idx;
        }
        return PlayerExploredCellsChunk {
            chunk_x,
            chunk_y,
            explored_count,
            encoding: PlayerExploredCellsChunkEncoding::SparseDeltaVarint,
            payload_b64: STANDARD_NO_PAD.encode(sparse_bytes),
        };
    }

    let mut bitset_bytes = Vec::<u8>::with_capacity(words.len() * 8);
    for word in words {
        bitset_bytes.extend_from_slice(&word.to_le_bytes());
    }
    PlayerExploredCellsChunk {
        chunk_x,
        chunk_y,
        explored_count,
        encoding: PlayerExploredCellsChunkEncoding::Bitset,
        payload_b64: STANDARD_NO_PAD.encode(bitset_bytes),
    }
}

fn ensure_explored_memory_shape(
    memory: &mut PlayerExploredCells,
    cell_size_m: f32,
    chunk_size_cells: u16,
) {
    let expected_chunk_size = chunk_size_cells.max(1);
    let cell_size_changed = (memory.cell_size_m - cell_size_m).abs() > f32::EPSILON;
    let chunk_size_changed = memory.chunk_size_cells != expected_chunk_size;
    if cell_size_changed || chunk_size_changed {
        memory.cell_size_m = cell_size_m;
        memory.chunk_size_cells = expected_chunk_size;
        memory.chunks.clear();
    }
}

fn grid_cell_from_component(cell: &VisibilityGridCell) -> Option<GridCell> {
    let x = i32::try_from(cell.x).ok()?;
    let y = i32::try_from(cell.y).ok()?;
    Some(GridCell { x, y })
}

fn materialize_explored_cells(memory: &PlayerExploredCells) -> Vec<GridCell> {
    let chunk_size = memory.chunk_size_cells.max(1);
    let chunk_size_i32 = i32::from(chunk_size);
    let mut explored = Vec::<GridCell>::new();
    for chunk in &memory.chunks {
        let words = decode_chunk_words(chunk, chunk_size);
        let cell_count = usize::from(chunk_size) * usize::from(chunk_size);
        for idx in 0..cell_count {
            let word_idx = idx / 64;
            let bit_idx = idx % 64;
            if word_idx >= words.len() || (words[word_idx] & (1_u64 << bit_idx)) == 0 {
                continue;
            }
            let local_x = (idx % usize::from(chunk_size)) as i32;
            let local_y = (idx / usize::from(chunk_size)) as i32;
            let world_x = chunk
                .chunk_x
                .saturating_mul(chunk_size_i32)
                .saturating_add(local_x);
            let world_y = chunk
                .chunk_y
                .saturating_mul(chunk_size_i32)
                .saturating_add(local_y);
            explored.push(GridCell {
                x: world_x,
                y: world_y,
            });
        }
    }
    explored.sort_by_key(|cell| (cell.x, cell.y));
    explored.dedup_by_key(|cell| (cell.x, cell.y));
    explored
}

fn apply_live_cells_to_explored_memory(
    memory: &mut PlayerExploredCells,
    live_cells: &HashSet<GridCell>,
) -> Vec<GridCell> {
    let chunk_size = memory.chunk_size_cells.max(1);
    let words_len = words_len_for_chunk(chunk_size);
    let mut chunk_index_by_key = memory
        .chunks
        .iter()
        .enumerate()
        .map(|(index, chunk)| ((chunk.chunk_x, chunk.chunk_y), index))
        .collect::<HashMap<_, _>>();
    let mut touched_words = HashMap::<(i32, i32), Vec<u64>>::new();
    let mut touched_dirty = HashSet::<(i32, i32)>::new();
    let mut explored_cells_added = Vec::<GridCell>::new();

    for cell in live_cells {
        let Some((chunk_x, chunk_y, _, _, bit_index)) = chunk_cell_coordinates(*cell, chunk_size)
        else {
            continue;
        };
        let key = (chunk_x, chunk_y);
        let words = touched_words.entry(key).or_insert_with(|| {
            if let Some(existing_index) = chunk_index_by_key.get(&key) {
                decode_chunk_words(&memory.chunks[*existing_index], chunk_size)
            } else {
                vec![0_u64; words_len]
            }
        });
        if bit_index / 64 >= words.len() {
            continue;
        }
        let word = &mut words[bit_index / 64];
        let mask = 1_u64 << (bit_index % 64);
        if (*word & mask) != 0 {
            continue;
        }
        *word |= mask;
        touched_dirty.insert(key);
        explored_cells_added.push(*cell);
    }

    for (key, words) in touched_words {
        if !touched_dirty.contains(&key) {
            continue;
        }
        let encoded = encode_chunk_words(key.0, key.1, chunk_size, &words);
        if let Some(existing_index) = chunk_index_by_key.get(&key).copied() {
            memory.chunks[existing_index] = encoded;
        } else {
            let index = memory.chunks.len();
            memory.chunks.push(encoded);
            chunk_index_by_key.insert(key, index);
        }
    }

    memory
        .chunks
        .sort_by_key(|chunk| (chunk.chunk_x, chunk.chunk_y));
    explored_cells_added.sort_by_key(|cell| (cell.x, cell.y));
    explored_cells_added
}

fn circle_intersects_grid_cell(
    center_x: f32,
    center_y: f32,
    range_m: f32,
    cell_x: i64,
    cell_y: i64,
    cell_size_m: f32,
) -> bool {
    let min_x = cell_x as f32 * cell_size_m;
    let min_y = cell_y as f32 * cell_size_m;
    let max_x = min_x + cell_size_m;
    let max_y = min_y + cell_size_m;

    let closest_x = center_x.clamp(min_x, max_x);
    let closest_y = center_y.clamp(min_y, max_y);
    let dx = center_x - closest_x;
    let dy = center_y - closest_y;
    let distance_squared = dx * dx + dy * dy;
    distance_squared <= range_m * range_m
}

fn build_live_cells_from_visibility_sources(
    visibility_sources: &[VisibilityRangeSource],
    cell_size_m: f32,
) -> HashSet<GridCell> {
    let mut cells = HashSet::<GridCell>::new();
    if !cell_size_m.is_finite() || cell_size_m <= 0.0 {
        return cells;
    }

    for source in visibility_sources {
        if !source.x.is_finite()
            || !source.y.is_finite()
            || !source.range_m.is_finite()
            || source.range_m <= 0.0
        {
            continue;
        }

        let cell_radius = (source.range_m / cell_size_m).ceil() as i64;
        let center_cell_x = (source.x / cell_size_m).floor() as i64;
        let center_cell_y = (source.y / cell_size_m).floor() as i64;

        for dx in -cell_radius..=cell_radius {
            for dy in -cell_radius..=cell_radius {
                let cell_x = center_cell_x + dx;
                let cell_y = center_cell_y + dy;
                if !circle_intersects_grid_cell(
                    source.x,
                    source.y,
                    source.range_m,
                    cell_x,
                    cell_y,
                    cell_size_m,
                ) {
                    continue;
                }
                let Ok(x) = i32::try_from(cell_x) else {
                    continue;
                };
                let Ok(y) = i32::try_from(cell_y) else {
                    continue;
                };
                cells.insert(GridCell { x, y });
            }
        }
    }

    cells
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
#[allow(clippy::too_many_arguments)]
pub fn stream_tactical_snapshot_messages(
    mut commands: Commands<'_, '_>,
    server_query: Query<'_, '_, &'_ Server>,
    mut sender: ServerMultiMessageSender<'_, '_, With<Connected>>,
    time: Res<'_, Time<Real>>,
    mut stream_state: ResMut<'_, TacticalStreamState>,
    bindings: Res<'_, AuthenticatedClientBindings>,
    client_remotes: Query<'_, '_, (&'_ LinkOf, &'_ RemoteId), With<ClientOf>>,
    player_entities: Res<'_, PlayerRuntimeEntityMap>,
    mut player_visibility: Query<
        '_,
        '_,
        (
            &'_ VisibilitySpatialGrid,
            Option<&'_ VisibilityDisclosure>,
            Option<&'_ mut PlayerExploredCells>,
        ),
        With<PlayerTag>,
    >,
    replicated_entities: Query<
        '_,
        '_,
        (
            Option<&'_ EntityGuid>,
            Option<&'_ EntityLabels>,
            Option<&'_ FactionId>,
            Option<&'_ MapIcon>,
            Option<&'_ Position>,
            Option<&'_ GlobalTransform>,
            Option<&'_ Rotation>,
            Option<&'_ LinearVelocity>,
            Option<&'_ PlayerTag>,
            &'_ ReplicationState,
        ),
    >,
) {
    stream_state.tick = stream_state.tick.saturating_add(1);
    let generated_at_tick = stream_state.tick;
    let now_s = time.elapsed_secs_f64();

    for (client_entity, bound_player_id) in &bindings.by_client_entity {
        let Some(player_entity_id) =
            PlayerEntityId::parse(bound_player_id.as_str()).map(|id| id.canonical_wire_id())
        else {
            continue;
        };
        let Ok((link_of, remote_id)) = client_remotes.get(*client_entity) else {
            continue;
        };
        let Ok(server) = server_query.get(link_of.server) else {
            continue;
        };
        let Some(&player_entity) = player_entities
            .by_player_entity_id
            .get(player_entity_id.as_str())
        else {
            continue;
        };
        let Ok((player_grid, visibility_disclosure, explored_component)) =
            player_visibility.get_mut(player_entity)
        else {
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

        let fog_cell_size_m = FOG_CELL_SIZE_M;
        let scanner_live_cells = visibility_disclosure
            .map(|disclosure| {
                build_live_cells_from_visibility_sources(
                    disclosure.visibility_sources.as_slice(),
                    fog_cell_size_m,
                )
            })
            .unwrap_or_default();
        let live_cells = if scanner_live_cells.is_empty() {
            if (player_grid.cell_size_m - fog_cell_size_m).abs() <= f32::EPSILON {
                player_grid
                    .queried_cells
                    .iter()
                    .filter_map(grid_cell_from_component)
                    .collect::<HashSet<_>>()
            } else {
                HashSet::new()
            }
        } else {
            scanner_live_cells
        };
        let needs_snapshot = force_snapshot
            || !state.initialized
            || now_s - state.last_snapshot_at_s >= SNAPSHOT_RESYNC_INTERVAL_S;

        let (explored_cells_added, explored_cells_snapshot) =
            if let Some(mut component) = explored_component {
                ensure_explored_memory_shape(
                    &mut component,
                    fog_cell_size_m,
                    PlayerExploredCells::DEFAULT_CHUNK_SIZE_CELLS,
                );
                let added = apply_live_cells_to_explored_memory(&mut component, &live_cells);
                let snapshot = if needs_snapshot {
                    materialize_explored_cells(&component)
                } else {
                    Vec::new()
                };
                (added, snapshot)
            } else {
                let mut component = PlayerExploredCells::empty_for_fog();
                ensure_explored_memory_shape(
                    &mut component,
                    fog_cell_size_m,
                    PlayerExploredCells::DEFAULT_CHUNK_SIZE_CELLS,
                );
                let added = apply_live_cells_to_explored_memory(&mut component, &live_cells);
                let snapshot = if needs_snapshot {
                    materialize_explored_cells(&component)
                } else {
                    Vec::new()
                };
                commands.entity(player_entity).insert(component);
                (added, snapshot)
            };

        let mut contacts_by_entity_id = HashMap::<String, TacticalContact>::new();
        for (
            guid,
            labels,
            faction_id,
            map_icon,
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
            contacts_by_entity_id.insert(
                entity_id.clone(),
                TacticalContact {
                    entity_id: guid.0.to_string(),
                    kind: contact_kind_from_labels(labels),
                    map_icon_asset_id: map_icon.map(|icon| icon.asset_id.clone()),
                    faction_id: faction_id.map(|id| id.0.clone()),
                    position_xy: [world.x, world.y],
                    heading_rad: rotation.map_or(0.0, |value| value.as_radians()),
                    velocity_xy: linear_velocity.map(|v| [v.0.x, v.0.y]),
                    is_live_now: true,
                    last_seen_tick: generated_at_tick,
                    classification: None,
                    contact_quality: None,
                },
            );
        }

        let target = NetworkTarget::Single(remote_id.0);

        if needs_snapshot {
            state.fog_sequence = state.fog_sequence.saturating_add(1);
            let mut live_cells_snapshot = live_cells.iter().copied().collect::<Vec<_>>();
            live_cells_snapshot.sort_by_key(|cell| (cell.x, cell.y));
            let fog_message = ServerTacticalFogSnapshotMessage {
                player_entity_id: player_entity_id.clone(),
                sequence: state.fog_sequence,
                cell_size_m: fog_cell_size_m,
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
            let (live_cells_added, live_cells_removed) =
                compute_live_cells_delta(&state.live_cells, &live_cells);
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
    use super::{
        apply_live_cells_to_explored_memory, build_live_cells_from_visibility_sources,
        circle_intersects_grid_cell, compute_contacts_delta, compute_live_cells_delta,
        ensure_explored_memory_shape, materialize_explored_cells,
    };
    use sidereal_game::{PlayerExploredCells, VisibilityRangeSource};
    use sidereal_net::{GridCell, TacticalContact};
    use std::collections::{HashMap, HashSet};

    fn contact(entity_id: &str, position_xy: [f32; 2], last_seen_tick: u64) -> TacticalContact {
        TacticalContact {
            entity_id: entity_id.to_string(),
            kind: "ship".to_string(),
            map_icon_asset_id: Some("map_icon_ship_svg".to_string()),
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
    fn live_delta_reports_added_and_removed_cells() {
        let previous_live = HashSet::from([GridCell { x: 1, y: 1 }, GridCell { x: 2, y: 2 }]);
        let current_live = HashSet::from([GridCell { x: 3, y: 3 }, GridCell { x: 2, y: 2 }]);

        let (live_added, live_removed) = compute_live_cells_delta(&previous_live, &current_live);

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

    #[test]
    fn explored_memory_chunk_encoding_roundtrip() {
        let mut memory = PlayerExploredCells::empty_for_fog();
        ensure_explored_memory_shape(
            &mut memory,
            PlayerExploredCells::DEFAULT_FOG_CELL_SIZE_M,
            PlayerExploredCells::DEFAULT_CHUNK_SIZE_CELLS,
        );
        let live = HashSet::from([
            GridCell { x: 2, y: 2 },
            GridCell { x: 3, y: 2 },
            GridCell { x: -1, y: -1 },
        ]);
        let added = apply_live_cells_to_explored_memory(&mut memory, &live);
        assert_eq!(added.len(), 3);
        let explored = materialize_explored_cells(&memory);
        assert_eq!(
            explored,
            vec![
                GridCell { x: -1, y: -1 },
                GridCell { x: 2, y: 2 },
                GridCell { x: 3, y: 2 }
            ]
        );
    }

    #[test]
    fn explored_memory_resets_when_shape_changes() {
        let mut memory = PlayerExploredCells::empty_for_fog();
        let live = HashSet::from([GridCell { x: 3, y: 3 }]);
        let _ = apply_live_cells_to_explored_memory(&mut memory, &live);
        assert!(!memory.chunks.is_empty());

        ensure_explored_memory_shape(&mut memory, 250.0, 32);
        assert_eq!(memory.cell_size_m, 250.0);
        assert_eq!(memory.chunk_size_cells, 32);
        assert!(memory.chunks.is_empty());
    }

    #[test]
    fn circle_cell_intersection_rejects_diagonal_outside_range() {
        assert!(circle_intersects_grid_cell(0.0, 0.0, 100.0, 0, 0, 100.0));
        assert!(circle_intersects_grid_cell(0.0, 0.0, 100.0, 1, 0, 100.0));
        assert!(!circle_intersects_grid_cell(0.0, 0.0, 100.0, 1, 1, 100.0));
    }

    #[test]
    fn visibility_sources_rasterize_to_circle_intersection_cells() {
        let visibility_sources = vec![VisibilityRangeSource {
            x: 50.0,
            y: 50.0,
            z: 0.0,
            range_m: 50.0,
        }];

        let cells = build_live_cells_from_visibility_sources(visibility_sources.as_slice(), 100.0);

        assert!(cells.contains(&GridCell { x: 0, y: 0 }));
        assert!(cells.contains(&GridCell { x: 1, y: 0 }));
        assert!(cells.contains(&GridCell { x: -1, y: 0 }));
        assert!(cells.contains(&GridCell { x: 0, y: 1 }));
        assert!(cells.contains(&GridCell { x: 0, y: -1 }));
        assert!(!cells.contains(&GridCell { x: 1, y: 1 }));
        assert!(!cells.contains(&GridCell { x: -1, y: -1 }));
    }
}
