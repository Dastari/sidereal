use avian2d::prelude::Position;
use bevy::prelude::*;
use lightyear::prelude::server::ClientOf;
use lightyear::prelude::{
    ControlledBy, MessageReceiver, NetworkVisibility, Replicate, ReplicationState,
};
use sidereal_game::{
    EntityGuid, FactionId, FactionVisibility, MountedOn, OwnerId, ParentGuid, PlayerTag,
    PublicVisibility, ScannerComponent, ScannerRangeM, ShipTag, VisibilityDisclosure,
    VisibilityGridCell, VisibilityScannerSource, VisibilitySpatialGrid,
};
use sidereal_net::{ClientLocalViewMode, ClientLocalViewModeMessage, PlayerEntityId};
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;
use std::time::Instant;

use crate::replication::PlayerRuntimeEntityMap;
use crate::replication::auth::AuthenticatedClientBindings;
use crate::replication::debug_env;

pub const DEFAULT_VIEW_RANGE_M: f32 = 300.0;
const DEFAULT_VISIBILITY_CELL_SIZE_M: f32 = 2000.0;

fn canonical_player_entity_id(id: &str) -> String {
    sidereal_net::PlayerEntityId::parse(id)
        .map(sidereal_net::PlayerEntityId::canonical_wire_id)
        .unwrap_or_else(|| id.to_string())
}

fn player_entity_ids_match(left: &str, right: &str) -> bool {
    if left == right {
        return true;
    }
    let left_canonical = canonical_player_entity_id(left);
    let right_canonical = canonical_player_entity_id(right);
    if left_canonical == right_canonical {
        return true;
    }
    sidereal_runtime_sync::parse_guid_from_entity_id(left)
        .zip(sidereal_runtime_sync::parse_guid_from_entity_id(right))
        .is_some_and(|(l, r)| l == r)
}

fn parse_delivery_range_m(raw: Option<&str>) -> Option<f32> {
    raw.and_then(|value| value.parse::<f32>().ok())
        .filter(|value| value.is_finite() && *value > 0.0)
}

fn delivery_range_m_from_env() -> f32 {
    parse_delivery_range_m(
        std::env::var("SIDEREAL_VISIBILITY_DELIVERY_RANGE_M")
            .ok()
            .as_deref(),
    )
    .unwrap_or(DEFAULT_VIEW_RANGE_M)
}

fn parse_cell_size_m(raw: Option<&str>) -> Option<f32> {
    raw.and_then(|value| value.parse::<f32>().ok())
        .filter(|value| value.is_finite() && *value >= 50.0)
}

fn cell_size_m_from_env() -> f32 {
    parse_cell_size_m(
        std::env::var("SIDEREAL_VISIBILITY_CELL_SIZE_M")
            .ok()
            .as_deref(),
    )
    .unwrap_or(DEFAULT_VISIBILITY_CELL_SIZE_M)
}

fn bypass_all_visibility_filters_from_env() -> bool {
    std::env::var("SIDEREAL_VISIBILITY_BYPASS_ALL")
        .ok()
        .is_some_and(|raw| {
            let normalized = raw.trim().to_ascii_lowercase();
            normalized == "1" || normalized == "true" || normalized == "on"
        })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VisibilityCandidateMode {
    FullScan,
    SpatialGrid,
}

impl VisibilityCandidateMode {
    fn from_raw(raw: Option<&str>) -> Self {
        match raw
            .unwrap_or("spatial_grid")
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "full" | "full_scan" => Self::FullScan,
            _ => Self::SpatialGrid,
        }
    }

    fn from_env() -> Self {
        Self::from_raw(
            std::env::var("SIDEREAL_VISIBILITY_CANDIDATE_MODE")
                .ok()
                .as_deref(),
        )
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::FullScan => "full_scan",
            Self::SpatialGrid => "spatial_grid",
        }
    }
}

#[derive(Resource, Default)]
pub struct ClientVisibilityRegistry {
    pub player_entity_id_by_client: HashMap<Entity, String>,
}

impl ClientVisibilityRegistry {
    pub fn register_client(&mut self, client_entity: Entity, player_entity_id: String) {
        self.player_entity_id_by_client
            .insert(client_entity, player_entity_id);
    }

    pub fn unregister_client(&mut self, client_entity: Entity) {
        self.player_entity_id_by_client.remove(&client_entity);
    }
}

/// Tracks position of each player's observer anchor entity for spatial queries.
#[derive(Resource, Default)]
pub struct ClientObserverAnchorPositionMap {
    pub position_by_player_entity_id: HashMap<String, Vec3>,
}

impl ClientObserverAnchorPositionMap {
    pub fn update_position(&mut self, player_entity_id: &str, position: Vec3) {
        self.position_by_player_entity_id
            .insert(player_entity_id.to_string(), position);
    }

    pub fn get_position(&self, player_entity_id: &str) -> Option<Vec3> {
        self.position_by_player_entity_id
            .get(player_entity_id)
            .copied()
    }
}

