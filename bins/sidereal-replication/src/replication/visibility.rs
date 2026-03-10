use avian2d::prelude::Position;
use bevy::prelude::*;
use lightyear::prelude::server::ClientOf;
use lightyear::prelude::{
    ControlledBy, MessageReceiver, NetworkVisibility, Replicate, ReplicationState,
};
use sidereal_game::{
    DiscoveredStaticLandmarks, EntityGuid, FactionId, FactionVisibility, FullscreenLayer,
    MountedOn, OwnerId, ParentGuid, PlayerTag, PublicVisibility, RENDER_DOMAIN_FULLSCREEN,
    RENDER_PHASE_FULLSCREEN_BACKGROUND, RENDER_PHASE_FULLSCREEN_FOREGROUND,
    RuntimeRenderLayerDefinition, RuntimeRenderLayerOverride, RuntimeWorldVisualStack, SizeM,
    StaticLandmark, VisibilityDisclosure, VisibilityGridCell, VisibilityRangeM,
    VisibilityRangeSource, VisibilitySpatialGrid, default_main_world_render_layer,
};
use sidereal_net::{ClientLocalViewMode, ClientLocalViewModeMessage, PlayerEntityId};
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;
use std::time::Instant;

use crate::replication::PlayerRuntimeEntityMap;
use crate::replication::auth::AuthenticatedClientBindings;
use crate::replication::debug_env;
use crate::replication::lifecycle::ClientLastActivity;

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
    if !cfg!(test) {
        return false;
    }
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

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone)]
pub(crate) struct PlayerVisibilityContext {
    pub player_entity_id: String,
    pub observer_anchor_position: Option<Vec3>,
    pub visibility_sources: Vec<(Vec3, f32)>,
    pub discovered_static_landmarks: HashSet<uuid::Uuid>,
    pub player_faction_id: Option<String>,
    pub view_mode: ClientLocalViewMode,
}

