use avian2d::prelude::{LinearVelocity, Position, Rotation};
use base64::Engine;
use base64::engine::general_purpose::STANDARD_NO_PAD;
use bevy::{math::DVec3, prelude::*};
use lightyear::prelude::client::Connected;
use lightyear::prelude::server::{ClientOf, LinkOf};
use lightyear::prelude::{
    NetworkTarget, RemoteId, ReplicationState, Server, ServerMultiMessageSender,
};
use sidereal_game::{
    ContactResolutionM, EntityGuid, EntityLabels, FactionId, MapIcon, MountedOn,
    PlayerExploredCells, PlayerExploredCellsChunk, PlayerExploredCellsChunkEncoding, PlayerTag,
    ScannerComponent, ScannerContactDetailTier, SignalSignature, SizeM, StaticLandmark,
    TotalMassKg, VisibilityRangeM, VisibilityRangeSource, WorldPosition,
};
use sidereal_net::{
    ClientTacticalResnapshotRequestMessage, GridCell, NotificationPayload, NotificationPlacement,
    NotificationSeverity, PlayerEntityId, ServerTacticalContactsDeltaMessage,
    ServerTacticalContactsSnapshotMessage, ServerTacticalFogDeltaMessage,
    ServerTacticalFogSnapshotMessage, TacticalContact, TacticalDeltaChannel,
    TacticalSnapshotChannel,
};
use std::collections::{HashMap, HashSet};

use crate::replication::auth::AuthenticatedClientBindings;
use crate::replication::notifications::{
    NotificationCommand, NotificationCommandQueue, enqueue_player_notification,
};
use crate::replication::{PlayerControlledEntityMap, PlayerRuntimeEntityMap};
use lightyear::prelude::MessageReceiver;

const UPDATE_INTERVAL_S: f64 = 0.5;
const SNAPSHOT_RESYNC_INTERVAL_S: f64 = 2.0;
const FOG_CELL_SIZE_M: f32 = 100.0;
const DEFAULT_CONTACT_RESOLUTION_M: f32 = 100.0;
const UNKNOWN_CONTACT_ICON_ASSET_ID: &str = "map_icon_unknown_contact_svg";
const GRAVITY_WELL_SIGNAL_EVENT_TYPE: &str = "long_range_gravity_well_detected";
const SIGNAL_CONTACT_MIN_STRENGTH: f32 = 0.15;

type ControlledScannerSourceQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static EntityGuid,
        Option<&'static ScannerComponent>,
        Option<&'static VisibilityRangeM>,
        Option<&'static Position>,
        Option<&'static WorldPosition>,
        Option<&'static GlobalTransform>,
    ),
>;

type MountedScannerSourceQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static EntityGuid,
        &'static ScannerComponent,
        Option<&'static VisibilityRangeM>,
        Option<&'static MountedOn>,
    ),
>;