#[derive(Debug, Clone)]
pub(crate) struct PlayerVisibilityContext {
    pub player_entity_id: String,
    pub observer_anchor_position: Option<Vec3>,
    pub scanner_sources: Vec<(Vec3, f32)>,
    pub player_faction_id: Option<String>,
    pub view_mode: ClientLocalViewMode,
}

#[derive(Resource, Default)]
pub struct VisibilityScratch {
    live_clients: Vec<Entity>,
    live_client_set: HashSet<Entity>,
    registered_clients: Vec<(Entity, String)>,
    all_replicated_entities: Vec<Entity>,
    /// All replicated entities by GUID (roots and mounted children) for mount-chain resolution.
    entity_by_guid: HashMap<uuid::Uuid, Entity>,
    /// World position from GlobalTransform for every replicated entity.
    world_position_by_entity: HashMap<Entity, Vec3>,
    /// Effective visibility position used by candidate/auth/delivery checks.
    /// For mounted entities this is inherited from their mount root.
    visibility_position_by_entity: HashMap<Entity, Vec3>,
    /// Parent entity in mount chain (MountedOn.parent_entity_id -> entity). Used to resolve root.
    parent_entity_by_entity: HashMap<Entity, Entity>,
    /// Mount root entity for inheritance (owner/public/faction). Resolved by traversing MountedOn.
    root_entity_by_entity: HashMap<Entity, Entity>,
    root_public_by_entity: HashMap<Entity, bool>,
    root_owner_by_entity: HashMap<Entity, String>,
    root_faction_by_entity: HashMap<Entity, String>,
    scanner_sources_by_owner: HashMap<String, Vec<(Vec3, f32)>>,
    player_faction_by_owner: HashMap<String, String>,
    context_by_client: HashMap<Entity, PlayerVisibilityContext>,
    entities_by_cell: HashMap<(i64, i64), Vec<Entity>>,
    owned_entities_by_player: HashMap<String, Vec<Entity>>,
    candidate_entities_by_client: HashMap<Entity, HashSet<Entity>>,
    candidate_cells_by_client: HashMap<Entity, HashSet<(i64, i64)>>,
}

#[derive(Resource)]
pub(crate) struct VisibilityRuntimeConfig {
    candidate_mode: VisibilityCandidateMode,
    delivery_range_m: f32,
    cell_size_m: f32,
    bypass_all_filters: bool,
}

#[derive(Resource, Default)]
pub struct VisibilityTelemetryLogState {
    pub last_logged_at_s: f64,
}

#[derive(Resource, Default)]
pub struct ClientLocalViewModeRegistry {
    pub by_client_entity: HashMap<Entity, ClientLocalViewMode>,
}

pub fn init_resources(app: &mut App) {
    let candidate_mode = VisibilityCandidateMode::from_env();
    let delivery_range_m = delivery_range_m_from_env();
    let cell_size_m = cell_size_m_from_env();
    if delivery_range_m > cell_size_m * 4.0 {
        let cell_radius = (delivery_range_m / cell_size_m).ceil() as i64;
        let cells_per_axis = cell_radius * 2 + 1;
        warn!(
            "delivery_range_m ({:.0}) is large relative to cell_size_m ({:.0}); grid queries will iterate {} cells per axis per query. Consider increasing SIDEREAL_VISIBILITY_CELL_SIZE_M.",
            delivery_range_m, cell_size_m, cells_per_axis
        );
    }

    app.insert_resource(ClientVisibilityRegistry::default());
    app.insert_resource(VisibilityScratch::default());
    app.insert_resource(ClientObserverAnchorPositionMap::default());
    app.insert_resource(VisibilityRuntimeConfig {
        candidate_mode,
        delivery_range_m,
        cell_size_m,
        bypass_all_filters: bypass_all_visibility_filters_from_env(),
    });
    app.insert_resource(VisibilityTelemetryLogState::default());
    app.insert_resource(ClientLocalViewModeRegistry::default());
}