impl PlayerVisibilityContext {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn as_ref(&self) -> PlayerVisibilityContextRef<'_> {
        PlayerVisibilityContextRef {
            player_entity_id: self.player_entity_id.as_str(),
            observer_anchor_position: self.observer_anchor_position,
            visibility_sources: self.visibility_sources.as_slice(),
            player_faction_id: self.player_faction_id.as_deref(),
            view_mode: self.view_mode,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct PlayerVisibilityContextRef<'a> {
    pub player_entity_id: &'a str,
    pub observer_anchor_position: Option<Vec3>,
    pub visibility_sources: &'a [(Vec3, f32)],
    pub player_faction_id: Option<&'a str>,
    pub view_mode: ClientLocalViewMode,
}

impl<'a> PlayerVisibilityContextRef<'a> {
    fn from_client_state(client_state: &'a ClientVisibilityComputedState) -> Self {
        Self {
            player_entity_id: client_state.player_entity_id.as_str(),
            observer_anchor_position: client_state.observer_anchor_position,
            visibility_sources: client_state.visibility_sources.as_slice(),
            player_faction_id: client_state.player_faction_id.as_deref(),
            view_mode: client_state.view_mode,
        }
    }
}

#[derive(Debug, Clone)]
struct ClientVisibilityComputedState {
    client_entity: Entity,
    player_entity_id: String,
    player_entity: Option<Entity>,
    observer_anchor_position: Option<Vec3>,
    visibility_sources: Vec<(Vec3, f32)>,
    discovered_static_landmarks: HashSet<uuid::Uuid>,
    player_faction_id: Option<String>,
    view_mode: ClientLocalViewMode,
    delivery_range_m: f32,
    candidate_entities: HashSet<Entity>,
    candidate_cells: HashSet<(i64, i64)>,
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
    /// Effective visibility extent (radius) used by candidate/auth/delivery checks.
    /// For mounted entities this is inherited from their mount root.
    visibility_extent_m_by_entity: HashMap<Entity, f32>,
    /// Parent entity in mount chain (MountedOn.parent_entity_id -> entity). Used to resolve root.
    parent_entity_by_entity: HashMap<Entity, Entity>,
    /// Mount root entity for inheritance (owner/public/faction). Resolved by traversing MountedOn.
    root_entity_by_entity: HashMap<Entity, Entity>,
    root_public_by_entity: HashMap<Entity, bool>,
    root_owner_by_entity: HashMap<Entity, String>,
    root_faction_by_entity: HashMap<Entity, String>,
    pending_world_layer_override_by_entity: HashMap<Entity, String>,
    resolved_world_layer_by_entity: HashMap<Entity, RuntimeRenderLayerDefinition>,
    visibility_source_candidates: Vec<(Entity, String, f32)>,
    visibility_sources_by_owner: HashMap<String, Vec<(Vec3, f32)>>,
    player_faction_by_owner: HashMap<String, String>,
    entities_by_cell: HashMap<(i64, i64), Vec<Entity>>,
    owned_entities_by_player: HashMap<String, Vec<Entity>>,
    static_landmarks_by_entity: HashMap<Entity, (uuid::Uuid, StaticLandmark)>,
    max_static_landmark_discovery_padding_m: f32,
    client_states: Vec<ClientVisibilityComputedState>,
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

#[allow(dead_code)]
#[derive(Debug, Resource, Default, Clone)]
pub struct VisibilityRuntimeMetrics {
    pub query_ms: f64,
    pub scratch_build_ms: f64,
    pub discovery_and_candidate_ms: f64,
    pub disclosure_sync_ms: f64,
    pub apply_ms: f64,
    pub clients: usize,
    pub entities: usize,
    pub candidates_total: usize,
    pub candidates_per_client: f64,
    pub discovered_checks: usize,
    pub discovered_new_total: usize,
    pub delivery_range_min_m: f64,
    pub delivery_range_avg_m: f64,
    pub delivery_range_max_m: f64,
}

#[derive(Resource, Default)]
pub struct ClientLocalViewModeRegistry {
    pub by_client_entity: HashMap<Entity, ClientLocalViewSettings>,
}

#[derive(Debug, Clone, Copy)]
pub struct ClientLocalViewSettings {
    pub view_mode: ClientLocalViewMode,
    pub delivery_range_m: f32,
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
    app.insert_resource(VisibilityRuntimeMetrics::default());
    app.insert_resource(ClientLocalViewModeRegistry::default());
}

#[allow(clippy::type_complexity)]
pub fn receive_client_local_view_mode_messages(
    time: Res<'_, Time<Real>>,
    mut receivers: Query<
        '_,
        '_,
        (Entity, &'_ mut MessageReceiver<ClientLocalViewModeMessage>),
        With<ClientOf>,
    >,
    bindings: Res<'_, AuthenticatedClientBindings>,
    mut last_activity: ResMut<'_, ClientLastActivity>,
    mut registry: ResMut<'_, ClientLocalViewModeRegistry>,
) {
    let now_s = time.elapsed_secs_f64();
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
            last_activity.0.insert(client_entity, now_s);
            registry.by_client_entity.insert(
                client_entity,
                ClientLocalViewSettings {
                    view_mode: message.view_mode,
                    delivery_range_m: message.delivery_range_m.max(1.0),
                },
            );
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
        self.visibility_extent_m_by_entity.clear();
        self.parent_entity_by_entity.clear();
        self.root_entity_by_entity.clear();
        self.root_public_by_entity.clear();
        self.root_owner_by_entity.clear();
        self.root_faction_by_entity.clear();
        self.pending_world_layer_override_by_entity.clear();
        self.resolved_world_layer_by_entity.clear();
        self.visibility_source_candidates.clear();
        self.visibility_sources_by_owner.clear();
        self.player_faction_by_owner.clear();
        self.entities_by_cell.clear();
        self.owned_entities_by_player.clear();
        self.static_landmarks_by_entity.clear();
        self.max_static_landmark_discovery_padding_m = 0.0;
        self.client_states.clear();
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
    visibility_sources: &[(Vec3, f32)],
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
                for (visibility_pos, visibility_range) in visibility_sources {
                    add_cell_keys_in_radius(
                        *visibility_pos,
                        *visibility_range,
                        cell_size_m,
                        &mut cells,
                    );
                }
            }
            cells
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn build_candidate_set_for_client(
    candidate_mode: VisibilityCandidateMode,
    player_entity_id: &str,
    observer_anchor_position: Option<Vec3>,
    observer_delivery_range_m: f32,
    visibility_sources: &[(Vec3, f32)],
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
                for (visibility_pos, visibility_range) in visibility_sources {
                    add_entities_in_radius(
                        *visibility_pos,
                        *visibility_range,
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

#[allow(clippy::too_many_arguments)]
pub(crate) fn should_bypass_candidate_filter(
    player_entity_id: &str,
    owner_player_id: Option<&str>,
    is_public_visibility: bool,
    is_faction_visibility: bool,
    is_discovered_static_landmark: bool,
    entity_faction_id: Option<&str>,
    entity_position: Option<Vec3>,
    entity_extent_m: f32,
    visibility_context: &PlayerVisibilityContextRef<'_>,
) -> bool {
    if owner_player_id.is_some_and(|owner| owner == player_entity_id) {
        return true;
    }
    if is_public_visibility {
        return true;
    }
    if is_discovered_static_landmark {
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
        .visibility_sources
        .iter()
        .any(|(visibility_pos, visibility_range_m)| {
            (target_position - *visibility_pos).length() <= *visibility_range_m + entity_extent_m
        })
}

fn entity_visibility_extent_m(size: Option<&SizeM>) -> f32 {
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

fn runtime_layer_parallax_factor(definition: Option<&RuntimeRenderLayerDefinition>) -> f32 {
    definition
        .and_then(|value| value.parallax_factor)
        .unwrap_or(1.0)
        .clamp(0.01, 4.0)
}

fn discovered_landmark_delivery_range_m(
    base_delivery_range_m: f32,
    resolved_render_layer: Option<&RuntimeRenderLayerDefinition>,
) -> f32 {
    base_delivery_range_m / runtime_layer_parallax_factor(resolved_render_layer)
}

fn runtime_layer_screen_scale_factor(definition: Option<&RuntimeRenderLayerDefinition>) -> f32 {
    definition
        .and_then(|value| value.screen_scale_factor)
        .unwrap_or(1.0)
        .clamp(0.01, 64.0)
}

fn max_visual_scale_multiplier(visual_stack: Option<&RuntimeWorldVisualStack>) -> f32 {
    visual_stack
        .map(|stack| {
            stack.passes.iter().fold(1.0_f32, |max_scale, pass| {
                if !pass.enabled {
                    return max_scale;
                }
                max_scale.max(pass.scale_multiplier.unwrap_or(1.0))
            })
        })
        .unwrap_or(1.0)
}

fn effective_discovered_landmark_extent_m(
    entity_extent_m: f32,
    resolved_render_layer: Option<&RuntimeRenderLayerDefinition>,
    visual_stack: Option<&RuntimeWorldVisualStack>,
) -> f32 {
    entity_extent_m
        * runtime_layer_screen_scale_factor(resolved_render_layer)
        * max_visual_scale_multiplier(visual_stack)
}

fn landmark_discovery_overlap(
    entity_position: Option<Vec3>,
    entity_extent_m: f32,
    static_landmark: &StaticLandmark,
    visibility_context: &PlayerVisibilityContextRef<'_>,
) -> bool {
    if static_landmark.always_known {
        return true;
    }
    if !static_landmark.discoverable {
        return false;
    }
    let Some(target_position) = entity_position else {
        return false;
    };
    visibility_context
        .visibility_sources
        .iter()
        .any(|(visibility_pos, visibility_range_m)| {
            let extra_radius = static_landmark.discovery_radius_m.unwrap_or(0.0).max(0.0)
                + if static_landmark.use_extent_for_discovery {
                    entity_extent_m
                } else {
                    0.0
                };
            (target_position - *visibility_pos).length() <= *visibility_range_m + extra_radius
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
    mut player_landmark_state: Query<
        '_,
        '_,
        Option<&'_ mut DiscoveredStaticLandmarks>,
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
            Option<&'_ VisibilityRangeM>,
            Option<&'_ PublicVisibility>,
            Option<&'_ FactionVisibility>,
            Option<&'_ FactionId>,
            Option<&'_ MountedOn>,
            Option<&'_ SizeM>,
            Option<&'_ RuntimeRenderLayerDefinition>,
            Option<&'_ RuntimeRenderLayerOverride>,
            Option<&'_ StaticLandmark>,
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
            Option<&'_ FullscreenLayer>,
            Option<&'_ RuntimeRenderLayerDefinition>,
            Option<&'_ OwnerId>,
            Option<&'_ PublicVisibility>,
            Option<&'_ FactionVisibility>,
            Option<&'_ FactionId>,
            Option<&'_ MountedOn>,
            Option<&'_ RuntimeWorldVisualStack>,
            Option<&'_ RuntimeRenderLayerOverride>,
            Option<&'_ StaticLandmark>,
        ),
        With<Replicate>,
    >,
) {
    let started_at = Instant::now();
    let mut discovered_checks = 0usize;
    let mut discovered_new_total = 0usize;
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

    let mut runtime_layer_definitions_by_id =
        HashMap::<String, RuntimeRenderLayerDefinition>::new();
    runtime_layer_definitions_by_id.insert(
        sidereal_game::DEFAULT_MAIN_WORLD_LAYER_ID.to_string(),
        default_main_world_render_layer(),
    );

    // 1) Build entity_by_guid and world position from GlobalTransform for all replicated entities.
    for (
        entity,
        position,
        global_transform,
        entity_guid,
        owner_id,
        _visibility_range,
        public_visibility,
        _faction_visibility,
        faction_id,
        mounted_on,
        size,
        runtime_render_layer_definition,
        runtime_render_layer_override,
        _static_landmark,
    ) in &all_replicated
    {
        if let Some(definition) = runtime_render_layer_definition {
            runtime_layer_definitions_by_id.insert(definition.layer_id.clone(), definition.clone());
        }
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
        let entity_extent_m = entity_visibility_extent_m(size);
        scratch
            .visibility_extent_m_by_entity
            .insert(entity, entity_extent_m);
        if let Some(guid) = entity_guid {
            scratch.entity_by_guid.insert(guid.0, entity);
            if let Some(static_landmark) = _static_landmark {
                let discovery_padding_m =
                    static_landmark.discovery_radius_m.unwrap_or(0.0).max(0.0)
                        + if static_landmark.use_extent_for_discovery {
                            entity_extent_m
                        } else {
                            0.0
                        };
                scratch.max_static_landmark_discovery_padding_m = scratch
                    .max_static_landmark_discovery_padding_m
                    .max(discovery_padding_m);
                scratch
                    .static_landmarks_by_entity
                    .insert(entity, (guid.0, static_landmark.clone()));
            }
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
        if let Some(override_layer) = runtime_render_layer_override {
            scratch
                .pending_world_layer_override_by_entity
                .insert(entity, override_layer.layer_id.clone());
        }
        if let (Some(owner), Some(range)) = (owner_id, _visibility_range.map(|r| r.0))
            && range > 0.0
        {
            scratch.visibility_source_candidates.push((
                entity,
                canonical_player_entity_id(owner.0.as_str()),
                range,
            ));
        }
        let _ = mounted_on;
    }

    let pending_layer_overrides = scratch
        .pending_world_layer_override_by_entity
        .iter()
        .map(|(entity, layer_id)| (*entity, layer_id.clone()))
        .collect::<Vec<_>>();
    for (entity, layer_id) in pending_layer_overrides {
        if scratch.resolved_world_layer_by_entity.contains_key(&entity) {
            continue;
        }
        if let Some(definition) = runtime_layer_definitions_by_id.get(layer_id.as_str()) {
            scratch
                .resolved_world_layer_by_entity
                .insert(entity, definition.clone());
        }
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
    let all_replicated_entities = scratch.all_replicated_entities.clone();
    for &entity in &all_replicated_entities {
        let root = resolve_mount_root(entity, &scratch.parent_entity_by_entity);
        scratch.root_entity_by_entity.insert(entity, root);
    }

    // 3b) Build effective visibility positions and spatial index.
    // Mounted children inherit root world position for visibility checks to avoid
    // false positives from unhydrated child transforms at origin.
    scratch.entities_by_cell.clear();
    for &entity in &all_replicated_entities {
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
        let effective_extent_m = scratch
            .visibility_extent_m_by_entity
            .get(&root)
            .copied()
            .or_else(|| scratch.visibility_extent_m_by_entity.get(&entity).copied())
            .unwrap_or(0.0);
        scratch
            .visibility_extent_m_by_entity
            .insert(entity, effective_extent_m);
        scratch
            .entities_by_cell
            .entry(cell_key(effective, runtime_cfg.cell_size_m))
            .or_default()
            .push(entity);
    }

    // 4) Build visibility sources from owned roots with a resolved effective visibility range.
    // Child entities contribute via root VisibilityRangeM aggregation; they are not sources.
    let visibility_source_candidates = scratch.visibility_source_candidates.clone();
    for (entity, canonical_owner, range) in &visibility_source_candidates {
        let is_root = scratch
            .root_entity_by_entity
            .get(entity)
            .is_some_and(|root| *root == *entity);
        if !is_root {
            continue;
        }
        let Some(position) = scratch.world_position_by_entity.get(entity).copied() else {
            continue;
        };
        scratch
            .visibility_sources_by_owner
            .entry(canonical_owner.clone())
            .or_default()
            .push((position, *range));
    }
    let scratch_build_ms = started_at.elapsed().as_secs_f64() * 1000.0;

    let discovery_started_at = Instant::now();
    let registered_clients = scratch.registered_clients.clone();
    for (client_entity, player_entity_id) in &registered_clients {
        let canonical_player_id = canonical_player_entity_id(player_entity_id.as_str());
        let visibility_sources = scratch
            .visibility_sources_by_owner
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
        let local_view_settings = view_mode_registry
            .by_client_entity
            .get(client_entity)
            .copied()
            .unwrap_or(ClientLocalViewSettings {
                view_mode: ClientLocalViewMode::Tactical,
                delivery_range_m: runtime_cfg.delivery_range_m,
            });
        let local_view_mode = local_view_settings.view_mode;
        let client_delivery_range_m = local_view_settings.delivery_range_m;
        let player_entity = player_entities
            .by_player_entity_id
            .get(canonical_player_id.as_str())
            .copied()
            .or_else(|| {
                player_entities
                    .by_player_entity_id
                    .get(player_entity_id.as_str())
                    .copied()
            });
        let mut discovered_static_landmarks = HashSet::<uuid::Uuid>::new();
        if let Some(player_entity) = player_entity
            && let Ok(discovered_component) = player_landmark_state.get_mut(player_entity)
        {
            let mut discovered_component = discovered_component;
            if let Some(component) = discovered_component.as_deref() {
                discovered_static_landmarks.extend(component.landmark_entity_ids.iter().copied());
            }
            let mut newly_discovered = Vec::<uuid::Uuid>::new();
            let mut discovery_candidates = HashSet::<Entity>::new();
            for (visibility_pos, visibility_range_m) in &visibility_sources {
                add_entities_in_radius(
                    *visibility_pos,
                    *visibility_range_m + scratch.max_static_landmark_discovery_padding_m,
                    runtime_cfg.cell_size_m,
                    &scratch.entities_by_cell,
                    &mut discovery_candidates,
                );
            }
            for target_entity in discovery_candidates {
                let Some((target_guid, static_landmark)) =
                    scratch.static_landmarks_by_entity.get(&target_entity)
                else {
                    continue;
                };
                discovered_checks = discovered_checks.saturating_add(1);
                if discovered_static_landmarks.contains(target_guid) {
                    continue;
                }
                let target_position = scratch
                    .visibility_position_by_entity
                    .get(&target_entity)
                    .copied();
                let entity_extent_m = scratch
                    .visibility_extent_m_by_entity
                    .get(&target_entity)
                    .copied()
                    .unwrap_or(0.0);
                let discovery_context = PlayerVisibilityContextRef {
                    player_entity_id: canonical_player_id.as_str(),
                    observer_anchor_position,
                    visibility_sources: &visibility_sources,
                    player_faction_id: player_faction_id.as_deref(),
                    view_mode: local_view_mode,
                };
                if landmark_discovery_overlap(
                    target_position,
                    entity_extent_m,
                    static_landmark,
                    &discovery_context,
                ) {
                    newly_discovered.push(*target_guid);
                }
            }
            discovered_new_total = discovered_new_total.saturating_add(newly_discovered.len());
            if !newly_discovered.is_empty() {
                if let Some(component) = discovered_component.as_deref_mut() {
                    for landmark_id in newly_discovered {
                        if component.insert(landmark_id) {
                            discovered_static_landmarks.insert(landmark_id);
                        }
                    }
                } else {
                    let mut component = DiscoveredStaticLandmarks::default();
                    for landmark_id in newly_discovered {
                        if component.insert(landmark_id) {
                            discovered_static_landmarks.insert(landmark_id);
                        }
                    }
                    commands.entity(player_entity).insert(component);
                }
            }
        }
        let candidates = build_candidate_set_for_client(
            runtime_cfg.candidate_mode,
            canonical_player_id.as_str(),
            observer_anchor_position,
            client_delivery_range_m,
            &visibility_sources,
            local_view_mode,
            runtime_cfg.cell_size_m,
            &scratch,
        );
        let candidate_cells = build_candidate_cells_for_client(
            runtime_cfg.candidate_mode,
            observer_anchor_position,
            client_delivery_range_m,
            &visibility_sources,
            local_view_mode,
            runtime_cfg.cell_size_m,
        );
        scratch.client_states.push(ClientVisibilityComputedState {
            client_entity: *client_entity,
            player_entity_id: canonical_player_id,
            player_entity,
            observer_anchor_position,
            visibility_sources,
            discovered_static_landmarks,
            player_faction_id,
            view_mode: local_view_mode,
            delivery_range_m: client_delivery_range_m,
            candidate_entities: candidates,
            candidate_cells,
        });
    }
    let discovery_and_candidate_ms = discovery_started_at.elapsed().as_secs_f64() * 1000.0;

    let disclosure_started_at = Instant::now();
    for client_state in &scratch.client_states {
        let Some(player_entity) = client_state.player_entity else {
            continue;
        };
        let visibility_sources = client_state
            .visibility_sources
            .iter()
            .map(|(position, range_m)| VisibilityRangeSource {
                x: position.x,
                y: position.y,
                z: position.z,
                range_m: *range_m,
            })
            .collect::<Vec<_>>();
        let mut queried_cells = client_state
            .candidate_cells
            .iter()
            .copied()
            .map(|(x, y)| VisibilityGridCell { x, y })
            .collect::<Vec<_>>();
        queried_cells.sort_by_key(|cell| (cell.x, cell.y));

        let next_grid = VisibilitySpatialGrid {
            candidate_mode: runtime_cfg.candidate_mode.as_str().to_string(),
            cell_size_m: runtime_cfg.cell_size_m,
            delivery_range_m: client_state.delivery_range_m,
            queried_cells,
        };
        let next_disclosure = VisibilityDisclosure { visibility_sources };

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
    let disclosure_sync_ms = disclosure_started_at.elapsed().as_secs_f64() * 1000.0;

    let apply_started_at = Instant::now();
    for (
        entity,
        mut replication_state,
        controlled_by,
        entity_guid,
        player_tag,
        fullscreen_layer,
        runtime_render_layer,
        owner_id,
        public_visibility,
        faction_visibility,
        faction_id,
        _mounted_on,
        runtime_world_visual_stack,
        runtime_render_layer_override,
        static_landmark,
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
        let entity_extent_m = scratch
            .visibility_extent_m_by_entity
            .get(&entity)
            .copied()
            .unwrap_or(0.0);
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
            for client_state in &scratch.client_states {
                let is_owner = owner_player_id.is_some_and(|owner_id| {
                    player_entity_ids_match(client_state.player_entity_id.as_str(), owner_id)
                });
                if is_owner {
                    replication_state.gain_visibility(client_state.client_entity);
                } else if replication_state.is_visible(client_state.client_entity) {
                    replication_state.lose_visibility(client_state.client_entity);
                }
            }
            continue;
        }

        // Runtime render config entities are global non-spatial authored state and must not be
        // culled by delivery-range / visibility-range candidate logic as players move through the world.
        let is_global_fullscreen_config = fullscreen_layer.is_some()
            || runtime_render_layer.is_some_and(|layer| {
                layer.material_domain == RENDER_DOMAIN_FULLSCREEN
                    && matches!(
                        layer.phase.as_str(),
                        RENDER_PHASE_FULLSCREEN_BACKGROUND | RENDER_PHASE_FULLSCREEN_FOREGROUND
                    )
            });
        let is_global_render_config = is_global_fullscreen_config || runtime_render_layer.is_some();
        if is_global_render_config {
            for client_state in &scratch.client_states {
                replication_state.gain_visibility(client_state.client_entity);
            }
            for client_entity in &scratch.live_clients {
                if scratch
                    .client_states
                    .iter()
                    .all(|state| state.client_entity != *client_entity)
                    && replication_state.is_visible(*client_entity)
                {
                    replication_state.lose_visibility(*client_entity);
                }
            }
            continue;
        }

        if runtime_cfg.bypass_all_filters {
            for client_state in &scratch.client_states {
                replication_state.gain_visibility(client_state.client_entity);
            }
            continue;
        }

        for client_state in &scratch.client_states {
            let client_entity = client_state.client_entity;
            if controlled_by.is_some_and(|binding| binding.owner == client_entity) {
                // Hard guarantee: the owning client must always receive state for
                // their currently controlled entity, independent of visibility/range.
                replication_state.gain_visibility(client_entity);
                continue;
            }
            let visibility_context = PlayerVisibilityContextRef::from_client_state(client_state);
            let client_delivery_range_m = client_state.delivery_range_m;
            let is_discovered_static_landmark = static_landmark.is_some_and(|landmark| {
                landmark.always_known
                    || entity_guid.is_some_and(|guid| {
                        client_state.discovered_static_landmarks.contains(&guid.0)
                    })
            });
            let resolved_world_layer = scratch
                .resolved_world_layer_by_entity
                .get(&entity)
                .or_else(|| scratch.resolved_world_layer_by_entity.get(&root_entity))
                .or_else(|| {
                    runtime_render_layer_override.and_then(|override_layer| {
                        runtime_layer_definitions_by_id.get(&override_layer.layer_id)
                    })
                });
            let landmark_delivery_range_m = if is_discovered_static_landmark {
                discovered_landmark_delivery_range_m(client_delivery_range_m, resolved_world_layer)
            } else {
                client_delivery_range_m
            };
            let effective_entity_extent_m = if is_discovered_static_landmark {
                effective_discovered_landmark_extent_m(
                    entity_extent_m,
                    resolved_world_layer,
                    runtime_world_visual_stack,
                )
            } else {
                entity_extent_m
            };
            let in_candidates = client_state.candidate_entities.contains(&entity);
            let bypass_candidate = should_bypass_candidate_filter(
                visibility_context.player_entity_id,
                owner_player_id,
                is_public,
                is_faction_visible,
                is_discovered_static_landmark,
                entity_faction_id,
                entity_position,
                effective_entity_extent_m,
                &visibility_context,
            );
            if !in_candidates && !bypass_candidate {
                if replication_state.is_visible(client_entity) {
                    replication_state.lose_visibility(client_entity);
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
                visibility_context.player_entity_id,
                owner_player_id,
                is_public,
                is_faction_visible,
                is_discovered_static_landmark,
                entity_faction_id,
                entity_position,
                entity_extent_m,
                &visibility_context,
            );
            let delivery_ok =
                passes_delivery_scope(
                    entity_position,
                    effective_entity_extent_m,
                    &visibility_context,
                    landmark_delivery_range_m,
                ) || (matches!(visibility_context.view_mode, ClientLocalViewMode::Map)
                    && matches!(authorization, Some(VisibilityAuthorization::Owner)));
            let should_be_visible = is_entity_visible_to_player(
                visibility_context.player_entity_id,
                owner_player_id,
                is_public,
                is_faction_visible,
                is_discovered_static_landmark,
                entity_faction_id,
                entity_position,
                effective_entity_extent_m,
                &visibility_context,
                landmark_delivery_range_m,
                matches!(visibility_context.view_mode, ClientLocalViewMode::Map),
            );
            if should_be_visible {
                replication_state.gain_visibility(client_entity);
            } else if replication_state.is_visible(client_entity) {
                replication_state.lose_visibility(client_entity);
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
                    replication_state.is_visible(client_entity),
                    if should_be_visible {
                        "gain/keep"
                    } else {
                        "lose"
                    }
                );
            }
        }
    }
    let apply_ms = apply_started_at.elapsed().as_secs_f64() * 1000.0;

    if summary_logging_enabled() {
        let now_s = time.elapsed_secs_f64();
        const LOG_INTERVAL_S: f64 = 5.0;
        if now_s - telemetry_state.last_logged_at_s >= LOG_INTERVAL_S {
            telemetry_state.last_logged_at_s = now_s;
            let clients_count = scratch.client_states.len();
            let entities_count = scratch.all_replicated_entities.len();
            let candidates_total = scratch
                .client_states
                .iter()
                .map(|state| state.candidate_entities.len())
                .sum::<usize>();
            let candidates_per_client = if clients_count > 0 {
                candidates_total as f64 / clients_count as f64
            } else {
                0.0
            };
            let (delivery_min, delivery_avg, delivery_max) = if scratch.client_states.is_empty() {
                (
                    runtime_cfg.delivery_range_m as f64,
                    runtime_cfg.delivery_range_m as f64,
                    runtime_cfg.delivery_range_m as f64,
                )
            } else {
                let mut values = scratch
                    .client_states
                    .iter()
                    .map(|state| state.delivery_range_m as f64)
                    .collect::<Vec<_>>();
                values.sort_by(|a, b| a.total_cmp(b));
                let min = *values
                    .first()
                    .unwrap_or(&(runtime_cfg.delivery_range_m as f64));
                let max = *values
                    .last()
                    .unwrap_or(&(runtime_cfg.delivery_range_m as f64));
                let avg = values.iter().sum::<f64>() / values.len() as f64;
                (min, avg, max)
            };
            info!(
                "replication visibility summary mode={} bypass_all={} delivery_range_m[min/avg/max]={:.1}/{:.1}/{:.1} query_ms={:.2} clients={} entities={} candidates_per_client={:.1}",
                runtime_cfg.candidate_mode.as_str(),
                runtime_cfg.bypass_all_filters,
                delivery_min,
                delivery_avg,
                delivery_max,
                started_at.elapsed().as_secs_f64() * 1000.0,
                clients_count,
                entities_count,
                candidates_per_client
            );
        }
    }

    let clients_count = scratch.client_states.len();
    let entities_count = scratch.all_replicated_entities.len();
    let candidates_total = scratch
        .client_states
        .iter()
        .map(|state| state.candidate_entities.len())
        .sum::<usize>();
    let candidates_per_client = if clients_count > 0 {
        candidates_total as f64 / clients_count as f64
    } else {
        0.0
    };
    let (delivery_min, delivery_avg, delivery_max) = if scratch.client_states.is_empty() {
        (
            runtime_cfg.delivery_range_m as f64,
            runtime_cfg.delivery_range_m as f64,
            runtime_cfg.delivery_range_m as f64,
        )
    } else {
        let mut values = scratch
            .client_states
            .iter()
            .map(|state| state.delivery_range_m as f64)
            .collect::<Vec<_>>();
        values.sort_by(|a, b| a.total_cmp(b));
        let min = *values
            .first()
            .unwrap_or(&(runtime_cfg.delivery_range_m as f64));
        let max = *values
            .last()
            .unwrap_or(&(runtime_cfg.delivery_range_m as f64));
        let avg = values.iter().sum::<f64>() / values.len() as f64;
        (min, avg, max)
    };
    commands.insert_resource(VisibilityRuntimeMetrics {
        query_ms: started_at.elapsed().as_secs_f64() * 1000.0,
        scratch_build_ms,
        discovery_and_candidate_ms,
        disclosure_sync_ms,
        apply_ms,
        clients: clients_count,
        entities: entities_count,
        candidates_total,
        candidates_per_client,
        discovered_checks,
        discovered_new_total,
        delivery_range_min_m: delivery_min,
        delivery_range_avg_m: delivery_avg,
        delivery_range_max_m: delivery_max,
    });
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
    is_discovered_static_landmark: bool,
    entity_faction_id: Option<&str>,
    entity_position: Option<Vec3>,
    entity_extent_m: f32,
    visibility_context: &PlayerVisibilityContextRef<'_>,
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
        is_discovered_static_landmark,
        entity_faction_id,
        entity_position,
        entity_extent_m,
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

    passes_delivery_scope(
        entity_position,
        entity_extent_m,
        visibility_context,
        delivery_range_m,
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum VisibilityAuthorization {
    Owner,
    Public,
    Faction,
    DiscoveredStaticLandmark,
    Range,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn authorize_visibility(
    player_entity_id: &str,
    owner_player_id: Option<&str>,
    is_public_visibility: bool,
    is_faction_visibility: bool,
    is_discovered_static_landmark: bool,
    entity_faction_id: Option<&str>,
    entity_position: Option<Vec3>,
    entity_extent_m: f32,
    visibility_context: &PlayerVisibilityContextRef<'_>,
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
    if is_discovered_static_landmark {
        return Some(VisibilityAuthorization::DiscoveredStaticLandmark);
    }
    let target_position = entity_position?;
    visibility_context
        .visibility_sources
        .iter()
        .find(|(visibility_pos, visibility_range_m)| {
            (target_position - *visibility_pos).length() <= *visibility_range_m + entity_extent_m
        })
        .map(|_| VisibilityAuthorization::Range)
}

fn passes_delivery_scope(
    entity_position: Option<Vec3>,
    entity_extent_m: f32,
    visibility_context: &PlayerVisibilityContextRef<'_>,
    delivery_range_m: f32,
) -> bool {
    let (Some(observer_anchor_position), Some(target_position)) =
        (visibility_context.observer_anchor_position, entity_position)
    else {
        return false;
    };
    (target_position - observer_anchor_position).length() <= delivery_range_m + entity_extent_m
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

    #[test]
    fn delivery_scope_includes_entity_extent() {
        let discovered = HashSet::new();
        let visibility_sources = Vec::new();
        let visibility_context = PlayerVisibilityContextRef {
            player_entity_id: "11111111-1111-1111-1111-111111111111",
            observer_anchor_position: Some(Vec3::ZERO),
            visibility_sources: &visibility_sources,
            discovered_static_landmarks: &discovered,
            player_faction_id: None,
            view_mode: ClientLocalViewMode::Tactical,
        };

        assert!(passes_delivery_scope(
            Some(Vec3::new(1000.0, 0.0, 0.0)),
            100.0,
            &visibility_context,
            900.0,
        ));
    }

    #[test]
    fn authorization_range_includes_entity_extent() {
        let discovered = HashSet::new();
        let visibility_sources = vec![(Vec3::ZERO, 900.0)];
        let visibility_context = PlayerVisibilityContextRef {
            player_entity_id: "11111111-1111-1111-1111-111111111111",
            observer_anchor_position: Some(Vec3::ZERO),
            visibility_sources: &visibility_sources,
            discovered_static_landmarks: &discovered,
            player_faction_id: None,
            view_mode: ClientLocalViewMode::Tactical,
        };

        assert_eq!(
            authorize_visibility(
                "11111111-1111-1111-1111-111111111111",
                None,
                false,
                false,
                false,
                None,
                Some(Vec3::new(1000.0, 0.0, 0.0)),
                100.0,
                &visibility_context,
            ),
            Some(VisibilityAuthorization::Range)
        );
    }
}