#[derive(Debug, Clone, PartialEq)]
struct SignalContactMemory {
    contact_id: String,
    approximate_position_xy: [f64; 2],
    position_accuracy_m: f32,
    strongest_signal_strength: f32,
    last_detected_tick: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct EffectiveScannerSource {
    scanner_guid: uuid::Uuid,
    source_position: DVec3,
    range_m: f32,
    detail_tier: ScannerContactDetailTier,
    level: u8,
    max_contacts: u16,
    contact_resolution_m: f32,
}

impl EffectiveScannerSource {
    fn visibility_range_source(self) -> VisibilityRangeSource {
        VisibilityRangeSource {
            x: self.source_position.x as f32,
            y: self.source_position.y as f32,
            z: self.source_position.z as f32,
            range_m: self.range_m,
        }
    }
}

#[derive(Debug, Default)]
struct PlayerTacticalStreamState {
    fog_sequence: u64,
    contacts_sequence: u64,
    last_sent_at_s: f64,
    last_snapshot_at_s: f64,
    initialized: bool,
    live_cells: HashSet<GridCell>,
    contacts_by_entity_id: HashMap<String, TacticalContact>,
    signal_contacts_by_target: HashMap<String, SignalContactMemory>,
    notified_long_range_signal_targets: HashSet<String>,
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
        || previous.size_m != current.size_m
        || previous.mass_kg != current.mass_kg
        || previous.heading_rad != current.heading_rad
        || previous.velocity_xy != current.velocity_xy
        || previous.is_live_now != current.is_live_now
        || previous.classification != current.classification
        || previous.contact_quality != current.contact_quality
        || previous.signal_strength != current.signal_strength
        || previous.position_accuracy_m != current.position_accuracy_m
}

fn stable_signal_contact_id(player_entity_id: &str, target_entity_id: &str) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in player_entity_id
        .bytes()
        .chain([b':'])
        .chain(target_entity_id.bytes())
    {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("signal-{hash:016x}")
}

fn entity_extent_m(size: Option<&SizeM>) -> f32 {
    let Some(size) = size else {
        return 0.0;
    };
    let max_dimension = size.length.max(size.width).max(size.height);
    if max_dimension.is_finite() && max_dimension > 0.0 {
        max_dimension * 0.5
    } else {
        0.0
    }
}

fn tactical_world_position(
    position: Option<&Position>,
    world_position: Option<&WorldPosition>,
    global_transform: Option<&GlobalTransform>,
) -> DVec3 {
    if let Some(position) = position
        && position.0.is_finite()
    {
        return position.0.extend(0.0);
    }
    if let Some(world_position) = world_position
        && world_position.0.is_finite()
    {
        return world_position.0.extend(0.0);
    }
    global_transform
        .map(|value| value.translation().as_dvec3())
        .unwrap_or(DVec3::ZERO)
}

fn finite_positive(value: f32) -> Option<f32> {
    (value.is_finite() && value > 0.0).then_some(value)
}

fn scanner_effective_range(
    scanner: &ScannerComponent,
    visibility_range: Option<&VisibilityRangeM>,
) -> Option<f32> {
    visibility_range
        .and_then(|value| finite_positive(value.0))
        .or_else(|| finite_positive(scanner.base_range_m))
}

fn scanner_candidate(
    source_position: DVec3,
    scanner_guid: uuid::Uuid,
    scanner: &ScannerComponent,
    visibility_range: Option<&VisibilityRangeM>,
    contact_resolution_m: f32,
) -> Option<EffectiveScannerSource> {
    if !source_position.is_finite() {
        return None;
    }
    Some(EffectiveScannerSource {
        scanner_guid,
        source_position,
        range_m: scanner_effective_range(scanner, visibility_range)?,
        detail_tier: scanner.detail_tier,
        level: scanner.level,
        max_contacts: scanner.max_contacts.max(1),
        contact_resolution_m: contact_resolution_m.max(1.0),
    })
}

fn scanner_candidate_is_better(
    current: &EffectiveScannerSource,
    candidate: &EffectiveScannerSource,
) -> bool {
    (
        candidate.detail_tier,
        candidate.range_m.to_bits(),
        candidate.level,
        candidate.max_contacts,
        std::cmp::Reverse(candidate.scanner_guid),
    ) > (
        current.detail_tier,
        current.range_m.to_bits(),
        current.level,
        current.max_contacts,
        std::cmp::Reverse(current.scanner_guid),
    )
}

fn best_effective_scanner_candidate(
    current: Option<EffectiveScannerSource>,
    candidate: EffectiveScannerSource,
) -> Option<EffectiveScannerSource> {
    match current {
        Some(current) if !scanner_candidate_is_better(&current, &candidate) => Some(current),
        _ => Some(candidate),
    }
}

fn resolve_effective_scanner_source(
    player_id: &PlayerEntityId,
    player_entity: Entity,
    controlled_entity_map: &PlayerControlledEntityMap,
    controlled_sources: &ControlledScannerSourceQuery<'_, '_>,
    mounted_sources: &MountedScannerSourceQuery<'_, '_>,
    contact_resolution_m: f32,
) -> Option<EffectiveScannerSource> {
    let controlled_entity = controlled_entity_map
        .by_player_entity_id
        .get(player_id)
        .copied()?;
    if controlled_entity == player_entity {
        return None;
    }

    let Ok((
        _controlled_entity,
        controlled_guid,
        root_scanner,
        root_visibility_range,
        position,
        world_position,
        global_transform,
    )) = controlled_sources.get(controlled_entity)
    else {
        return None;
    };
    let source_position = tactical_world_position(position, world_position, global_transform);

    let mut best = root_scanner.and_then(|scanner| {
        scanner_candidate(
            source_position,
            controlled_guid.0,
            scanner,
            root_visibility_range,
            contact_resolution_m,
        )
    });

    for (scanner_guid, scanner, visibility_range, mounted_on) in mounted_sources.iter() {
        if mounted_on.is_none_or(|mounted_on| mounted_on.parent_entity_id != controlled_guid.0) {
            continue;
        }
        if let Some(candidate) = scanner_candidate(
            source_position,
            scanner_guid.0,
            scanner,
            visibility_range,
            contact_resolution_m,
        ) {
            best = best_effective_scanner_candidate(best, candidate);
        }
    }

    best
}

fn scanner_detects_world(
    scanner_source: EffectiveScannerSource,
    target_world: DVec3,
    entity_extent_m: f32,
) -> bool {
    if !target_world.is_finite()
        || !scanner_source.source_position.is_finite()
        || !scanner_source.range_m.is_finite()
        || scanner_source.range_m <= 0.0
    {
        return false;
    }
    let extent_m = finite_positive(entity_extent_m).unwrap_or(0.0);
    let effective_range_m = scanner_source.range_m + extent_m;
    effective_range_m.is_finite()
        && target_world.distance(scanner_source.source_position) <= f64::from(effective_range_m)
}

fn signal_contact_quality(relative_strength: f32) -> Option<&'static str> {
    if !relative_strength.is_finite() || relative_strength < SIGNAL_CONTACT_MIN_STRENGTH {
        return None;
    }
    if relative_strength < 0.35 {
        Some("weak")
    } else if relative_strength < 0.65 {
        Some("moderate")
    } else if relative_strength < 1.0 {
        Some("strong")
    } else {
        Some("overwhelming")
    }
}