#[allow(clippy::type_complexity)]
pub fn receive_client_local_view_mode_messages(
    mut receivers: Query<
        '_,
        '_,
        (Entity, &'_ mut MessageReceiver<ClientLocalViewModeMessage>),
        With<ClientOf>,
    >,
    bindings: Res<'_, AuthenticatedClientBindings>,
    mut registry: ResMut<'_, ClientLocalViewModeRegistry>,
) {
    for (client_entity, mut receiver) in &mut receivers {
        for message in receiver.receive() {
            let Some(bound_player_id) = bindings.by_client_entity.get(&client_entity) else {
                continue;
            };
            let Some(bound_player_id) = PlayerEntityId::parse(bound_player_id.as_str()) else {
                continue;
            };
            let Some(message_player_id) = PlayerEntityId::parse(message.player_entity_id.as_str())
            else {
                continue;
            };
            if bound_player_id != message_player_id {
                continue;
            }
            registry
                .by_client_entity
                .insert(client_entity, message.view_mode);
        }
    }
}

pub fn ensure_network_visibility_for_replicated_entities(
    mut commands: Commands<'_, '_>,
    query: Query<'_, '_, Entity, (With<Replicate>, Without<NetworkVisibility>)>,
) {
    for entity in &query {
        commands.entity(entity).insert(NetworkVisibility);
    }
}

impl VisibilityScratch {
    fn clear(&mut self) {
        self.live_clients.clear();
        self.live_client_set.clear();
        self.registered_clients.clear();
        self.all_replicated_entities.clear();
        self.entity_by_guid.clear();
        self.world_position_by_entity.clear();
        self.visibility_position_by_entity.clear();
        self.parent_entity_by_entity.clear();
        self.root_entity_by_entity.clear();
        self.root_public_by_entity.clear();
        self.root_owner_by_entity.clear();
        self.root_faction_by_entity.clear();
        self.scanner_sources_by_owner.clear();
        self.player_faction_by_owner.clear();
        self.context_by_client.clear();
        self.entities_by_cell.clear();
        self.owned_entities_by_player.clear();
        self.candidate_entities_by_client.clear();
        self.candidate_cells_by_client.clear();
    }
}

fn summary_logging_enabled() -> bool {
    debug_env("SIDEREAL_REPLICATION_SUMMARY_LOGS")
}

fn debug_visibility_entity_guid() -> Option<uuid::Uuid> {
    static GUID: OnceLock<Option<uuid::Uuid>> = OnceLock::new();
    *GUID.get_or_init(|| {
        std::env::var("SIDEREAL_DEBUG_VIS_ENTITY_GUID")
            .ok()
            .and_then(|raw| uuid::Uuid::parse_str(raw.trim()).ok())
    })
}

fn cell_key(position: Vec3, cell_size_m: f32) -> (i64, i64) {
    (
        (position.x / cell_size_m).floor() as i64,
        (position.y / cell_size_m).floor() as i64,
    )
}

fn add_entities_in_radius(
    center: Vec3,
    radius_m: f32,
    cell_size_m: f32,
    entities_by_cell: &HashMap<(i64, i64), Vec<Entity>>,
    out: &mut HashSet<Entity>,
) {
    let radius = radius_m.max(0.0);
    let cell_radius = (radius / cell_size_m).ceil() as i64;
    let (cx, cy) = cell_key(center, cell_size_m);
    for dx in -cell_radius..=cell_radius {
        for dy in -cell_radius..=cell_radius {
            if let Some(entities) = entities_by_cell.get(&(cx + dx, cy + dy)) {
                out.extend(entities.iter().copied());
            }
        }
    }
}

fn add_cell_keys_in_radius(
    center: Vec3,
    radius_m: f32,
    cell_size_m: f32,
    out: &mut HashSet<(i64, i64)>,
) {
    let radius = radius_m.max(0.0);
    let cell_radius = (radius / cell_size_m).ceil() as i64;
    let (cx, cy) = cell_key(center, cell_size_m);
    for dx in -cell_radius..=cell_radius {
        for dy in -cell_radius..=cell_radius {
            out.insert((cx + dx, cy + dy));
        }
    }
}

fn build_candidate_cells_for_client(
    candidate_mode: VisibilityCandidateMode,
    observer_anchor_position: Option<Vec3>,
    observer_delivery_range_m: f32,
    scanner_sources: &[(Vec3, f32)],
    view_mode: ClientLocalViewMode,
    cell_size_m: f32,
) -> HashSet<(i64, i64)> {
    match candidate_mode {
        VisibilityCandidateMode::FullScan => HashSet::new(),
        VisibilityCandidateMode::SpatialGrid => {
            let mut cells = HashSet::new();
            if let Some(observer_anchor) = observer_anchor_position {
                add_cell_keys_in_radius(
                    observer_anchor,
                    observer_delivery_range_m,
                    cell_size_m,
                    &mut cells,
                );
            }
            if matches!(view_mode, ClientLocalViewMode::Map) {
                for (scanner_pos, scanner_range) in scanner_sources {
                    add_cell_keys_in_radius(*scanner_pos, *scanner_range, cell_size_m, &mut cells);
                }
            }
            cells
        }
    }
}

fn build_candidate_set_for_client(
    candidate_mode: VisibilityCandidateMode,
    player_entity_id: &str,
    observer_anchor_position: Option<Vec3>,
    observer_delivery_range_m: f32,
    scanner_sources: &[(Vec3, f32)],
    view_mode: ClientLocalViewMode,
    cell_size_m: f32,
    scratch: &VisibilityScratch,
) -> HashSet<Entity> {
    match candidate_mode {
        VisibilityCandidateMode::FullScan => {
            let mut all = HashSet::with_capacity(scratch.all_replicated_entities.len());
            all.extend(scratch.all_replicated_entities.iter().copied());
            all
        }
        VisibilityCandidateMode::SpatialGrid => {
            let mut candidates = HashSet::new();
            if matches!(view_mode, ClientLocalViewMode::Map)
                && let Some(owned_entities) = scratch.owned_entities_by_player.get(player_entity_id)
            {
                candidates.extend(owned_entities.iter().copied());
            }
            if let Some(observer_anchor) = observer_anchor_position {
                add_entities_in_radius(
                    observer_anchor,
                    observer_delivery_range_m,
                    cell_size_m,
                    &scratch.entities_by_cell,
                    &mut candidates,
                );
            }
            if matches!(view_mode, ClientLocalViewMode::Map) {
                for (scanner_pos, scanner_range) in scanner_sources {
                    add_entities_in_radius(
                        *scanner_pos,
                        *scanner_range,
                        cell_size_m,
                        &scratch.entities_by_cell,
                        &mut candidates,
                    );
                }
            }
            candidates
        }
    }
}

pub(crate) fn should_bypass_candidate_filter(
    player_entity_id: &str,
    owner_player_id: Option<&str>,
    is_public_visibility: bool,
    is_faction_visibility: bool,
    entity_faction_id: Option<&str>,
    entity_position: Option<Vec3>,
    visibility_context: &PlayerVisibilityContext,
) -> bool {
    if owner_player_id.is_some_and(|owner| owner == player_entity_id) {
        return true;
    }
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
    let Some(target_position) = entity_position else {
        return false;
    };
    visibility_context
        .scanner_sources
        .iter()
        .any(|(scanner_pos, scanner_range_m)| {
            (target_position - *scanner_pos).length() <= *scanner_range_m
        })
}

#[allow(clippy::type_complexity)]
#[allow(clippy::too_many_arguments)]
pub fn update_network_visibility(
    mut commands: Commands<'_, '_>,
    time: Res<'_, Time>,
    runtime_cfg: Res<'_, VisibilityRuntimeConfig>,
    mut telemetry_state: ResMut<'_, VisibilityTelemetryLogState>,
    clients: Query<'_, '_, Entity, With<ClientOf>>,
    visibility_registry: Res<'_, ClientVisibilityRegistry>,
    mut view_mode_registry: ResMut<'_, ClientLocalViewModeRegistry>,
    player_entities: Res<'_, PlayerRuntimeEntityMap>,
    mut scratch: ResMut<'_, VisibilityScratch>,
    observer_anchor_positions: Res<'_, ClientObserverAnchorPositionMap>,
    player_visibility_state: Query<
        '_,
        '_,
        (
            Option<&'_ VisibilitySpatialGrid>,
            Option<&'_ VisibilityDisclosure>,
        ),
        With<PlayerTag>,
    >,
    all_replicated: Query<
        '_,
        '_,
        (
            Entity,
            Option<&'_ Position>,
            &'_ GlobalTransform,
            Option<&'_ EntityGuid>,
            Option<&'_ OwnerId>,
            Option<&'_ ShipTag>,
            Option<&'_ ScannerComponent>,
            Option<&'_ ScannerRangeM>,
            Option<&'_ PublicVisibility>,
            Option<&'_ FactionVisibility>,
            Option<&'_ FactionId>,
            Option<&'_ MountedOn>,
        ),
        With<Replicate>,
    >,
    parent_links: Query<
        '_,
        '_,
        (Entity, Option<&'_ MountedOn>, Option<&'_ ParentGuid>),
        With<Replicate>,
    >,
    mut replicated_entities: Query<
        '_,
        '_,
        (
            Entity,
            &'_ mut ReplicationState,
            Option<&'_ ControlledBy>,
            Option<&'_ EntityGuid>,
            Option<&'_ PlayerTag>,
            Option<&'_ OwnerId>,
            Option<&'_ PublicVisibility>,
            Option<&'_ FactionVisibility>,
            Option<&'_ FactionId>,
            Option<&'_ MountedOn>,
        ),
        With<Replicate>,
    >,
) {
    let started_at = Instant::now();
    scratch.clear();
    scratch.live_clients.extend(clients.iter());
    let live_clients_snapshot = scratch.live_clients.clone();
    scratch.live_client_set.extend(live_clients_snapshot);
    view_mode_registry
        .by_client_entity
        .retain(|client, _| scratch.live_client_set.contains(client));

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

    // 1) Build entity_by_guid and world position from GlobalTransform for all replicated entities.
    for (
        entity,
        position,
        global_transform,
        entity_guid,
        owner_id,
        _ship_tag,
        _scanner_component,
        _scanner_range,
        public_visibility,
        _faction_visibility,
        faction_id,
        mounted_on,
    ) in &all_replicated
    {
        scratch.all_replicated_entities.push(entity);
        // Contract: all visibility range/delivery checks are world-space; prefer GlobalTransform.
        let world_pos = global_transform.translation();
        let effective_world_pos = if world_pos.is_finite() {
            world_pos
        } else {
            // Defensive fallback only; GlobalTransform should be authoritative.
            position.map(|p| p.0.extend(0.0)).unwrap_or(Vec3::ZERO)
        };
        scratch
            .world_position_by_entity
            .insert(entity, effective_world_pos);
        scratch
            .entities_by_cell
            .entry(cell_key(effective_world_pos, runtime_cfg.cell_size_m))
            .or_default()
            .push(entity);
        if let Some(guid) = entity_guid {
            scratch.entity_by_guid.insert(guid.0, entity);
        }
        scratch
            .root_public_by_entity
            .insert(entity, public_visibility.is_some());
        if let Some(faction) = faction_id {
            scratch
                .root_faction_by_entity
                .insert(entity, faction.0.clone());
        }
        if let Some(owner) = owner_id {
            let canonical_owner = canonical_player_entity_id(owner.0.as_str());
            scratch
                .root_owner_by_entity
                .insert(entity, canonical_owner.clone());
            scratch
                .owned_entities_by_player
                .entry(canonical_owner.clone())
                .or_default()
                .push(entity);
            if let Some(faction) = faction_id {
                scratch
                    .player_faction_by_owner
                    .entry(canonical_owner)
                    .or_insert_with(|| faction.0.clone());
            }
        }
        let _ = mounted_on;
    }

    // 2) Build parent map (entity -> parent entity) for entities with MountedOn.
    for (entity, mounted_on, parent_guid) in &parent_links {
        let parent_guid = mounted_on
            .map(|mounted| mounted.parent_entity_id)
            .or_else(|| parent_guid.map(|parent| parent.0));
        let Some(parent_guid) = parent_guid else {
            continue;
        };
        let Some(&parent_entity) = scratch.entity_by_guid.get(&parent_guid) else {
            continue;
        };
        if parent_entity != entity {
            scratch
                .parent_entity_by_entity
                .insert(entity, parent_entity);
        }
    }

    // 3) Resolve mount root for each entity (traverse parent chain).
    for (entity, _, _, _, _, _, _, _, _, _, _, _) in &all_replicated {
        let root = resolve_mount_root(entity, &scratch.parent_entity_by_entity);
        scratch.root_entity_by_entity.insert(entity, root);
    }

    // 3b) Build effective visibility positions and spatial index.
    // Mounted children inherit root world position for visibility checks to avoid
    // false positives from unhydrated child transforms at origin.
    scratch.entities_by_cell.clear();
    for (entity, _, _, _, _, _, _, _, _, _, _, _) in &all_replicated {
        let root = scratch
            .root_entity_by_entity
            .get(&entity)
            .copied()
            .unwrap_or(entity);
        let effective = scratch
            .world_position_by_entity
            .get(&root)
            .copied()
            .or_else(|| scratch.world_position_by_entity.get(&entity).copied())
            .unwrap_or(Vec3::ZERO);
        scratch
            .visibility_position_by_entity
            .insert(entity, effective);
        scratch
            .entities_by_cell
            .entry(cell_key(effective, runtime_cfg.cell_size_m))
            .or_default()
            .push(entity);
    }

    // 4) Build scanner sources from scanner-capable owned roots only.
    // Scanner-capable = ShipTag baseline scanner or explicit ScannerComponent on the root.
    // Child entities contribute via root ScannerRangeM aggregation; they are not sources.
    for (
        entity,
        _position,
        _global,
        _guid,
        owner_id,
        ship_tag,
        scanner_component,
        scanner_range,
        _public,
        _faction_vis,
        _faction_id,
        _mounted_on,
    ) in &all_replicated
    {
        let is_root = scratch
            .root_entity_by_entity
            .get(&entity)
            .is_some_and(|root| *root == entity);
        if !is_root {
            continue;
        }
        if ship_tag.is_none() && scanner_component.is_none() {
            continue;
        }
        let Some(owner) = owner_id else { continue };
        let Some(range) = scanner_range.map(|r| r.0) else {
            continue;
        };
        if range <= 0.0 {
            continue;
        };
        let canonical_owner = canonical_player_entity_id(owner.0.as_str());
        let Some(position) = scratch.world_position_by_entity.get(&entity).copied() else {
            continue;
        };
        scratch
            .scanner_sources_by_owner
            .entry(canonical_owner)
            .or_default()
            .push((position, range));
    }

    let registered_clients = scratch.registered_clients.clone();
    for (client_entity, player_entity_id) in &registered_clients {
        let canonical_player_id = canonical_player_entity_id(player_entity_id.as_str());
        let scanner_sources = scratch
            .scanner_sources_by_owner
            .get(canonical_player_id.as_str())
            .cloned()
            .unwrap_or_default();
        let observer_anchor_position = observer_anchor_positions
            .get_position(canonical_player_id.as_str())
            .or_else(|| observer_anchor_positions.get_position(player_entity_id.as_str()));
        let player_faction_id = scratch
            .player_faction_by_owner
            .get(canonical_player_id.as_str())
            .cloned();
        let local_view_mode = view_mode_registry
            .by_client_entity
            .get(client_entity)
            .copied()
            .unwrap_or(ClientLocalViewMode::Tactical);
        scratch.context_by_client.insert(
            *client_entity,
            PlayerVisibilityContext {
                player_entity_id: canonical_player_id.clone(),
                observer_anchor_position,
                scanner_sources: scanner_sources.clone(),
                player_faction_id,
                view_mode: local_view_mode,
            },
        );

        let candidates = build_candidate_set_for_client(
            runtime_cfg.candidate_mode,
            canonical_player_id.as_str(),
            observer_anchor_position,
            runtime_cfg.delivery_range_m,
            &scanner_sources,
            local_view_mode,
            runtime_cfg.cell_size_m,
            &scratch,
        );
        let candidate_cells = build_candidate_cells_for_client(
            runtime_cfg.candidate_mode,
            observer_anchor_position,
            runtime_cfg.delivery_range_m,
            &scanner_sources,
            local_view_mode,
            runtime_cfg.cell_size_m,
        );
        scratch
            .candidate_entities_by_client
            .insert(*client_entity, candidates);
        scratch
            .candidate_cells_by_client
            .insert(*client_entity, candidate_cells);
    }

    for (client_entity, player_entity_id) in &registered_clients {
        let canonical_player_id = canonical_player_entity_id(player_entity_id.as_str());
        let Some(&player_entity) = player_entities
            .by_player_entity_id
            .get(canonical_player_id.as_str())
            .or_else(|| {
                player_entities
                    .by_player_entity_id
                    .get(player_entity_id.as_str())
            })
        else {
            continue;
        };
        let Some(scanner_sources) = scratch.context_by_client.get(client_entity).map(|ctx| {
            ctx.scanner_sources
                .iter()
                .map(|(position, range_m)| VisibilityScannerSource {
                    x: position.x,
                    y: position.y,
                    z: position.z,
                    range_m: *range_m,
                })
                .collect::<Vec<_>>()
        }) else {
            continue;
        };
        let mut queried_cells = scratch
            .candidate_cells_by_client
            .get(client_entity)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|(x, y)| VisibilityGridCell { x, y })
            .collect::<Vec<_>>();
        queried_cells.sort_by_key(|cell| (cell.x, cell.y));

        let next_grid = VisibilitySpatialGrid {
            candidate_mode: runtime_cfg.candidate_mode.as_str().to_string(),
            cell_size_m: runtime_cfg.cell_size_m,
            delivery_range_m: runtime_cfg.delivery_range_m,
            queried_cells,
        };
        let next_disclosure = VisibilityDisclosure { scanner_sources };

        let Ok((existing_grid, existing_disclosure)) = player_visibility_state.get(player_entity)
        else {
            continue;
        };
        let mut entity_commands = commands.entity(player_entity);
        if existing_grid.is_none_or(|current| current != &next_grid) {
            entity_commands.insert(next_grid);
        }
        if existing_disclosure.is_none_or(|current| current != &next_disclosure) {
            entity_commands.insert(next_disclosure);
        }
    }

    for (
        entity,
        mut replication_state,
        controlled_by,
        entity_guid,
        player_tag,
        owner_id,
        public_visibility,
        faction_visibility,
        faction_id,
        _mounted_on,
    ) in &mut replicated_entities
    {
        let tracked_guid = entity_guid.map(|guid| guid.0);
        let debug_track_this_entity =
            debug_visibility_entity_guid().is_some_and(|tracked| Some(tracked) == tracked_guid);
        let root_entity = scratch
            .root_entity_by_entity
            .get(&entity)
            .copied()
            .unwrap_or(entity);

        // Use world position from GlobalTransform (same as all_replicated); fallback from scratch.
        let entity_position = scratch.visibility_position_by_entity.get(&entity).copied();
        let is_public = public_visibility.is_some()
            || scratch
                .root_public_by_entity
                .get(&root_entity)
                .copied()
                .unwrap_or(false);
        let owner_player_id = owner_id
            .map(|owner| canonical_player_entity_id(owner.0.as_str()))
            .or_else(|| scratch.root_owner_by_entity.get(&root_entity).cloned());
        // Ensure players always receive replication for their own observer/player entity
        // even in valid no-ship states.
        let owner_player_id_owned = if owner_player_id.is_none() && player_tag.is_some() {
            entity_guid.map(|guid| guid.0.to_string())
        } else {
            None
        };
        let owner_player_id = owner_player_id
            .as_deref()
            .or(owner_player_id_owned.as_deref());
        let entity_faction_id = faction_id.map(|faction| faction.0.as_str()).or_else(|| {
            scratch
                .root_faction_by_entity
                .get(&root_entity)
                .map(String::as_str)
        });
        let is_faction_visible = faction_visibility.is_some();

        // Player anchor entities are strictly owner-only: never replicate them to
        // non-owner clients regardless of candidate mode, range, or bypass settings.
        if player_tag.is_some() {
            for client_entity in &scratch.live_clients {
                let is_owner = scratch
                    .context_by_client
                    .get(client_entity)
                    .is_some_and(|ctx| {
                        owner_player_id.is_some_and(|owner_id| {
                            player_entity_ids_match(ctx.player_entity_id.as_str(), owner_id)
                        })
                    });
                if is_owner {
                    replication_state.gain_visibility(*client_entity);
                } else if replication_state.is_visible(*client_entity) {
                    replication_state.lose_visibility(*client_entity);
                }
            }
            continue;
        }

        if runtime_cfg.bypass_all_filters {
            for client_entity in &scratch.live_clients {
                replication_state.gain_visibility(*client_entity);
            }
            continue;
        }

        for client_entity in &scratch.live_clients {
            if controlled_by.is_some_and(|binding| binding.owner == *client_entity) {
                // Hard guarantee: the owning client must always receive state for
                // their currently controlled entity, independent of scanner/range.
                replication_state.gain_visibility(*client_entity);
                continue;
            }
            let Some(candidates) = scratch.candidate_entities_by_client.get(client_entity) else {
                continue;
            };
            let Some(visibility_context) = scratch.context_by_client.get(client_entity) else {
                continue;
            };
            let in_candidates = candidates.contains(&entity);
            let bypass_candidate = should_bypass_candidate_filter(
                visibility_context.player_entity_id.as_str(),
                owner_player_id,
                is_public,
                is_faction_visible,
                entity_faction_id,
                entity_position,
                visibility_context,
            );
            if !in_candidates && !bypass_candidate {
                if replication_state.is_visible(*client_entity) {
                    replication_state.lose_visibility(*client_entity);
                }
                if debug_track_this_entity {
                    info!(
                        "vis-debug guid={} client_entity={:?} player={} in_candidates={} bypass_candidate={} owner={:?} public={} faction_visible={} entity_pos={:?} anchor_pos={:?} result=lose(candidate)",
                        tracked_guid
                            .map(|g| g.to_string())
                            .unwrap_or_else(|| "<none>".to_string()),
                        client_entity,
                        visibility_context.player_entity_id,
                        in_candidates,
                        bypass_candidate,
                        owner_player_id,
                        is_public,
                        is_faction_visible,
                        entity_position,
                        visibility_context.observer_anchor_position,
                    );
                }
                continue;
            }
            let authorization = authorize_visibility(
                visibility_context.player_entity_id.as_str(),
                owner_player_id,
                is_public,
                is_faction_visible,
                entity_faction_id,
                entity_position,
                visibility_context,
            );
            let delivery_ok =
                passes_delivery_scope(
                    entity_position,
                    visibility_context,
                    runtime_cfg.delivery_range_m,
                ) || (matches!(visibility_context.view_mode, ClientLocalViewMode::Map)
                    && matches!(authorization, Some(VisibilityAuthorization::Owner)));
            let should_be_visible = is_entity_visible_to_player(
                visibility_context.player_entity_id.as_str(),
                owner_player_id,
                is_public,
                is_faction_visible,
                entity_faction_id,
                entity_position,
                visibility_context,
                runtime_cfg.delivery_range_m,
                matches!(visibility_context.view_mode, ClientLocalViewMode::Map),
            );
            if should_be_visible {
                replication_state.gain_visibility(*client_entity);
            } else if replication_state.is_visible(*client_entity) {
                replication_state.lose_visibility(*client_entity);
            }
            if debug_track_this_entity {
                info!(
                    "vis-debug guid={} client_entity={:?} player={} in_candidates={} bypass_candidate={} owner={:?} public={} faction_visible={} authorization={:?} delivery_ok={} entity_pos={:?} anchor_pos={:?} currently_visible={} result={}",
                    tracked_guid
                        .map(|g| g.to_string())
                        .unwrap_or_else(|| "<none>".to_string()),
                    client_entity,
                    visibility_context.player_entity_id,
                    in_candidates,
                    bypass_candidate,
                    owner_player_id,
                    is_public,
                    is_faction_visible,
                    authorization,
                    delivery_ok,
                    entity_position,
                    visibility_context.observer_anchor_position,
                    replication_state.is_visible(*client_entity),
                    if should_be_visible {
                        "gain/keep"
                    } else {
                        "lose"
                    }
                );
            }
        }
    }

    if summary_logging_enabled() {
        let now_s = time.elapsed_secs_f64();
        const LOG_INTERVAL_S: f64 = 5.0;
        if now_s - telemetry_state.last_logged_at_s >= LOG_INTERVAL_S {
            telemetry_state.last_logged_at_s = now_s;
            let clients_count = scratch.live_clients.len();
            let entities_count = scratch.all_replicated_entities.len();
            let candidates_total = scratch
                .candidate_entities_by_client
                .values()
                .map(HashSet::len)
                .sum::<usize>();
            let candidates_per_client = if clients_count > 0 {
                candidates_total as f64 / clients_count as f64
            } else {
                0.0
            };
            info!(
                "replication visibility summary mode={} bypass_all={} delivery_range_m={:.1} query_ms={:.2} clients={} entities={} candidates_per_client={:.1}",
                runtime_cfg.candidate_mode.as_str(),
                runtime_cfg.bypass_all_filters,
                runtime_cfg.delivery_range_m,
                started_at.elapsed().as_secs_f64() * 1000.0,
                clients_count,
                entities_count,
                candidates_per_client
            );
        }
    }
}