fn strongest_signal_detection(
    visibility_sources: &[VisibilityRangeSource],
    signal: &SignalSignature,
    target_world: DVec3,
    entity_extent_m: f32,
) -> Option<(f32, &'static str)> {
    if signal.strength <= 0.0 || signal.detection_radius_m <= 0.0 {
        return None;
    }
    let mut best_strength = None::<f32>;
    for source in visibility_sources {
        if !source.x.is_finite()
            || !source.y.is_finite()
            || !source.z.is_finite()
            || !source.range_m.is_finite()
            || source.range_m <= 0.0
        {
            continue;
        }
        let source_position = DVec3::new(source.x as f64, source.y as f64, source.z as f64);
        let effective_range_m = source.range_m
            + signal.detection_radius_m
            + if signal.use_extent_for_detection {
                entity_extent_m
            } else {
                0.0
            };
        if !effective_range_m.is_finite() || effective_range_m <= 0.0 {
            continue;
        }
        let distance_m = target_world.distance(source_position) as f32;
        if distance_m > effective_range_m {
            continue;
        }
        let normalized = 1.0 - distance_m / effective_range_m;
        let relative_strength = signal.strength * normalized.clamp(0.0, 1.0);
        best_strength = Some(
            best_strength
                .map(|current| current.max(relative_strength))
                .unwrap_or(relative_strength),
        );
    }
    let relative_strength = best_strength?;
    signal_contact_quality(relative_strength).map(|quality| (relative_strength, quality))
}

fn approximate_signal_contact_position(target_world: DVec3, resolution_m: f32) -> [f64; 2] {
    let resolution = resolution_m.max(1.0) as f64;
    [
        (target_world.x / resolution).floor() * resolution + resolution * 0.5,
        (target_world.y / resolution).floor() * resolution + resolution * 0.5,
    ]
}

fn should_update_signal_memory(memory: &SignalContactMemory, next_resolution_m: f32) -> bool {
    next_resolution_m.is_finite()
        && next_resolution_m > 0.0
        && next_resolution_m < memory.position_accuracy_m * 0.75
}

fn is_gravity_well_landmark(static_landmark: Option<&StaticLandmark>) -> bool {
    let Some(static_landmark) = static_landmark else {
        return false;
    };
    matches!(
        static_landmark.kind.trim().to_ascii_lowercase().as_str(),
        "planet" | "star" | "blackhole" | "black_hole"
    )
}