/// Resolves the mount root entity by traversing the parent chain (MountedOn).
/// The root is used for owner/public/faction inheritance; the entity's own world
/// position is used for distance checks.
fn resolve_mount_root(entity: Entity, parent_entity_by_entity: &HashMap<Entity, Entity>) -> Entity {
    let mut current = entity;
    let mut visited = std::collections::HashSet::new();
    while let Some(&parent) = parent_entity_by_entity.get(&current) {
        if !visited.insert(current) {
            break;
        }
        current = parent;
    }
    current
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn is_entity_visible_to_player(
    player_entity_id: &str,
    owner_player_id: Option<&str>,
    is_public_visibility: bool,
    is_faction_visibility: bool,
    entity_faction_id: Option<&str>,
    entity_position: Option<Vec3>,
    visibility_context: &PlayerVisibilityContext,
    delivery_range_m: f32,
    owner_bypasses_delivery_scope: bool,
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
    if authorization.is_none() {
        return false;
    }

    if owner_bypasses_delivery_scope
        && matches!(authorization, Some(VisibilityAuthorization::Owner))
    {
        return true;
    }

    passes_delivery_scope(entity_position, visibility_context, delivery_range_m)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum VisibilityAuthorization {
    Owner,
    Public,
    Faction,
    Scanner,
}

pub(crate) fn authorize_visibility(
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
    delivery_range_m: f32,
) -> bool {
    let (Some(observer_anchor_position), Some(target_position)) =
        (visibility_context.observer_anchor_position, entity_position)
    else {
        return false;
    };
    (target_position - observer_anchor_position).length() <= delivery_range_m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidate_mode_defaults_to_spatial_grid() {
        assert_eq!(
            VisibilityCandidateMode::from_raw(None),
            VisibilityCandidateMode::SpatialGrid
        );
    }

    #[test]
    fn candidate_mode_parses_full_aliases() {
        assert_eq!(
            VisibilityCandidateMode::from_raw(Some("full_scan")),
            VisibilityCandidateMode::FullScan
        );
        assert_eq!(
            VisibilityCandidateMode::from_raw(Some("full")),
            VisibilityCandidateMode::FullScan
        );
    }

    #[test]
    fn candidate_mode_unknown_values_fall_back_to_spatial_grid() {
        assert_eq!(
            VisibilityCandidateMode::from_raw(Some("grid")),
            VisibilityCandidateMode::SpatialGrid
        );
        assert_eq!(
            VisibilityCandidateMode::from_raw(Some("random")),
            VisibilityCandidateMode::SpatialGrid
        );
    }

    #[test]
    fn parse_cell_size_requires_minimum_and_finite_value() {
        assert_eq!(parse_cell_size_m(Some("49.9")), None);
        assert_eq!(parse_cell_size_m(Some("2000")), Some(2000.0));
        assert_eq!(parse_cell_size_m(Some("NaN")), None);
    }

    #[test]
    fn cell_key_uses_i64_for_large_coordinates() {
        let position = Vec3::new(5.0e12, -5.0e12, 0.0);
        let key = cell_key(position, 2000.0);
        assert!(key.0 > i64::from(i32::MAX));
        assert!(key.1 < i64::from(i32::MIN));
    }

    #[test]
    fn add_entities_in_radius_uses_configured_cell_size() {
        let center = Vec3::new(0.0, 0.0, 0.0);
        let near = Entity::from_raw_u32(1).expect("valid entity id");
        let far = Entity::from_raw_u32(2).expect("valid entity id");
        let mut grid = HashMap::new();
        grid.insert((0_i64, 0_i64), vec![near]);
        grid.insert((2_i64, 0_i64), vec![far]);

        let mut out = HashSet::new();
        add_entities_in_radius(center, 500.0, 1000.0, &grid, &mut out);
        assert!(out.contains(&near));
        assert!(!out.contains(&far));
    }

    #[test]
    fn candidate_set_uses_configured_delivery_range_for_observer_anchor() {
        let observer = Vec3::ZERO;
        let candidate = Entity::from_raw_u32(3).expect("valid entity id");
        let mut scratch = VisibilityScratch::default();
        // Candidate is two cells away on X when cell size is 1000m.
        scratch
            .entities_by_cell
            .insert((2_i64, 0_i64), vec![candidate]);

        let short = build_candidate_set_for_client(
            VisibilityCandidateMode::SpatialGrid,
            "11111111-1111-1111-1111-111111111111",
            Some(observer),
            500.0,
            &[],
            ClientLocalViewMode::Tactical,
            1000.0,
            &scratch,
        );
        let long = build_candidate_set_for_client(
            VisibilityCandidateMode::SpatialGrid,
            "11111111-1111-1111-1111-111111111111",
            Some(observer),
            2500.0,
            &[],
            ClientLocalViewMode::Tactical,
            1000.0,
            &scratch,
        );

        assert!(!short.contains(&candidate));
        assert!(long.contains(&candidate));
    }

    #[test]
    fn candidate_cells_include_observer_region_only() {
        let observer = Vec3::new(0.0, 0.0, 0.0);
        let cells = build_candidate_cells_for_client(
            VisibilityCandidateMode::SpatialGrid,
            Some(observer),
            1000.0,
            &[],
            ClientLocalViewMode::Tactical,
            1000.0,
        );
        assert!(cells.contains(&(0, 0)));
        assert!(cells.contains(&(1, 0)));
        assert!(!cells.contains(&(2, 0)));
    }
}