fn enqueue_gravity_well_signal_notification(
    queue: &mut NotificationCommandQueue,
    player_entity_id: &str,
    contact: &TacticalContact,
) {
    enqueue_player_notification(
        queue,
        NotificationCommand {
            player_entity_id: player_entity_id.to_string(),
            title: "Long-Range Sensor Contact".to_string(),
            body: "A gravity well has been detected nearby.".to_string(),
            severity: NotificationSeverity::Info,
            placement: NotificationPlacement::BottomRight,
            image: None,
            payload: NotificationPayload::Generic {
                event_type: GRAVITY_WELL_SIGNAL_EVENT_TYPE.to_string(),
                data: serde_json::json!({
                    "contact_id": contact.entity_id,
                    "signal_strength": contact.signal_strength,
                    "contact_quality": contact.contact_quality,
                    "position_accuracy_m": contact.position_accuracy_m,
                    "approximate_position_xy": contact.position_xy,
                }),
            },
            auto_dismiss_after_s: None,
        },
    );
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

fn contact_budget_rank(contact: &TacticalContact) -> (u8, std::cmp::Reverse<u32>, String) {
    let signal_rank = contact
        .signal_strength
        .filter(|value| value.is_finite() && *value > 0.0)
        .map(|value| (value.clamp(0.0, 1.0) * 1_000_000.0).round() as u32)
        .unwrap_or(0);
    let exact_rank = if contact.signal_strength.is_none() {
        0
    } else {
        1
    };
    (
        exact_rank,
        std::cmp::Reverse(signal_rank),
        contact.entity_id.clone(),
    )
}

fn enforce_contact_budget(contacts: &mut HashMap<String, TacticalContact>, max_contacts: u16) {
    let max_contacts = usize::from(max_contacts.max(1));
    if contacts.len() <= max_contacts {
        return;
    }
    let mut ranked = contacts.values().collect::<Vec<_>>();
    ranked.sort_by_key(|contact| contact_budget_rank(contact));
    let retained = ranked
        .into_iter()
        .take(max_contacts)
        .map(|contact| contact.entity_id.clone())
        .collect::<HashSet<_>>();
    contacts.retain(|entity_id, _| retained.contains(entity_id));
}

#[allow(clippy::type_complexity)]
#[allow(clippy::too_many_arguments)]
pub fn stream_tactical_snapshot_messages(
    mut commands: Commands<'_, '_>,
    server_query: Query<'_, '_, &'_ Server>,
    mut sender: ServerMultiMessageSender<'_, '_, With<Connected>>,
    time: Res<'_, Time<Real>>,
    mut stream_state: ResMut<'_, TacticalStreamState>,
    mut notification_queue: ResMut<'_, NotificationCommandQueue>,
    bindings: Res<'_, AuthenticatedClientBindings>,
    client_remotes: Query<'_, '_, (&'_ LinkOf, &'_ RemoteId), With<ClientOf>>,
    player_entities: Res<'_, PlayerRuntimeEntityMap>,
    controlled_entity_map: Res<'_, PlayerControlledEntityMap>,
    mut player_visibility: Query<
        '_,
        '_,
        (
            Option<&'_ ContactResolutionM>,
            Option<&'_ mut PlayerExploredCells>,
        ),
        With<PlayerTag>,
    >,
    controlled_scanner_sources: ControlledScannerSourceQuery<'_, '_>,
    mounted_scanner_sources: MountedScannerSourceQuery<'_, '_>,
    replicated_entities: Query<
        '_,
        '_,
        (
            Option<&'_ EntityGuid>,
            Option<&'_ EntityLabels>,
            Option<&'_ FactionId>,
            Option<&'_ MapIcon>,
            Option<&'_ Position>,
            Option<&'_ WorldPosition>,
            Option<&'_ GlobalTransform>,
            Option<&'_ Rotation>,
            Option<&'_ LinearVelocity>,
            Option<&'_ SizeM>,
            Option<&'_ TotalMassKg>,
            Option<&'_ StaticLandmark>,
            Option<&'_ SignalSignature>,
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
        let Some(player_id) = PlayerEntityId::parse(player_entity_id.as_str()) else {
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
        let Ok((contact_resolution, explored_component)) = player_visibility.get_mut(player_entity)
        else {
            continue;
        };
        let contact_resolution_m = contact_resolution
            .map(|value| value.0)
            .unwrap_or(DEFAULT_CONTACT_RESOLUTION_M)
            .max(1.0);

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
        let scanner_source = resolve_effective_scanner_source(
            &player_id,
            player_entity,
            &controlled_entity_map,
            &controlled_scanner_sources,
            &mounted_scanner_sources,
            contact_resolution_m,
        );
        let scanner_visibility_source =
            scanner_source.map(EffectiveScannerSource::visibility_range_source);
        let live_cells = scanner_visibility_source
            .as_ref()
            .map(|source| {
                build_live_cells_from_visibility_sources(
                    std::slice::from_ref(source),
                    fog_cell_size_m,
                )
            })
            .unwrap_or_default();
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
        if let (Some(scanner_source), Some(scanner_visibility_source)) =
            (scanner_source, scanner_visibility_source)
        {
            for (
                guid,
                labels,
                faction_id,
                map_icon,
                position,
                world_position,
                global_transform,
                rotation,
                linear_velocity,
                size,
                total_mass,
                static_landmark,
                signal_signature,
                player_tag,
                replication_state,
            ) in &replicated_entities
            {
                if player_tag.is_some() {
                    continue;
                }
                let Some(guid) = guid else {
                    continue;
                };
                let world = tactical_world_position(position, world_position, global_transform);
                let extent_m = entity_extent_m(size);
                let entity_id = guid.0.to_string();
                if replication_state.is_visible(*client_entity) {
                    if !scanner_detects_world(scanner_source, world, extent_m) {
                        continue;
                    }
                    let signal_detection = signal_signature.and_then(|signal_signature| {
                        strongest_signal_detection(
                            std::slice::from_ref(&scanner_visibility_source),
                            signal_signature,
                            world,
                            extent_m,
                        )
                    });
                    contacts_by_entity_id.insert(
                        entity_id.clone(),
                        TacticalContact {
                            entity_id: guid.0.to_string(),
                            kind: contact_kind_from_labels(labels),
                            map_icon_asset_id: map_icon.map(|icon| icon.asset_id.clone()),
                            faction_id: faction_id.map(|id| id.0.clone()),
                            position_xy: [world.x, world.y],
                            size_m: size.map(|value| [value.length, value.width, value.height]),
                            mass_kg: total_mass.map(|value| value.0),
                            heading_rad: rotation.map_or(0.0, |value| value.as_radians()),
                            velocity_xy: linear_velocity.map(|v| [v.0.x, v.0.y]),
                            is_live_now: true,
                            last_seen_tick: generated_at_tick,
                            classification: None,
                            contact_quality: signal_detection
                                .map(|(_, quality)| quality.to_string()),
                            signal_strength: signal_detection.map(|(strength, _)| strength),
                            position_accuracy_m: None,
                        },
                    );
                    continue;
                }

                let Some(signal_signature) = signal_signature else {
                    continue;
                };
                let Some((signal_strength, contact_quality)) = strongest_signal_detection(
                    std::slice::from_ref(&scanner_visibility_source),
                    signal_signature,
                    world,
                    extent_m,
                ) else {
                    continue;
                };
                let target_key = guid.0.to_string();
                let contact_memory = state
                    .signal_contacts_by_target
                    .entry(target_key.clone())
                    .or_insert_with(|| SignalContactMemory {
                        contact_id: stable_signal_contact_id(
                            player_entity_id.as_str(),
                            &target_key,
                        ),
                        approximate_position_xy: approximate_signal_contact_position(
                            world,
                            scanner_source.contact_resolution_m,
                        ),
                        position_accuracy_m: scanner_source.contact_resolution_m,
                        strongest_signal_strength: signal_strength,
                        last_detected_tick: generated_at_tick,
                    });
                if should_update_signal_memory(contact_memory, scanner_source.contact_resolution_m)
                {
                    contact_memory.approximate_position_xy = approximate_signal_contact_position(
                        world,
                        scanner_source.contact_resolution_m,
                    );
                    contact_memory.position_accuracy_m = scanner_source.contact_resolution_m;
                }
                contact_memory.strongest_signal_strength = contact_memory
                    .strongest_signal_strength
                    .max(signal_strength);
                contact_memory.last_detected_tick = generated_at_tick;

                let contact = TacticalContact {
                    entity_id: contact_memory.contact_id.clone(),
                    kind: "unknown".to_string(),
                    map_icon_asset_id: Some(UNKNOWN_CONTACT_ICON_ASSET_ID.to_string()),
                    faction_id: None,
                    position_xy: contact_memory.approximate_position_xy,
                    size_m: None,
                    mass_kg: None,
                    heading_rad: 0.0,
                    velocity_xy: None,
                    is_live_now: true,
                    last_seen_tick: generated_at_tick,
                    classification: Some("unknown".to_string()),
                    contact_quality: Some(contact_quality.to_string()),
                    signal_strength: Some(signal_strength),
                    position_accuracy_m: Some(contact_memory.position_accuracy_m),
                };
                if is_gravity_well_landmark(static_landmark)
                    && state
                        .notified_long_range_signal_targets
                        .insert(target_key.clone())
                {
                    enqueue_gravity_well_signal_notification(
                        &mut notification_queue,
                        player_entity_id.as_str(),
                        &contact,
                    );
                }
                contacts_by_entity_id.insert(contact.entity_id.clone(), contact);
            }
            enforce_contact_budget(&mut contacts_by_entity_id, scanner_source.max_contacts);
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
        SignalContactMemory, apply_live_cells_to_explored_memory,
        approximate_signal_contact_position, best_effective_scanner_candidate,
        build_live_cells_from_visibility_sources, circle_intersects_grid_cell,
        compute_contacts_delta, compute_live_cells_delta, enforce_contact_budget,
        ensure_explored_memory_shape, materialize_explored_cells, scanner_candidate,
        scanner_detects_world, scanner_effective_range, should_update_signal_memory,
        strongest_signal_detection,
    };
    use bevy::math::DVec3;
    use sidereal_game::{
        PlayerExploredCells, ScannerComponent, ScannerContactDetailTier, SignalSignature,
        VisibilityRangeM, VisibilityRangeSource,
    };
    use sidereal_net::{GridCell, TacticalContact};
    use std::collections::{HashMap, HashSet};

    fn scanner(
        base_range_m: f32,
        level: u8,
        detail_tier: ScannerContactDetailTier,
        max_contacts: u16,
    ) -> ScannerComponent {
        ScannerComponent {
            base_range_m,
            level,
            detail_tier,
            supports_density: false,
            supports_directional_awareness: false,
            max_contacts,
        }
    }

    fn contact(entity_id: &str, position_xy: [f64; 2], last_seen_tick: u64) -> TacticalContact {
        TacticalContact {
            entity_id: entity_id.to_string(),
            kind: "ship".to_string(),
            map_icon_asset_id: Some("map_icon_ship_svg".to_string()),
            faction_id: None,
            position_xy,
            size_m: None,
            mass_kg: None,
            heading_rad: 0.0,
            velocity_xy: None,
            is_live_now: true,
            last_seen_tick,
            classification: None,
            contact_quality: None,
            signal_strength: None,
            position_accuracy_m: None,
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
    fn signal_detection_reports_quality_inside_extended_range() {
        let source = VisibilityRangeSource {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            range_m: 300.0,
        };
        let signal = SignalSignature {
            strength: 1.0,
            detection_radius_m: 900.0,
            use_extent_for_detection: true,
        };

        let detection =
            strongest_signal_detection(&[source], &signal, DVec3::new(700.0, 0.0, 0.0), 100.0);

        assert!(matches!(detection, Some((strength, "moderate")) if strength > 0.4));
    }

    #[test]
    fn scanner_effective_range_prefers_hot_visibility_range() {
        let scanner = scanner(500.0, 1, ScannerContactDetailTier::Basic, 16);

        assert_eq!(
            scanner_effective_range(&scanner, Some(&VisibilityRangeM(750.0))),
            Some(750.0)
        );
        assert_eq!(scanner_effective_range(&scanner, None), Some(500.0));
        assert_eq!(
            scanner_effective_range(&scanner, Some(&VisibilityRangeM(-1.0))),
            Some(500.0)
        );
    }

    #[test]
    fn best_scanner_candidate_prefers_tier_then_range_then_stable_guid() {
        let base = scanner(500.0, 1, ScannerContactDetailTier::Basic, 16);
        let better_tier = scanner(250.0, 1, ScannerContactDetailTier::Iff, 16);
        let better_range = scanner(900.0, 1, ScannerContactDetailTier::Iff, 16);
        let lower_guid = uuid::Uuid::from_u128(1);
        let higher_guid = uuid::Uuid::from_u128(2);

        let candidate = scanner_candidate(DVec3::ZERO, higher_guid, &base, None, 100.0).unwrap();
        let selected = best_effective_scanner_candidate(None, candidate).unwrap();
        let selected = best_effective_scanner_candidate(
            Some(selected),
            scanner_candidate(DVec3::ZERO, lower_guid, &better_tier, None, 100.0).unwrap(),
        )
        .unwrap();
        assert_eq!(selected.scanner_guid, lower_guid);
        assert_eq!(selected.detail_tier, ScannerContactDetailTier::Iff);

        let selected = best_effective_scanner_candidate(
            Some(selected),
            scanner_candidate(DVec3::ZERO, higher_guid, &better_range, None, 100.0).unwrap(),
        )
        .unwrap();
        assert_eq!(selected.scanner_guid, higher_guid);
        assert_eq!(selected.range_m, 900.0);

        let selected = best_effective_scanner_candidate(
            Some(selected),
            scanner_candidate(DVec3::ZERO, lower_guid, &better_range, None, 100.0).unwrap(),
        )
        .unwrap();
        assert_eq!(selected.scanner_guid, lower_guid);
    }

    #[test]
    fn scanner_detection_uses_target_extent() {
        let source = scanner_candidate(
            DVec3::ZERO,
            uuid::Uuid::from_u128(1),
            &scanner(100.0, 1, ScannerContactDetailTier::Basic, 16),
            None,
            100.0,
        )
        .unwrap();

        assert!(!scanner_detects_world(
            source,
            DVec3::new(125.0, 0.0, 0.0),
            0.0
        ));
        assert!(scanner_detects_world(
            source,
            DVec3::new(125.0, 0.0, 0.0),
            30.0
        ));
    }

    #[test]
    fn contact_budget_prefers_exact_then_stronger_signal_deterministically() {
        let mut contacts = HashMap::new();
        contacts.insert("exact-b".to_string(), contact("exact-b", [0.0, 0.0], 1));
        contacts.insert("exact-a".to_string(), contact("exact-a", [0.0, 0.0], 1));
        let mut weak_signal = contact("signal-weak", [0.0, 0.0], 1);
        weak_signal.signal_strength = Some(0.2);
        let mut strong_signal = contact("signal-strong", [0.0, 0.0], 1);
        strong_signal.signal_strength = Some(0.9);
        contacts.insert(weak_signal.entity_id.clone(), weak_signal);
        contacts.insert(strong_signal.entity_id.clone(), strong_signal);

        enforce_contact_budget(&mut contacts, 3);

        let mut retained = contacts.keys().cloned().collect::<Vec<_>>();
        retained.sort();
        assert_eq!(
            retained,
            vec![
                "exact-a".to_string(),
                "exact-b".to_string(),
                "signal-strong".to_string()
            ]
        );
    }

    #[test]
    fn signal_contact_position_memory_only_improves_with_better_resolution() {
        let position = approximate_signal_contact_position(DVec3::new(124.0, 276.0, 0.0), 100.0);
        assert_eq!(position, [150.0, 250.0]);

        let memory = SignalContactMemory {
            contact_id: "signal-test".to_string(),
            approximate_position_xy: position,
            position_accuracy_m: 100.0,
            strongest_signal_strength: 0.5,
            last_detected_tick: 1,
        };

        assert!(!should_update_signal_memory(&memory, 80.0));
        assert!(should_update_signal_memory(&memory, 70.0));
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
