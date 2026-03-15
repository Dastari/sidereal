use avian2d::prelude::{AngularVelocity, LinearVelocity, Position, Rotation};
use bevy::ecs::system::SystemParam;
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
const DEFAULT_LANDMARK_DISCOVERY_INTERVAL_S: f64 = 0.25;

fn canonical_player_entity_id(id: &str) -> String {
    sidereal_net::PlayerEntityId::parse(id)
        .map(sidereal_net::PlayerEntityId::canonical_wire_id)
        .unwrap_or_else(|| id.to_string())
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
    fn from_cached_client_context(client_context: &'a CachedClientVisibilityContext) -> Self {
        Self {
            player_entity_id: client_context.player_entity_id.as_str(),
            observer_anchor_position: client_context.observer_anchor_position,
            visibility_sources: client_context.visibility_sources.as_slice(),
            player_faction_id: client_context.player_faction_id.as_deref(),
            view_mode: client_context.view_mode,
        }
    }
}

#[derive(Debug, Clone)]
struct ClientVisibilityComputedState {
    client_entity: Entity,
    candidate_entities: HashSet<Entity>,
    candidate_cells: HashSet<(i64, i64)>,
}

#[derive(Debug, Clone, PartialEq)]
struct CachedClientVisibilityContext {
    player_entity_id: String,
    player_entity: Option<Entity>,
    observer_anchor_position: Option<Vec3>,
    visibility_sources: Vec<(Vec3, f32)>,
    discovered_static_landmarks: HashSet<uuid::Uuid>,
    player_faction_id: Option<String>,
    view_mode: ClientLocalViewMode,
    delivery_range_m: f32,
}

#[derive(Resource, Default)]
pub struct VisibilityClientContextCache {
    by_client: HashMap<Entity, CachedClientVisibilityContext>,
}

impl VisibilityClientContextCache {
    pub fn remove_client(&mut self, client_entity: Entity) {
        self.by_client.remove(&client_entity);
    }

    pub fn clear(&mut self) {
        self.by_client.clear();
    }
}

#[derive(Resource, Default)]
pub struct VisibilityMembershipCache {
    by_entity: HashMap<Entity, HashSet<Entity>>,
}

impl VisibilityMembershipCache {
    pub fn clear(&mut self) {
        self.by_entity.clear();
    }
}

#[derive(Resource, Default)]
pub struct VisibilitySpatialIndex {
    cell_size_m: f32,
    entity_by_guid: HashMap<uuid::Uuid, Entity>,
    world_position_by_entity: HashMap<Entity, Vec3>,
    base_extent_m_by_entity: HashMap<Entity, f32>,
    visibility_position_by_entity: HashMap<Entity, Vec3>,
    visibility_extent_m_by_entity: HashMap<Entity, f32>,
    parent_entity_by_entity: HashMap<Entity, Entity>,
    root_entity_by_entity: HashMap<Entity, Entity>,
    entities_by_root: HashMap<Entity, HashSet<Entity>>,
    entities_by_cell: HashMap<(i64, i64), Vec<Entity>>,
    cell_by_entity: HashMap<Entity, (i64, i64)>,
}

impl VisibilitySpatialIndex {
    pub fn clear(&mut self) {
        self.cell_size_m = 0.0;
        self.entity_by_guid.clear();
        self.world_position_by_entity.clear();
        self.base_extent_m_by_entity.clear();
        self.visibility_position_by_entity.clear();
        self.visibility_extent_m_by_entity.clear();
        self.parent_entity_by_entity.clear();
        self.root_entity_by_entity.clear();
        self.entities_by_root.clear();
        self.entities_by_cell.clear();
        self.cell_by_entity.clear();
    }
}

#[derive(Debug, Clone, Default)]
struct CachedVisibilityEntity {
    guid: Option<uuid::Uuid>,
    owner_player_id: Option<String>,
    visibility_range_m: Option<f32>,
    public_visibility: bool,
    faction_visibility: bool,
    faction_id: Option<String>,
    parent_guid: Option<uuid::Uuid>,
    entity_extent_m: f32,
    runtime_render_layer_definition: Option<RuntimeRenderLayerDefinition>,
    pending_world_layer_override: Option<String>,
    static_landmark: Option<StaticLandmark>,
    is_player_tag: bool,
    is_global_render_config: bool,
}

#[derive(Resource, Default)]
pub struct VisibilityEntityCache {
    by_entity: HashMap<Entity, CachedVisibilityEntity>,
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

#[allow(clippy::type_complexity)]
#[derive(SystemParam)]
pub struct VisibilityCacheRefreshParams<'w, 's> {
    replicated_entities: Query<
        'w,
        's,
        (
            Entity,
            Option<&'static EntityGuid>,
            Option<&'static OwnerId>,
            Option<&'static VisibilityRangeM>,
            Option<&'static PublicVisibility>,
            Option<&'static FactionVisibility>,
            Option<&'static FactionId>,
            Option<&'static MountedOn>,
            Option<&'static ParentGuid>,
            Option<&'static SizeM>,
            Option<&'static RuntimeRenderLayerDefinition>,
            Option<&'static RuntimeRenderLayerOverride>,
            Option<&'static StaticLandmark>,
            Option<&'static PlayerTag>,
            Option<&'static FullscreenLayer>,
        ),
        With<Replicate>,
    >,
    changed_replicated_entities: Query<
        'w,
        's,
        (
            Entity,
            Option<&'static EntityGuid>,
            Option<&'static OwnerId>,
            Option<&'static VisibilityRangeM>,
            Option<&'static PublicVisibility>,
            Option<&'static FactionVisibility>,
            Option<&'static FactionId>,
            Option<&'static MountedOn>,
            Option<&'static ParentGuid>,
            Option<&'static SizeM>,
            Option<&'static RuntimeRenderLayerDefinition>,
            Option<&'static RuntimeRenderLayerOverride>,
            Option<&'static StaticLandmark>,
            Option<&'static PlayerTag>,
            Option<&'static FullscreenLayer>,
        ),
        (
            With<Replicate>,
            Or<(
                Added<Replicate>,
                Changed<EntityGuid>,
                Changed<OwnerId>,
                Changed<VisibilityRangeM>,
                Changed<PublicVisibility>,
                Changed<FactionVisibility>,
                Changed<FactionId>,
                Changed<MountedOn>,
                Changed<ParentGuid>,
                Changed<SizeM>,
                Changed<RuntimeRenderLayerDefinition>,
                Changed<RuntimeRenderLayerOverride>,
                Changed<StaticLandmark>,
                Changed<PlayerTag>,
                Changed<FullscreenLayer>,
            )>,
        ),
    >,
    removed_replicates: RemovedComponents<'w, 's, Replicate>,
    removed_entity_guid: RemovedComponents<'w, 's, EntityGuid>,
    removed_owner_id: RemovedComponents<'w, 's, OwnerId>,
    removed_visibility_range: RemovedComponents<'w, 's, VisibilityRangeM>,
    removed_public_visibility: RemovedComponents<'w, 's, PublicVisibility>,
    removed_faction_visibility: RemovedComponents<'w, 's, FactionVisibility>,
    removed_faction_id: RemovedComponents<'w, 's, FactionId>,
    removed_mounted_on: RemovedComponents<'w, 's, MountedOn>,
    removed_parent_guid: RemovedComponents<'w, 's, ParentGuid>,
    removed_size: RemovedComponents<'w, 's, SizeM>,
    removed_runtime_render_layer_definition:
        RemovedComponents<'w, 's, RuntimeRenderLayerDefinition>,
    removed_runtime_render_layer_override: RemovedComponents<'w, 's, RuntimeRenderLayerOverride>,
    removed_static_landmark: RemovedComponents<'w, 's, StaticLandmark>,
    removed_player_tag: RemovedComponents<'w, 's, PlayerTag>,
    removed_fullscreen_layer: RemovedComponents<'w, 's, FullscreenLayer>,
}

#[allow(clippy::type_complexity)]
#[derive(SystemParam)]
pub struct VisibilityUpdateParams<'w, 's> {
    clients: Query<'w, 's, Entity, With<ClientOf>>,
    cache: Res<'w, VisibilityEntityCache>,
    client_context_cache: ResMut<'w, VisibilityClientContextCache>,
    membership_cache: ResMut<'w, VisibilityMembershipCache>,
    spatial_index: Res<'w, VisibilitySpatialIndex>,
    visibility_registry: Res<'w, ClientVisibilityRegistry>,
    view_mode_registry: ResMut<'w, ClientLocalViewModeRegistry>,
    player_entities: Res<'w, PlayerRuntimeEntityMap>,
    scratch: ResMut<'w, VisibilityScratch>,
    observer_anchor_positions: Res<'w, ClientObserverAnchorPositionMap>,
    player_visibility_state: Query<
        'w,
        's,
        (
            Option<&'static VisibilitySpatialGrid>,
            Option<&'static VisibilityDisclosure>,
        ),
        With<PlayerTag>,
    >,
    player_landmark_state:
        Query<'w, 's, Option<&'static DiscoveredStaticLandmarks>, With<PlayerTag>>,
    all_replicated: Query<
        'w,
        's,
        (Entity, Option<&'static Position>, &'static GlobalTransform),
        With<Replicate>,
    >,
    replicated_entities: Query<
        'w,
        's,
        (
            Entity,
            &'static mut ReplicationState,
            Option<&'static ControlledBy>,
            Option<&'static RuntimeWorldVisualStack>,
        ),
        With<Replicate>,
    >,
}

#[allow(clippy::type_complexity)]
#[derive(SystemParam)]
pub struct VisibilityLandmarkDiscoveryParams<'w, 's> {
    cache: Res<'w, VisibilityEntityCache>,
    client_context_cache: ResMut<'w, VisibilityClientContextCache>,
    player_landmark_state:
        Query<'w, 's, Option<&'static mut DiscoveredStaticLandmarks>, With<PlayerTag>>,
    all_replicated: Query<
        'w,
        's,
        (Entity, Option<&'static Position>, &'static GlobalTransform),
        With<Replicate>,
    >,
}

#[allow(clippy::type_complexity)]
#[derive(SystemParam)]
pub struct VisibilitySpatialIndexRefreshParams<'w, 's> {
    cache: Res<'w, VisibilityEntityCache>,
    all_replicated: Query<
        'w,
        's,
        (Entity, Option<&'static Position>, &'static GlobalTransform),
        With<Replicate>,
    >,
    changed_replicated: Query<
        'w,
        's,
        (Entity, Option<&'static Position>, &'static GlobalTransform),
        (
            With<Replicate>,
            Or<(
                Added<Replicate>,
                Changed<GlobalTransform>,
                Changed<Position>,
                Changed<MountedOn>,
                Changed<ParentGuid>,
                Changed<SizeM>,
                Changed<EntityGuid>,
            )>,
        ),
    >,
    removed_replicates: RemovedComponents<'w, 's, Replicate>,
    removed_mounted_on: RemovedComponents<'w, 's, MountedOn>,
    removed_parent_guid: RemovedComponents<'w, 's, ParentGuid>,
    removed_size: RemovedComponents<'w, 's, SizeM>,
    removed_entity_guid: RemovedComponents<'w, 's, EntityGuid>,
}

#[derive(Resource)]
pub(crate) struct VisibilityRuntimeConfig {
    candidate_mode: VisibilityCandidateMode,
    delivery_range_m: f32,
    cell_size_m: f32,
    landmark_discovery_interval_s: f64,
    bypass_all_filters: bool,
}

#[derive(Resource, Default)]
pub struct VisibilityTelemetryLogState {
    pub last_logged_at_s: f64,
}

#[derive(Debug, Resource, Default, Clone)]
pub struct VisibilityPreparationMetrics {
    pub cache_refresh_ms: f64,
    pub cache_entries: usize,
    pub cache_upserts: usize,
    pub cache_removals: usize,
}

#[allow(dead_code)]
#[derive(Debug, Resource, Default, Clone)]
pub struct VisibilityRuntimeMetrics {
    pub cache_refresh_ms: f64,
    pub cache_upserts: usize,
    pub cache_removals: usize,
    pub client_context_refresh_ms: f64,
    pub client_cache_entries: usize,
    pub client_cache_upserts: usize,
    pub client_cache_removals: usize,
    pub landmark_discovery_ms: f64,
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
    pub occupied_cells: usize,
    pub max_entities_per_cell: usize,
    pub visible_gains: usize,
    pub visible_losses: usize,
}

#[derive(Debug, Resource, Default, Clone)]
pub struct VisibilityLandmarkDiscoveryMetrics {
    pub landmark_discovery_ms: f64,
    pub discovered_checks: usize,
    pub discovered_new_total: usize,
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

fn resolved_parent_guid(
    mounted_on: Option<&MountedOn>,
    parent_guid: Option<&ParentGuid>,
) -> Option<uuid::Uuid> {
    mounted_on
        .map(|mounted| mounted.parent_entity_id)
        .or_else(|| parent_guid.map(|parent| parent.0))
}

fn is_global_render_config_entity(
    has_fullscreen_layer: bool,
    runtime_render_layer: Option<&RuntimeRenderLayerDefinition>,
) -> bool {
    let is_global_fullscreen_config = has_fullscreen_layer
        || runtime_render_layer.is_some_and(|layer| {
            layer.material_domain == RENDER_DOMAIN_FULLSCREEN
                && matches!(
                    layer.phase.as_str(),
                    RENDER_PHASE_FULLSCREEN_BACKGROUND | RENDER_PHASE_FULLSCREEN_FOREGROUND
                )
        });
    is_global_fullscreen_config || runtime_render_layer.is_some()
}

#[allow(clippy::too_many_arguments)]
fn build_cached_visibility_entity(
    guid: Option<&EntityGuid>,
    owner_id: Option<&OwnerId>,
    visibility_range: Option<&VisibilityRangeM>,
    public_visibility: Option<&PublicVisibility>,
    faction_visibility: Option<&FactionVisibility>,
    faction_id: Option<&FactionId>,
    mounted_on: Option<&MountedOn>,
    parent_guid: Option<&ParentGuid>,
    size: Option<&SizeM>,
    runtime_render_layer_definition: Option<&RuntimeRenderLayerDefinition>,
    runtime_render_layer_override: Option<&RuntimeRenderLayerOverride>,
    static_landmark: Option<&StaticLandmark>,
    player_tag: Option<&PlayerTag>,
    fullscreen_layer: Option<&FullscreenLayer>,
) -> CachedVisibilityEntity {
    CachedVisibilityEntity {
        guid: guid.map(|value| value.0),
        owner_player_id: owner_id.map(|owner| canonical_player_entity_id(owner.0.as_str())),
        visibility_range_m: visibility_range.map(|value| value.0),
        public_visibility: public_visibility.is_some(),
        faction_visibility: faction_visibility.is_some(),
        faction_id: faction_id.map(|value| value.0.clone()),
        parent_guid: resolved_parent_guid(mounted_on, parent_guid),
        entity_extent_m: entity_visibility_extent_m(size),
        runtime_render_layer_definition: runtime_render_layer_definition.cloned(),
        pending_world_layer_override: runtime_render_layer_override
            .map(|value| value.layer_id.clone()),
        static_landmark: static_landmark.cloned(),
        is_player_tag: player_tag.is_some(),
        is_global_render_config: is_global_render_config_entity(
            fullscreen_layer.is_some(),
            runtime_render_layer_definition,
        ),
    }
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
    app.insert_resource(VisibilityEntityCache::default());
    app.insert_resource(VisibilityClientContextCache::default());
    app.insert_resource(VisibilityMembershipCache::default());
    app.insert_resource(VisibilitySpatialIndex::default());
    app.insert_resource(VisibilityScratch::default());
    app.insert_resource(ClientObserverAnchorPositionMap::default());
    app.insert_resource(VisibilityRuntimeConfig {
        candidate_mode,
        delivery_range_m,
        cell_size_m,
        landmark_discovery_interval_s: DEFAULT_LANDMARK_DISCOVERY_INTERVAL_S,
        bypass_all_filters: bypass_all_visibility_filters_from_env(),
    });
    app.insert_resource(VisibilityTelemetryLogState::default());
    app.insert_resource(VisibilityPreparationMetrics::default());
    app.insert_resource(VisibilityLandmarkDiscoveryMetrics::default());
    app.insert_resource(VisibilityRuntimeMetrics::default());
    app.insert_resource(ClientLocalViewModeRegistry::default());
}

#[allow(clippy::type_complexity)]
pub fn refresh_visibility_entity_cache(
    mut cache: ResMut<'_, VisibilityEntityCache>,
    mut preparation_metrics: ResMut<'_, VisibilityPreparationMetrics>,
    mut refresh: VisibilityCacheRefreshParams<'_, '_>,
) {
    let started_at = Instant::now();
    let mut cache_upserts = 0usize;
    let mut cache_removals = 0usize;
    let mut dirty_entities = HashSet::<Entity>::new();

    for entity in refresh.removed_replicates.read() {
        if cache.by_entity.remove(&entity).is_some() {
            cache_removals = cache_removals.saturating_add(1);
        }
    }

    for entity in refresh.removed_entity_guid.read() {
        dirty_entities.insert(entity);
    }
    for entity in refresh.removed_owner_id.read() {
        dirty_entities.insert(entity);
    }
    for entity in refresh.removed_visibility_range.read() {
        dirty_entities.insert(entity);
    }
    for entity in refresh.removed_public_visibility.read() {
        dirty_entities.insert(entity);
    }
    for entity in refresh.removed_faction_visibility.read() {
        dirty_entities.insert(entity);
    }
    for entity in refresh.removed_faction_id.read() {
        dirty_entities.insert(entity);
    }
    for entity in refresh.removed_mounted_on.read() {
        dirty_entities.insert(entity);
    }
    for entity in refresh.removed_parent_guid.read() {
        dirty_entities.insert(entity);
    }
    for entity in refresh.removed_size.read() {
        dirty_entities.insert(entity);
    }
    for entity in refresh.removed_runtime_render_layer_definition.read() {
        dirty_entities.insert(entity);
    }
    for entity in refresh.removed_runtime_render_layer_override.read() {
        dirty_entities.insert(entity);
    }
    for entity in refresh.removed_static_landmark.read() {
        dirty_entities.insert(entity);
    }
    for entity in refresh.removed_player_tag.read() {
        dirty_entities.insert(entity);
    }
    for entity in refresh.removed_fullscreen_layer.read() {
        dirty_entities.insert(entity);
    }

    for (
        entity,
        guid,
        owner_id,
        visibility_range,
        public_visibility,
        faction_visibility,
        faction_id,
        mounted_on,
        parent_guid,
        size,
        runtime_render_layer_definition,
        runtime_render_layer_override,
        static_landmark,
        player_tag,
        fullscreen_layer,
    ) in &refresh.changed_replicated_entities
    {
        cache.by_entity.insert(
            entity,
            build_cached_visibility_entity(
                guid,
                owner_id,
                visibility_range,
                public_visibility,
                faction_visibility,
                faction_id,
                mounted_on,
                parent_guid,
                size,
                runtime_render_layer_definition,
                runtime_render_layer_override,
                static_landmark,
                player_tag,
                fullscreen_layer,
            ),
        );
        cache_upserts = cache_upserts.saturating_add(1);
        dirty_entities.remove(&entity);
    }

    if cache.by_entity.is_empty() {
        for (
            entity,
            guid,
            owner_id,
            visibility_range,
            public_visibility,
            faction_visibility,
            faction_id,
            mounted_on,
            parent_guid,
            size,
            runtime_render_layer_definition,
            runtime_render_layer_override,
            static_landmark,
            player_tag,
            fullscreen_layer,
        ) in &refresh.replicated_entities
        {
            cache.by_entity.insert(
                entity,
                build_cached_visibility_entity(
                    guid,
                    owner_id,
                    visibility_range,
                    public_visibility,
                    faction_visibility,
                    faction_id,
                    mounted_on,
                    parent_guid,
                    size,
                    runtime_render_layer_definition,
                    runtime_render_layer_override,
                    static_landmark,
                    player_tag,
                    fullscreen_layer,
                ),
            );
            cache_upserts = cache_upserts.saturating_add(1);
        }
    } else {
        for entity in dirty_entities {
            if let Ok((
                _,
                guid,
                owner_id,
                visibility_range,
                public_visibility,
                faction_visibility,
                faction_id,
                mounted_on,
                parent_guid,
                size,
                runtime_render_layer_definition,
                runtime_render_layer_override,
                static_landmark,
                player_tag,
                fullscreen_layer,
            )) = refresh.replicated_entities.get(entity)
            {
                cache.by_entity.insert(
                    entity,
                    build_cached_visibility_entity(
                        guid,
                        owner_id,
                        visibility_range,
                        public_visibility,
                        faction_visibility,
                        faction_id,
                        mounted_on,
                        parent_guid,
                        size,
                        runtime_render_layer_definition,
                        runtime_render_layer_override,
                        static_landmark,
                        player_tag,
                        fullscreen_layer,
                    ),
                );
                cache_upserts = cache_upserts.saturating_add(1);
            } else if cache.by_entity.remove(&entity).is_some() {
                cache_removals = cache_removals.saturating_add(1);
            }
        }
    }

    preparation_metrics.cache_refresh_ms = started_at.elapsed().as_secs_f64() * 1000.0;
    preparation_metrics.cache_entries = cache.by_entity.len();
    preparation_metrics.cache_upserts = cache_upserts;
    preparation_metrics.cache_removals = cache_removals;
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

fn landmark_discovery_due(now_s: f64, last_run_at_s: Option<f64>, interval_s: f64) -> bool {
    last_run_at_s.is_none_or(|last_run_at_s| now_s - last_run_at_s >= interval_s)
}

pub fn refresh_static_landmark_discoveries(
    mut commands: Commands<'_, '_>,
    time: Res<'_, Time>,
    runtime_cfg: Res<'_, VisibilityRuntimeConfig>,
    mut metrics: ResMut<'_, VisibilityLandmarkDiscoveryMetrics>,
    mut last_run_at_s: Local<'_, Option<f64>>,
    mut params: VisibilityLandmarkDiscoveryParams<'_, '_>,
) {
    metrics.landmark_discovery_ms = 0.0;
    metrics.discovered_checks = 0;
    metrics.discovered_new_total = 0;

    let now_s = time.elapsed_secs_f64();
    if !landmark_discovery_due(
        now_s,
        *last_run_at_s,
        runtime_cfg.landmark_discovery_interval_s,
    ) {
        return;
    }
    *last_run_at_s = Some(now_s);

    let started_at = Instant::now();
    let mut static_landmarks_by_entity = HashMap::<Entity, (uuid::Uuid, StaticLandmark)>::new();
    let mut visibility_position_by_entity = HashMap::<Entity, Vec3>::new();
    let mut visibility_extent_m_by_entity = HashMap::<Entity, f32>::new();
    let mut entities_by_cell = HashMap::<(i64, i64), Vec<Entity>>::new();
    let mut max_static_landmark_discovery_padding_m = 0.0f32;

    for (entity, position, global_transform) in &params.all_replicated {
        let Some(cached) = params.cache.by_entity.get(&entity) else {
            continue;
        };
        let Some((guid, static_landmark)) = cached
            .guid
            .zip(cached.static_landmark.as_ref())
            .map(|(guid, landmark)| (guid, landmark.clone()))
        else {
            continue;
        };
        let world_pos = global_transform.translation();
        let effective_world_pos = if world_pos.is_finite() {
            world_pos
        } else {
            position.map(|p| p.0.extend(0.0)).unwrap_or(Vec3::ZERO)
        };
        visibility_position_by_entity.insert(entity, effective_world_pos);
        visibility_extent_m_by_entity.insert(entity, cached.entity_extent_m);
        static_landmarks_by_entity.insert(entity, (guid, static_landmark.clone()));
        entities_by_cell
            .entry(cell_key(effective_world_pos, runtime_cfg.cell_size_m))
            .or_default()
            .push(entity);
        let discovery_padding_m = static_landmark.discovery_radius_m.unwrap_or(0.0).max(0.0)
            + if static_landmark.use_extent_for_discovery {
                cached.entity_extent_m
            } else {
                0.0
            };
        max_static_landmark_discovery_padding_m =
            max_static_landmark_discovery_padding_m.max(discovery_padding_m);
    }

    for client_context in params.client_context_cache.by_client.values_mut() {
        let Some(player_entity) = client_context.player_entity else {
            continue;
        };
        let Ok(discovered_component) = params.player_landmark_state.get_mut(player_entity) else {
            continue;
        };
        let mut discovered_component = discovered_component;
        let mut discovered_static_landmarks: HashSet<uuid::Uuid> = discovered_component
            .as_deref()
            .map(|component| component.landmark_entity_ids.iter().copied().collect())
            .unwrap_or_default();
        let mut newly_discovered = Vec::<uuid::Uuid>::new();
        let mut discovery_candidates = HashSet::<Entity>::new();
        for (visibility_pos, visibility_range_m) in &client_context.visibility_sources {
            add_entities_in_radius(
                *visibility_pos,
                *visibility_range_m + max_static_landmark_discovery_padding_m,
                runtime_cfg.cell_size_m,
                &entities_by_cell,
                &mut discovery_candidates,
            );
        }
        let discovery_context =
            PlayerVisibilityContextRef::from_cached_client_context(client_context);
        for target_entity in discovery_candidates {
            let Some((target_guid, static_landmark)) =
                static_landmarks_by_entity.get(&target_entity)
            else {
                continue;
            };
            metrics.discovered_checks = metrics.discovered_checks.saturating_add(1);
            if discovered_static_landmarks.contains(target_guid) {
                continue;
            }
            let target_position = visibility_position_by_entity.get(&target_entity).copied();
            let entity_extent_m = visibility_extent_m_by_entity
                .get(&target_entity)
                .copied()
                .unwrap_or(0.0);
            if landmark_discovery_overlap(
                target_position,
                entity_extent_m,
                static_landmark,
                &discovery_context,
            ) {
                newly_discovered.push(*target_guid);
            }
        }
        metrics.discovered_new_total = metrics
            .discovered_new_total
            .saturating_add(newly_discovered.len());
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
        client_context.discovered_static_landmarks = discovered_static_landmarks;
    }

    metrics.landmark_discovery_ms = started_at.elapsed().as_secs_f64() * 1000.0;
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
    all_replicated_entities: &[Entity],
    owned_entities_by_player: &HashMap<String, Vec<Entity>>,
    entities_by_cell: &HashMap<(i64, i64), Vec<Entity>>,
) -> HashSet<Entity> {
    match candidate_mode {
        VisibilityCandidateMode::FullScan => {
            let mut all = HashSet::with_capacity(all_replicated_entities.len());
            all.extend(all_replicated_entities.iter().copied());
            all
        }
        VisibilityCandidateMode::SpatialGrid => {
            let mut candidates = HashSet::new();
            if matches!(view_mode, ClientLocalViewMode::Map)
                && let Some(owned_entities) = owned_entities_by_player.get(player_entity_id)
            {
                candidates.extend(owned_entities.iter().copied());
            }
            if let Some(observer_anchor) = observer_anchor_position {
                add_entities_in_radius(
                    observer_anchor,
                    observer_delivery_range_m,
                    cell_size_m,
                    entities_by_cell,
                    &mut candidates,
                );
            }
            if matches!(view_mode, ClientLocalViewMode::Map) {
                for (visibility_pos, visibility_range) in visibility_sources {
                    add_entities_in_radius(
                        *visibility_pos,
                        *visibility_range,
                        cell_size_m,
                        entities_by_cell,
                        &mut candidates,
                    );
                }
            }
            candidates
        }
    }
}

#[allow(clippy::too_many_arguments)]
#[cfg_attr(not(test), allow(dead_code))]
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

fn apply_visibility_membership_diff(
    replication_state: &mut ReplicationState,
    current_visible_clients: &mut HashSet<Entity>,
    desired_visible_clients: &HashSet<Entity>,
    visible_gains: &mut usize,
    visible_losses: &mut usize,
) -> usize {
    let gained_clients = desired_visible_clients
        .iter()
        .filter(|client_entity| !current_visible_clients.contains(client_entity))
        .copied()
        .collect::<Vec<_>>();
    let lost_clients = current_visible_clients
        .iter()
        .filter(|client_entity| !desired_visible_clients.contains(client_entity))
        .copied()
        .collect::<Vec<_>>();

    for client_entity in &gained_clients {
        replication_state.gain_visibility(*client_entity);
    }
    for client_entity in &lost_clients {
        replication_state.lose_visibility(*client_entity);
    }

    *visible_gains = visible_gains.saturating_add(gained_clients.len());
    *visible_losses = visible_losses.saturating_add(lost_clients.len());

    current_visible_clients.clear();
    current_visible_clients.extend(desired_visible_clients.iter().copied());

    gained_clients.len()
}

fn queue_visibility_gain_spatial_resend(commands: &mut Commands<'_, '_>, entity: Entity) {
    commands.queue(move |world: &mut World| {
        let Ok(mut entity_mut) = world.get_entity_mut(entity) else {
            return;
        };
        if let Some(mut position) = entity_mut.get_mut::<Position>() {
            position.set_changed();
        }
        if let Some(mut rotation) = entity_mut.get_mut::<Rotation>() {
            rotation.set_changed();
        }
        if let Some(mut linear_velocity) = entity_mut.get_mut::<LinearVelocity>() {
            linear_velocity.set_changed();
        }
        if let Some(mut angular_velocity) = entity_mut.get_mut::<AngularVelocity>() {
            angular_velocity.set_changed();
        }
    });
}

fn remove_entity_from_cell_index(index: &mut VisibilitySpatialIndex, entity: Entity) {
    let Some(cell_key) = index.cell_by_entity.remove(&entity) else {
        return;
    };
    if let Some(entities) = index.entities_by_cell.get_mut(&cell_key) {
        entities.retain(|candidate| *candidate != entity);
        if entities.is_empty() {
            index.entities_by_cell.remove(&cell_key);
        }
    }
}

fn expected_parent_entity_for_spatial_index(
    index: &VisibilitySpatialIndex,
    cache: &VisibilityEntityCache,
    entity: Entity,
) -> Option<Entity> {
    let parent_guid = cache
        .by_entity
        .get(&entity)
        .and_then(|cached| cached.parent_guid)?;
    let parent_entity = index.entity_by_guid.get(&parent_guid).copied()?;
    (parent_entity != entity).then_some(parent_entity)
}

fn spatial_index_requires_full_rebuild(
    index: &VisibilitySpatialIndex,
    cache: &VisibilityEntityCache,
    entity: Entity,
) -> bool {
    let Some(cached) = cache.by_entity.get(&entity) else {
        return true;
    };
    if !index.world_position_by_entity.contains_key(&entity)
        || !index.base_extent_m_by_entity.contains_key(&entity)
        || !index.visibility_position_by_entity.contains_key(&entity)
        || !index.visibility_extent_m_by_entity.contains_key(&entity)
        || !index.root_entity_by_entity.contains_key(&entity)
        || !index.cell_by_entity.contains_key(&entity)
    {
        return true;
    }
    if let Some(guid) = cached.guid
        && index.entity_by_guid.get(&guid).copied() != Some(entity)
    {
        return true;
    }
    index.parent_entity_by_entity.get(&entity).copied()
        != expected_parent_entity_for_spatial_index(index, cache, entity)
}

fn rebuild_visibility_spatial_index(
    index: &mut VisibilitySpatialIndex,
    cache: &VisibilityEntityCache,
    all_replicated: &Query<'_, '_, (Entity, Option<&Position>, &GlobalTransform), With<Replicate>>,
    cell_size_m: f32,
) {
    index.clear();
    index.cell_size_m = cell_size_m;

    let mut all_entities = Vec::<Entity>::new();
    for (entity, position, global_transform) in all_replicated {
        all_entities.push(entity);
        let Some(cached) = cache.by_entity.get(&entity) else {
            continue;
        };
        if let Some(guid) = cached.guid {
            index.entity_by_guid.insert(guid, entity);
        }
        let world_pos = global_transform.translation();
        let effective_world_pos = if world_pos.is_finite() {
            world_pos
        } else {
            position.map(|p| p.0.extend(0.0)).unwrap_or(Vec3::ZERO)
        };
        index
            .world_position_by_entity
            .insert(entity, effective_world_pos);
        index
            .base_extent_m_by_entity
            .insert(entity, cached.entity_extent_m);
    }

    for &entity in &all_entities {
        let parent_guid = cache
            .by_entity
            .get(&entity)
            .and_then(|cached| cached.parent_guid);
        let Some(parent_guid) = parent_guid else {
            continue;
        };
        let Some(&parent_entity) = index.entity_by_guid.get(&parent_guid) else {
            continue;
        };
        if parent_entity != entity {
            index.parent_entity_by_entity.insert(entity, parent_entity);
        }
    }

    for &entity in &all_entities {
        let root = resolve_mount_root(entity, &index.parent_entity_by_entity);
        index.root_entity_by_entity.insert(entity, root);
        index
            .entities_by_root
            .entry(root)
            .or_default()
            .insert(entity);
    }

    for &entity in &all_entities {
        let root = index
            .root_entity_by_entity
            .get(&entity)
            .copied()
            .unwrap_or(entity);
        let effective_position = index
            .world_position_by_entity
            .get(&root)
            .copied()
            .or_else(|| index.world_position_by_entity.get(&entity).copied())
            .unwrap_or(Vec3::ZERO);
        let effective_extent_m = index
            .base_extent_m_by_entity
            .get(&root)
            .copied()
            .or_else(|| index.base_extent_m_by_entity.get(&entity).copied())
            .unwrap_or(0.0);
        let cell = cell_key(effective_position, cell_size_m);
        index
            .visibility_position_by_entity
            .insert(entity, effective_position);
        index
            .visibility_extent_m_by_entity
            .insert(entity, effective_extent_m);
        index.cell_by_entity.insert(entity, cell);
        index.entities_by_cell.entry(cell).or_default().push(entity);
    }
}

pub fn refresh_visibility_spatial_index(
    mut index: ResMut<'_, VisibilitySpatialIndex>,
    runtime_cfg: Res<'_, VisibilityRuntimeConfig>,
    mut params: VisibilitySpatialIndexRefreshParams<'_, '_>,
) {
    let mut full_rebuild_required =
        index.cell_size_m != runtime_cfg.cell_size_m || index.world_position_by_entity.is_empty();

    for _ in params.removed_replicates.read() {
        full_rebuild_required = true;
    }
    for _ in params.removed_mounted_on.read() {
        full_rebuild_required = true;
    }
    for _ in params.removed_parent_guid.read() {
        full_rebuild_required = true;
    }
    for _ in params.removed_size.read() {
        full_rebuild_required = true;
    }
    for _ in params.removed_entity_guid.read() {
        full_rebuild_required = true;
    }
    for (entity, _, _) in &params.changed_replicated {
        if spatial_index_requires_full_rebuild(&index, &params.cache, entity) {
            full_rebuild_required = true;
            break;
        }
    }

    if full_rebuild_required {
        rebuild_visibility_spatial_index(
            &mut index,
            &params.cache,
            &params.all_replicated,
            runtime_cfg.cell_size_m,
        );
        return;
    }

    let mut affected_roots = HashSet::<Entity>::new();
    for (entity, position, global_transform) in &params.changed_replicated {
        let world_pos = global_transform.translation();
        let effective_world_pos = if world_pos.is_finite() {
            world_pos
        } else {
            position.map(|p| p.0.extend(0.0)).unwrap_or(Vec3::ZERO)
        };
        index
            .world_position_by_entity
            .insert(entity, effective_world_pos);
        if let Some(cached) = params.cache.by_entity.get(&entity) {
            index
                .base_extent_m_by_entity
                .insert(entity, cached.entity_extent_m);
        }
        affected_roots.insert(
            index
                .root_entity_by_entity
                .get(&entity)
                .copied()
                .unwrap_or(entity),
        );
    }

    for affected_root in affected_roots {
        let Some(entities_under_root) = index.entities_by_root.get(&affected_root).cloned() else {
            continue;
        };
        let root_position = index
            .world_position_by_entity
            .get(&affected_root)
            .copied()
            .unwrap_or(Vec3::ZERO);
        let root_extent_m = index
            .base_extent_m_by_entity
            .get(&affected_root)
            .copied()
            .unwrap_or(0.0);
        for entity in entities_under_root {
            let effective_position = root_position;
            let effective_extent_m = root_extent_m;
            let next_cell = cell_key(effective_position, runtime_cfg.cell_size_m);
            let previous_cell = index.cell_by_entity.get(&entity).copied();
            if previous_cell != Some(next_cell) {
                remove_entity_from_cell_index(&mut index, entity);
                index.cell_by_entity.insert(entity, next_cell);
                index
                    .entities_by_cell
                    .entry(next_cell)
                    .or_default()
                    .push(entity);
            }
            index
                .visibility_position_by_entity
                .insert(entity, effective_position);
            index
                .visibility_extent_m_by_entity
                .insert(entity, effective_extent_m);
        }
    }
}

#[allow(clippy::type_complexity)]
#[allow(clippy::too_many_arguments)]
pub fn update_network_visibility(
    mut commands: Commands<'_, '_>,
    time: Res<'_, Time>,
    runtime_cfg: Res<'_, VisibilityRuntimeConfig>,
    preparation_metrics: Res<'_, VisibilityPreparationMetrics>,
    landmark_metrics: Res<'_, VisibilityLandmarkDiscoveryMetrics>,
    mut telemetry_state: ResMut<'_, VisibilityTelemetryLogState>,
    params: VisibilityUpdateParams<'_, '_>,
) {
    let clients = params.clients;
    let cache = params.cache;
    let mut client_context_cache = params.client_context_cache;
    let mut membership_cache = params.membership_cache;
    let spatial_index = params.spatial_index;
    let visibility_registry = params.visibility_registry;
    let mut view_mode_registry = params.view_mode_registry;
    let player_entities = params.player_entities;
    let mut scratch = params.scratch;
    let observer_anchor_positions = params.observer_anchor_positions;
    let player_visibility_state = params.player_visibility_state;
    let player_landmark_state = params.player_landmark_state;
    let all_replicated = params.all_replicated;
    let mut replicated_entities = params.replicated_entities;
    let started_at = Instant::now();
    let mut client_cache_upserts = 0usize;
    let mut visible_gains = 0usize;
    let mut visible_losses = 0usize;
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

    // 1) Build policy/runtime lookup state for all replicated entities while reading
    // stable spatial state from the persistent visibility index.
    for (entity, _position, _global_transform) in &all_replicated {
        let Some(cached) = cache.by_entity.get(&entity) else {
            continue;
        };
        if let Some(definition) = cached.runtime_render_layer_definition.as_ref() {
            runtime_layer_definitions_by_id.insert(definition.layer_id.clone(), definition.clone());
        }
        scratch.all_replicated_entities.push(entity);
        if let Some(guid) = cached.guid
            && let Some(static_landmark) = cached.static_landmark.as_ref()
        {
            let discovery_padding_m = static_landmark.discovery_radius_m.unwrap_or(0.0).max(0.0)
                + if static_landmark.use_extent_for_discovery {
                    cached.entity_extent_m
                } else {
                    0.0
                };
            scratch.max_static_landmark_discovery_padding_m = scratch
                .max_static_landmark_discovery_padding_m
                .max(discovery_padding_m);
            scratch
                .static_landmarks_by_entity
                .insert(entity, (guid, static_landmark.clone()));
        }
        scratch
            .root_public_by_entity
            .insert(entity, cached.public_visibility);
        if let Some(faction) = cached.faction_id.as_ref() {
            scratch
                .root_faction_by_entity
                .insert(entity, faction.clone());
        }
        if let Some(owner) = cached.owner_player_id.as_ref() {
            let canonical_owner = owner.clone();
            scratch
                .root_owner_by_entity
                .insert(entity, canonical_owner.clone());
            scratch
                .owned_entities_by_player
                .entry(canonical_owner.clone())
                .or_default()
                .push(entity);
            if let Some(faction) = cached.faction_id.as_ref() {
                scratch
                    .player_faction_by_owner
                    .entry(canonical_owner)
                    .or_insert_with(|| faction.clone());
            }
        }
        if let Some(override_layer) = cached.pending_world_layer_override.as_ref() {
            scratch
                .pending_world_layer_override_by_entity
                .insert(entity, override_layer.clone());
        }
        if let (Some(owner), Some(range)) =
            (cached.owner_player_id.as_ref(), cached.visibility_range_m)
            && range > 0.0
        {
            scratch
                .visibility_source_candidates
                .push((entity, owner.clone(), range));
        }
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

    let all_replicated_entities = scratch.all_replicated_entities.clone();
    let live_entity_set = all_replicated_entities
        .iter()
        .copied()
        .collect::<HashSet<_>>();
    membership_cache
        .by_entity
        .retain(|entity, visible_clients| {
            visible_clients.retain(|client_entity| scratch.live_client_set.contains(client_entity));
            live_entity_set.contains(entity)
        });

    // 2) Build visibility sources from owned roots with a resolved effective visibility range.
    // Child entities contribute via root VisibilityRangeM aggregation; they are not sources.
    let visibility_source_candidates = scratch.visibility_source_candidates.clone();
    for (entity, canonical_owner, range) in &visibility_source_candidates {
        let is_root = spatial_index
            .root_entity_by_entity
            .get(entity)
            .is_some_and(|root| *root == *entity);
        if !is_root {
            continue;
        }
        let Some(position) = spatial_index.world_position_by_entity.get(entity).copied() else {
            continue;
        };
        scratch
            .visibility_sources_by_owner
            .entry(canonical_owner.clone())
            .or_default()
            .push((position, *range));
    }
    let scratch_build_ms = started_at.elapsed().as_secs_f64() * 1000.0;

    let candidate_started_at = Instant::now();
    let client_context_refresh_started_at = Instant::now();
    let live_client_count_before_retain = client_context_cache.by_client.len();
    client_context_cache
        .by_client
        .retain(|client_entity, _| scratch.live_client_set.contains(client_entity));
    let client_cache_removals_local =
        live_client_count_before_retain.saturating_sub(client_context_cache.by_client.len());
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
        let next_context = CachedClientVisibilityContext {
            player_entity_id: canonical_player_id.clone(),
            player_entity,
            observer_anchor_position,
            visibility_sources,
            discovered_static_landmarks: player_entity
                .and_then(|player_entity| {
                    player_landmark_state
                        .get(player_entity)
                        .ok()
                        .and_then(|component| {
                            component.map(|component| {
                                component.landmark_entity_ids.iter().copied().collect()
                            })
                        })
                })
                .unwrap_or_default(),
            player_faction_id,
            view_mode: local_view_mode,
            delivery_range_m: client_delivery_range_m,
        };
        let should_upsert =
            client_context_cache.by_client.get(client_entity) != Some(&next_context);
        if should_upsert {
            client_context_cache
                .by_client
                .insert(*client_entity, next_context);
            client_cache_upserts = client_cache_upserts.saturating_add(1);
        }
    }
    let client_context_refresh_ms =
        client_context_refresh_started_at.elapsed().as_secs_f64() * 1000.0;
    let client_cache_entries = client_context_cache.by_client.len();
    let client_cache_removals = client_cache_removals_local;

    for (client_entity, _) in &registered_clients {
        let Some(client_context) = client_context_cache.by_client.get(client_entity) else {
            continue;
        };
        let candidates = build_candidate_set_for_client(
            runtime_cfg.candidate_mode,
            client_context.player_entity_id.as_str(),
            client_context.observer_anchor_position,
            client_context.delivery_range_m,
            &client_context.visibility_sources,
            client_context.view_mode,
            runtime_cfg.cell_size_m,
            &scratch.all_replicated_entities,
            &scratch.owned_entities_by_player,
            &spatial_index.entities_by_cell,
        );
        let candidate_cells = build_candidate_cells_for_client(
            runtime_cfg.candidate_mode,
            client_context.observer_anchor_position,
            client_context.delivery_range_m,
            &client_context.visibility_sources,
            client_context.view_mode,
            runtime_cfg.cell_size_m,
        );
        scratch.client_states.push(ClientVisibilityComputedState {
            client_entity: *client_entity,
            candidate_entities: candidates,
            candidate_cells,
        });
    }
    let discovery_and_candidate_ms = candidate_started_at.elapsed().as_secs_f64() * 1000.0;

    let disclosure_started_at = Instant::now();
    for client_state in &scratch.client_states {
        let Some(client_context) = client_context_cache
            .by_client
            .get(&client_state.client_entity)
        else {
            continue;
        };
        let Some(player_entity) = client_context.player_entity else {
            continue;
        };
        let visibility_sources = client_context
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
            delivery_range_m: client_context.delivery_range_m,
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

    // Cache client buckets once so owner-only and owner-map fast paths do not keep
    // rediscovering the same client subsets while iterating every replicated entity.
    let mut client_entities_by_player_id = HashMap::<String, Vec<Entity>>::new();
    let mut map_mode_client_entities_by_player_id = HashMap::<String, Vec<Entity>>::new();
    for client_state in &scratch.client_states {
        let Some(client_context) = client_context_cache
            .by_client
            .get(&client_state.client_entity)
        else {
            continue;
        };
        client_entities_by_player_id
            .entry(client_context.player_entity_id.clone())
            .or_default()
            .push(client_state.client_entity);
        if matches!(client_context.view_mode, ClientLocalViewMode::Map) {
            map_mode_client_entities_by_player_id
                .entry(client_context.player_entity_id.clone())
                .or_default()
                .push(client_state.client_entity);
        }
    }

    let apply_started_at = Instant::now();
    for (entity, mut replication_state, controlled_by, runtime_world_visual_stack) in
        &mut replicated_entities
    {
        let Some(cached) = cache.by_entity.get(&entity) else {
            continue;
        };
        let current_visible_clients = membership_cache
            .by_entity
            .get(&entity)
            .cloned()
            .unwrap_or_default();
        let tracked_guid = cached.guid;
        let debug_track_this_entity =
            debug_visibility_entity_guid().is_some_and(|tracked| Some(tracked) == tracked_guid);
        let root_entity = spatial_index
            .root_entity_by_entity
            .get(&entity)
            .copied()
            .unwrap_or(entity);

        let entity_position = spatial_index
            .visibility_position_by_entity
            .get(&entity)
            .copied();
        let entity_extent_m = spatial_index
            .visibility_extent_m_by_entity
            .get(&entity)
            .copied()
            .unwrap_or(cached.entity_extent_m);
        let mut desired_visible_clients = HashSet::<Entity>::new();
        let resolved_world_layer = scratch
            .resolved_world_layer_by_entity
            .get(&entity)
            .or_else(|| scratch.resolved_world_layer_by_entity.get(&root_entity))
            .or_else(|| {
                cached
                    .pending_world_layer_override
                    .as_ref()
                    .and_then(|layer_id| runtime_layer_definitions_by_id.get(layer_id))
            });
        let prepared_policy = prepare_entity_apply_policy(
            cached,
            scratch
                .root_public_by_entity
                .get(&root_entity)
                .copied()
                .unwrap_or(false),
            scratch.root_owner_by_entity.get(&root_entity),
            scratch.root_faction_by_entity.get(&root_entity),
            entity_position,
            entity_extent_m,
            resolved_world_layer,
            runtime_world_visual_stack,
            controlled_by,
        );

        if runtime_cfg.bypass_all_filters {
            for client_state in &scratch.client_states {
                desired_visible_clients.insert(client_state.client_entity);
            }
            let current_visible_clients = membership_cache.by_entity.entry(entity).or_default();
            let gained_count = apply_visibility_membership_diff(
                &mut replication_state,
                current_visible_clients,
                &desired_visible_clients,
                &mut visible_gains,
                &mut visible_losses,
            );
            if gained_count > 0 && entity_position.is_some() {
                // Some stationary spatial roots were observed to spawn for newly visible clients
                // with a default origin pose until a later movement delta arrived. Force one
                // resend of the current replicated motion state on visibility gain so late-join
                // observers receive an authoritative bootstrap even when the entity is idle.
                queue_visibility_gain_spatial_resend(&mut commands, entity);
            }
            continue;
        }

        match &prepared_policy {
            PreparedEntityApplyPolicy::OwnerOnlyAnchor { owner_player_id } => {
                if let Some(owner_player_id) = owner_player_id.as_ref()
                    && let Some(owner_clients) =
                        client_entities_by_player_id.get(owner_player_id.as_str())
                {
                    for client_entity in owner_clients {
                        desired_visible_clients.insert(*client_entity);
                    }
                }
            }
            PreparedEntityApplyPolicy::GlobalVisible => {
                for client_state in &scratch.client_states {
                    desired_visible_clients.insert(client_state.client_entity);
                }
            }
            PreparedEntityApplyPolicy::PublicVisible(_)
            | PreparedEntityApplyPolicy::FactionVisible(_)
            | PreparedEntityApplyPolicy::DiscoveredLandmark(_)
            | PreparedEntityApplyPolicy::RangeChecked(_) => {
                // Owner-in-map-view is a stable fast path once authorization resolves
                // to Owner. Seed those clients up front so the generic loop focuses on
                // client-varying range/faction/discovery work instead of repeating the
                // same owner-map bypass check for every client candidate.
                let owner_map_clients =
                    prepared_policy
                        .owner_player_id()
                        .and_then(|owner_player_id| {
                            map_mode_client_entities_by_player_id.get(owner_player_id)
                        });
                if let Some(owner_map_clients) = owner_map_clients {
                    for client_entity in owner_map_clients {
                        desired_visible_clients.insert(*client_entity);
                    }
                }
                for client_state in &scratch.client_states {
                    let client_entity = client_state.client_entity;
                    if owner_map_clients.is_some_and(|clients| clients.contains(&client_entity)) {
                        continue;
                    }
                    let Some(client_context) = client_context_cache.by_client.get(&client_entity)
                    else {
                        continue;
                    };
                    if prepared_policy.controlled_owner_client() == Some(client_entity) {
                        // Hard guarantee: the owning client must always receive state for
                        // their currently controlled entity, independent of visibility/range.
                        desired_visible_clients.insert(client_entity);
                        continue;
                    }
                    let visibility_context =
                        PlayerVisibilityContextRef::from_cached_client_context(client_context);
                    let in_candidates = client_state.candidate_entities.contains(&entity);
                    let visibility_eval = evaluate_prepared_entity_policy_for_client(
                        &prepared_policy,
                        client_context,
                        &visibility_context,
                        matches!(visibility_context.view_mode, ClientLocalViewMode::Map),
                    );
                    if !in_candidates && !visibility_eval.bypass_candidate {
                        if debug_track_this_entity {
                            info!(
                                "vis-debug guid={} client_entity={:?} player={} in_candidates={} bypass_candidate={} owner={:?} public={} faction_visible={} entity_pos={:?} anchor_pos={:?} result=lose(candidate)",
                                tracked_guid
                                    .map(|g| g.to_string())
                                    .unwrap_or_else(|| "<none>".to_string()),
                                client_entity,
                                visibility_context.player_entity_id,
                                in_candidates,
                                visibility_eval.bypass_candidate,
                                prepared_policy.owner_player_id(),
                                prepared_policy.is_public_visibility(),
                                prepared_policy.is_faction_visibility(),
                                prepared_policy.entity_position(),
                                visibility_context.observer_anchor_position,
                            );
                        }
                        continue;
                    }
                    if visibility_eval.should_be_visible {
                        desired_visible_clients.insert(client_entity);
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
                            visibility_eval.bypass_candidate,
                            prepared_policy.owner_player_id(),
                            prepared_policy.is_public_visibility(),
                            prepared_policy.is_faction_visibility(),
                            visibility_eval.authorization,
                            visibility_eval.delivery_ok,
                            prepared_policy.entity_position(),
                            visibility_context.observer_anchor_position,
                            current_visible_clients.contains(&client_entity),
                            if visibility_eval.should_be_visible {
                                "gain/keep"
                            } else {
                                "lose"
                            }
                        );
                    }
                }
            }
        }
        let current_visible_clients = membership_cache.by_entity.entry(entity).or_default();
        let gained_count = apply_visibility_membership_diff(
            &mut replication_state,
            current_visible_clients,
            &desired_visible_clients,
            &mut visible_gains,
            &mut visible_losses,
        );
        if gained_count > 0 && entity_position.is_some() {
            queue_visibility_gain_spatial_resend(&mut commands, entity);
        }
    }
    let apply_ms = apply_started_at.elapsed().as_secs_f64() * 1000.0;
    let occupied_cells = spatial_index.entities_by_cell.len();
    let max_entities_per_cell = spatial_index
        .entities_by_cell
        .values()
        .map(Vec::len)
        .max()
        .unwrap_or(0);

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
                    .filter_map(|state| {
                        client_context_cache
                            .by_client
                            .get(&state.client_entity)
                            .map(|context| context.delivery_range_m as f64)
                    })
                    .collect::<Vec<_>>();
                if values.is_empty() {
                    (
                        runtime_cfg.delivery_range_m as f64,
                        runtime_cfg.delivery_range_m as f64,
                        runtime_cfg.delivery_range_m as f64,
                    )
                } else {
                    values.sort_by(|a, b| a.total_cmp(b));
                    let min = *values
                        .first()
                        .unwrap_or(&(runtime_cfg.delivery_range_m as f64));
                    let max = *values
                        .last()
                        .unwrap_or(&(runtime_cfg.delivery_range_m as f64));
                    let avg = values.iter().sum::<f64>() / values.len() as f64;
                    (min, avg, max)
                }
            };
            info!(
                "replication visibility summary mode={} bypass_all={} delivery_range_m[min/avg/max]={:.1}/{:.1}/{:.1} query_ms={:.2} cache_refresh_ms={:.2} cache_upserts={} cache_removals={} client_context_refresh_ms={:.2} client_cache_entries={} client_cache_upserts={} client_cache_removals={} landmark_discovery_ms={:.2} landmark_discovery_checks={} landmark_discovery_new_total={} clients={} entities={} candidates_per_client={:.1} occupied_cells={} max_entities_per_cell={} visible_gains={} visible_losses={}",
                runtime_cfg.candidate_mode.as_str(),
                runtime_cfg.bypass_all_filters,
                delivery_min,
                delivery_avg,
                delivery_max,
                started_at.elapsed().as_secs_f64() * 1000.0,
                preparation_metrics.cache_refresh_ms,
                preparation_metrics.cache_upserts,
                preparation_metrics.cache_removals,
                client_context_refresh_ms,
                client_cache_entries,
                client_cache_upserts,
                client_cache_removals,
                landmark_metrics.landmark_discovery_ms,
                landmark_metrics.discovered_checks,
                landmark_metrics.discovered_new_total,
                clients_count,
                entities_count,
                candidates_per_client,
                occupied_cells,
                max_entities_per_cell,
                visible_gains,
                visible_losses
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
            .filter_map(|state| {
                client_context_cache
                    .by_client
                    .get(&state.client_entity)
                    .map(|context| context.delivery_range_m as f64)
            })
            .collect::<Vec<_>>();
        if values.is_empty() {
            (
                runtime_cfg.delivery_range_m as f64,
                runtime_cfg.delivery_range_m as f64,
                runtime_cfg.delivery_range_m as f64,
            )
        } else {
            values.sort_by(|a, b| a.total_cmp(b));
            let min = *values
                .first()
                .unwrap_or(&(runtime_cfg.delivery_range_m as f64));
            let max = *values
                .last()
                .unwrap_or(&(runtime_cfg.delivery_range_m as f64));
            let avg = values.iter().sum::<f64>() / values.len() as f64;
            (min, avg, max)
        }
    };
    commands.insert_resource(VisibilityRuntimeMetrics {
        cache_refresh_ms: preparation_metrics.cache_refresh_ms,
        cache_upserts: preparation_metrics.cache_upserts,
        cache_removals: preparation_metrics.cache_removals,
        client_context_refresh_ms,
        client_cache_entries,
        client_cache_upserts,
        client_cache_removals,
        landmark_discovery_ms: landmark_metrics.landmark_discovery_ms,
        query_ms: started_at.elapsed().as_secs_f64() * 1000.0,
        scratch_build_ms,
        discovery_and_candidate_ms,
        disclosure_sync_ms,
        apply_ms,
        clients: clients_count,
        entities: entities_count,
        candidates_total,
        candidates_per_client,
        discovered_checks: landmark_metrics.discovered_checks,
        discovered_new_total: landmark_metrics.discovered_new_total,
        delivery_range_min_m: delivery_min,
        delivery_range_avg_m: delivery_avg,
        delivery_range_max_m: delivery_max,
        occupied_cells,
        max_entities_per_cell,
        visible_gains,
        visible_losses,
    });
}

/// Resolves the mount root entity by traversing the parent chain (MountedOn).
/// The root is used for owner/public/faction inheritance and to derive the
/// effective visibility position/extent for mounted children.
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
#[cfg_attr(not(test), allow(dead_code))]
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

#[derive(Debug, Clone, Copy, PartialEq)]
struct VisibilityEvaluation {
    authorization: Option<VisibilityAuthorization>,
    bypass_candidate: bool,
    delivery_ok: bool,
    should_be_visible: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PreparedLandmarkVisibilityPolicy {
    None,
    AlwaysKnown,
    PlayerDiscovered(uuid::Uuid),
}

#[derive(Debug, Clone, PartialEq)]
enum PreparedEntityApplyPolicy {
    OwnerOnlyAnchor { owner_player_id: Option<String> },
    GlobalVisible,
    PublicVisible(PreparedPublicEntityApplyPolicy),
    FactionVisible(PreparedFactionEntityApplyPolicy),
    DiscoveredLandmark(PreparedDiscoveredLandmarkApplyPolicy),
    RangeChecked(PreparedRangeCheckedEntityApplyPolicy),
}

#[derive(Debug, Clone, PartialEq)]
struct PreparedConditionalEntityApplyCommon {
    owner_player_id: Option<String>,
    entity_position: Option<Vec3>,
    authorization_extent_m: f32,
    controlled_owner_client: Option<Entity>,
}

#[derive(Debug, Clone, PartialEq)]
struct PreparedLandmarkDeliveryPolicy {
    visibility_policy: PreparedLandmarkVisibilityPolicy,
    discovered_extent_m: f32,
    discovered_delivery_scale: f32,
}

impl PreparedLandmarkDeliveryPolicy {
    fn is_discovered_for_client(&self, client_context: &CachedClientVisibilityContext) -> bool {
        match self.visibility_policy {
            PreparedLandmarkVisibilityPolicy::None => false,
            PreparedLandmarkVisibilityPolicy::AlwaysKnown => true,
            PreparedLandmarkVisibilityPolicy::PlayerDiscovered(guid) => {
                client_context.discovered_static_landmarks.contains(&guid)
            }
        }
    }

    fn delivery_profile_for_client(
        &self,
        client_context: &CachedClientVisibilityContext,
        default_extent_m: f32,
        default_delivery_range_m: f32,
    ) -> (bool, f32, f32) {
        if self.is_discovered_for_client(client_context) {
            (
                true,
                self.discovered_extent_m,
                default_delivery_range_m * self.discovered_delivery_scale,
            )
        } else {
            (false, default_extent_m, default_delivery_range_m)
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct PreparedPublicEntityApplyPolicy {
    common: PreparedConditionalEntityApplyCommon,
    landmark_delivery: Option<PreparedLandmarkDeliveryPolicy>,
}

#[derive(Debug, Clone, PartialEq)]
struct PreparedFactionEntityApplyPolicy {
    common: PreparedConditionalEntityApplyCommon,
    entity_faction_id: Option<String>,
    landmark_delivery: Option<PreparedLandmarkDeliveryPolicy>,
}

#[derive(Debug, Clone, PartialEq)]
struct PreparedDiscoveredLandmarkApplyPolicy {
    common: PreparedConditionalEntityApplyCommon,
    landmark_delivery: PreparedLandmarkDeliveryPolicy,
}

#[derive(Debug, Clone, PartialEq)]
struct PreparedRangeCheckedEntityApplyPolicy {
    common: PreparedConditionalEntityApplyCommon,
}

impl PreparedEntityApplyPolicy {
    fn owner_player_id(&self) -> Option<&str> {
        match self {
            Self::OwnerOnlyAnchor { owner_player_id } => owner_player_id.as_deref(),
            Self::GlobalVisible => None,
            Self::PublicVisible(policy) => policy.common.owner_player_id.as_deref(),
            Self::FactionVisible(policy) => policy.common.owner_player_id.as_deref(),
            Self::DiscoveredLandmark(policy) => policy.common.owner_player_id.as_deref(),
            Self::RangeChecked(policy) => policy.common.owner_player_id.as_deref(),
        }
    }

    fn controlled_owner_client(&self) -> Option<Entity> {
        match self {
            Self::OwnerOnlyAnchor { .. } | Self::GlobalVisible => None,
            Self::PublicVisible(policy) => policy.common.controlled_owner_client,
            Self::FactionVisible(policy) => policy.common.controlled_owner_client,
            Self::DiscoveredLandmark(policy) => policy.common.controlled_owner_client,
            Self::RangeChecked(policy) => policy.common.controlled_owner_client,
        }
    }

    fn entity_position(&self) -> Option<Vec3> {
        match self {
            Self::OwnerOnlyAnchor { .. } | Self::GlobalVisible => None,
            Self::PublicVisible(policy) => policy.common.entity_position,
            Self::FactionVisible(policy) => policy.common.entity_position,
            Self::DiscoveredLandmark(policy) => policy.common.entity_position,
            Self::RangeChecked(policy) => policy.common.entity_position,
        }
    }

    fn is_public_visibility(&self) -> bool {
        matches!(self, Self::PublicVisible(_))
    }

    fn is_faction_visibility(&self) -> bool {
        matches!(self, Self::FactionVisible(_))
    }
}

#[allow(clippy::too_many_arguments)]
fn prepare_entity_apply_policy(
    cached: &CachedVisibilityEntity,
    root_public: bool,
    root_owner_player_id: Option<&String>,
    root_faction_id: Option<&String>,
    entity_position: Option<Vec3>,
    entity_extent_m: f32,
    resolved_world_layer: Option<&RuntimeRenderLayerDefinition>,
    runtime_world_visual_stack: Option<&RuntimeWorldVisualStack>,
    controlled_by: Option<&ControlledBy>,
) -> PreparedEntityApplyPolicy {
    // Keep entity policy preparation outside the per-client apply loop. If future
    // work adds more visibility fast paths, extend these prepared buckets rather
    // than pushing more root/public/faction/landmark branching back into the hot loop.
    let is_public = cached.public_visibility || root_public;
    let mut owner_player_id = cached
        .owner_player_id
        .clone()
        .or_else(|| root_owner_player_id.cloned());
    let entity_faction_id = cached
        .faction_id
        .clone()
        .or_else(|| root_faction_id.cloned());
    if cached.is_player_tag {
        if owner_player_id.is_none() {
            owner_player_id = cached.guid.map(|guid| guid.to_string());
        }
        return PreparedEntityApplyPolicy::OwnerOnlyAnchor { owner_player_id };
    }
    if cached.is_global_render_config {
        return PreparedEntityApplyPolicy::GlobalVisible;
    }
    let landmark_policy = match (cached.static_landmark.as_ref(), cached.guid) {
        (Some(landmark), _) if landmark.always_known => {
            PreparedLandmarkVisibilityPolicy::AlwaysKnown
        }
        (Some(_), Some(guid)) => PreparedLandmarkVisibilityPolicy::PlayerDiscovered(guid),
        _ => PreparedLandmarkVisibilityPolicy::None,
    };
    let common = PreparedConditionalEntityApplyCommon {
        owner_player_id,
        entity_position,
        authorization_extent_m: entity_extent_m,
        controlled_owner_client: controlled_by.map(|binding| binding.owner),
    };
    let landmark_delivery = (!matches!(landmark_policy, PreparedLandmarkVisibilityPolicy::None))
        .then(|| PreparedLandmarkDeliveryPolicy {
            visibility_policy: landmark_policy,
            discovered_extent_m: effective_discovered_landmark_extent_m(
                entity_extent_m,
                resolved_world_layer,
                runtime_world_visual_stack,
            ),
            discovered_delivery_scale: 1.0 / runtime_layer_parallax_factor(resolved_world_layer),
        });

    if is_public {
        return PreparedEntityApplyPolicy::PublicVisible(PreparedPublicEntityApplyPolicy {
            common,
            landmark_delivery,
        });
    }
    if cached.faction_visibility {
        return PreparedEntityApplyPolicy::FactionVisible(PreparedFactionEntityApplyPolicy {
            common,
            entity_faction_id,
            landmark_delivery,
        });
    }
    if let Some(landmark_delivery) = landmark_delivery {
        return PreparedEntityApplyPolicy::DiscoveredLandmark(
            PreparedDiscoveredLandmarkApplyPolicy {
                common,
                landmark_delivery,
            },
        );
    }
    PreparedEntityApplyPolicy::RangeChecked(PreparedRangeCheckedEntityApplyPolicy { common })
}

fn evaluate_prepared_entity_policy_for_client(
    prepared_policy: &PreparedEntityApplyPolicy,
    client_context: &CachedClientVisibilityContext,
    visibility_context: &PlayerVisibilityContextRef<'_>,
    owner_bypasses_delivery_scope: bool,
) -> VisibilityEvaluation {
    match prepared_policy {
        PreparedEntityApplyPolicy::OwnerOnlyAnchor { .. }
        | PreparedEntityApplyPolicy::GlobalVisible => VisibilityEvaluation {
            authorization: None,
            bypass_candidate: false,
            delivery_ok: false,
            should_be_visible: false,
        },
        PreparedEntityApplyPolicy::PublicVisible(policy) => {
            let authorization = authorize_owner_visibility(
                visibility_context.player_entity_id,
                policy.common.owner_player_id.as_deref(),
            )
            .or(Some(VisibilityAuthorization::Public));
            let (_, delivery_extent_m, delivery_range_m) = policy
                .landmark_delivery
                .as_ref()
                .map(|landmark| {
                    landmark.delivery_profile_for_client(
                        client_context,
                        policy.common.authorization_extent_m,
                        client_context.delivery_range_m,
                    )
                })
                .unwrap_or((
                    false,
                    policy.common.authorization_extent_m,
                    client_context.delivery_range_m,
                ));
            finalize_visibility_evaluation(
                authorization,
                policy.common.entity_position,
                delivery_extent_m,
                visibility_context,
                delivery_range_m,
                owner_bypasses_delivery_scope,
            )
        }
        PreparedEntityApplyPolicy::FactionVisible(policy) => {
            let authorization = authorize_owner_visibility(
                visibility_context.player_entity_id,
                policy.common.owner_player_id.as_deref(),
            )
            .or_else(|| {
                authorize_faction_visibility(
                    policy.entity_faction_id.as_deref(),
                    visibility_context,
                )
            });
            let (_, delivery_extent_m, delivery_range_m) = policy
                .landmark_delivery
                .as_ref()
                .map(|landmark| {
                    landmark.delivery_profile_for_client(
                        client_context,
                        policy.common.authorization_extent_m,
                        client_context.delivery_range_m,
                    )
                })
                .unwrap_or((
                    false,
                    policy.common.authorization_extent_m,
                    client_context.delivery_range_m,
                ));
            finalize_visibility_evaluation(
                authorization,
                policy.common.entity_position,
                delivery_extent_m,
                visibility_context,
                delivery_range_m,
                owner_bypasses_delivery_scope,
            )
        }
        PreparedEntityApplyPolicy::DiscoveredLandmark(policy) => {
            let (is_discovered_static_landmark, delivery_extent_m, delivery_range_m) =
                policy.landmark_delivery.delivery_profile_for_client(
                    client_context,
                    policy.common.authorization_extent_m,
                    client_context.delivery_range_m,
                );
            let authorization = authorize_owner_visibility(
                visibility_context.player_entity_id,
                policy.common.owner_player_id.as_deref(),
            )
            .or_else(|| authorize_discovered_landmark_visibility(is_discovered_static_landmark));
            finalize_visibility_evaluation(
                authorization,
                policy.common.entity_position,
                delivery_extent_m,
                visibility_context,
                delivery_range_m,
                owner_bypasses_delivery_scope,
            )
        }
        PreparedEntityApplyPolicy::RangeChecked(policy) => {
            let authorization = authorize_owner_visibility(
                visibility_context.player_entity_id,
                policy.common.owner_player_id.as_deref(),
            )
            .or_else(|| {
                authorize_range_visibility(
                    policy.common.entity_position,
                    policy.common.authorization_extent_m,
                    visibility_context,
                )
            });
            finalize_visibility_evaluation(
                authorization,
                policy.common.entity_position,
                policy.common.authorization_extent_m,
                visibility_context,
                client_context.delivery_range_m,
                owner_bypasses_delivery_scope,
            )
        }
    }
}

fn finalize_visibility_evaluation(
    authorization: Option<VisibilityAuthorization>,
    entity_position: Option<Vec3>,
    delivery_extent_m: f32,
    visibility_context: &PlayerVisibilityContextRef<'_>,
    delivery_range_m: f32,
    owner_bypasses_delivery_scope: bool,
) -> VisibilityEvaluation {
    let delivery_ok = authorization.is_some_and(|authorization| {
        if owner_bypasses_delivery_scope && matches!(authorization, VisibilityAuthorization::Owner)
        {
            return true;
        }
        passes_delivery_scope(
            entity_position,
            delivery_extent_m,
            visibility_context,
            delivery_range_m,
        )
    });
    VisibilityEvaluation {
        authorization,
        bypass_candidate: authorization.is_some(),
        delivery_ok,
        should_be_visible: authorization.is_some() && delivery_ok,
    }
}

#[allow(clippy::too_many_arguments)]
#[cfg_attr(not(test), allow(dead_code))]
fn evaluate_visibility_for_client(
    player_entity_id: &str,
    owner_player_id: Option<&str>,
    is_public_visibility: bool,
    is_faction_visibility: bool,
    is_discovered_static_landmark: bool,
    entity_faction_id: Option<&str>,
    entity_position: Option<Vec3>,
    authorization_extent_m: f32,
    delivery_extent_m: f32,
    visibility_context: &PlayerVisibilityContextRef<'_>,
    delivery_range_m: f32,
    owner_bypasses_delivery_scope: bool,
) -> VisibilityEvaluation {
    let authorization = authorize_visibility(
        player_entity_id,
        owner_player_id,
        is_public_visibility,
        is_faction_visibility,
        is_discovered_static_landmark,
        entity_faction_id,
        entity_position,
        authorization_extent_m,
        visibility_context,
    );
    finalize_visibility_evaluation(
        authorization,
        entity_position,
        delivery_extent_m,
        visibility_context,
        delivery_range_m,
        owner_bypasses_delivery_scope,
    )
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
    if let Some(authorization) = authorize_owner_visibility(player_entity_id, owner_player_id) {
        return Some(authorization);
    }
    if is_faction_visibility
        && let Some(authorization) =
            authorize_faction_visibility(entity_faction_id, visibility_context)
    {
        return Some(authorization);
    }
    if is_public_visibility {
        return Some(VisibilityAuthorization::Public);
    }
    if let Some(authorization) =
        authorize_discovered_landmark_visibility(is_discovered_static_landmark)
    {
        return Some(authorization);
    }
    authorize_range_visibility(entity_position, entity_extent_m, visibility_context)
}

fn authorize_owner_visibility(
    player_entity_id: &str,
    owner_player_id: Option<&str>,
) -> Option<VisibilityAuthorization> {
    owner_player_id
        .is_some_and(|owner| owner == player_entity_id)
        .then_some(VisibilityAuthorization::Owner)
}

fn authorize_faction_visibility(
    entity_faction_id: Option<&str>,
    visibility_context: &PlayerVisibilityContextRef<'_>,
) -> Option<VisibilityAuthorization> {
    visibility_context
        .player_faction_id
        .zip(entity_faction_id)
        .is_some_and(|(player_faction, entity_faction)| player_faction == entity_faction)
        .then_some(VisibilityAuthorization::Faction)
}

fn authorize_discovered_landmark_visibility(
    is_discovered_static_landmark: bool,
) -> Option<VisibilityAuthorization> {
    is_discovered_static_landmark.then_some(VisibilityAuthorization::DiscoveredStaticLandmark)
}

fn authorize_range_visibility(
    entity_position: Option<Vec3>,
    entity_extent_m: f32,
    visibility_context: &PlayerVisibilityContextRef<'_>,
) -> Option<VisibilityAuthorization> {
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

    fn cached_client_visibility_context(
        player_entity_id: &str,
        player_faction_id: Option<&str>,
        delivery_range_m: f32,
        visibility_sources: Vec<(Vec3, f32)>,
        discovered_static_landmarks: impl IntoIterator<Item = uuid::Uuid>,
    ) -> CachedClientVisibilityContext {
        CachedClientVisibilityContext {
            player_entity_id: player_entity_id.to_string(),
            player_entity: None,
            observer_anchor_position: Some(Vec3::ZERO),
            visibility_sources,
            discovered_static_landmarks: discovered_static_landmarks.into_iter().collect(),
            player_faction_id: player_faction_id.map(str::to_string),
            view_mode: ClientLocalViewMode::Tactical,
            delivery_range_m,
        }
    }

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
            &scratch.all_replicated_entities,
            &scratch.owned_entities_by_player,
            &scratch.entities_by_cell,
        );
        let long = build_candidate_set_for_client(
            VisibilityCandidateMode::SpatialGrid,
            "11111111-1111-1111-1111-111111111111",
            Some(observer),
            2500.0,
            &[],
            ClientLocalViewMode::Tactical,
            1000.0,
            &scratch.all_replicated_entities,
            &scratch.owned_entities_by_player,
            &scratch.entities_by_cell,
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
        let visibility_sources = Vec::new();
        let visibility_context = PlayerVisibilityContextRef {
            player_entity_id: "11111111-1111-1111-1111-111111111111",
            observer_anchor_position: Some(Vec3::ZERO),
            visibility_sources: &visibility_sources,
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
        let visibility_sources = vec![(Vec3::ZERO, 900.0)];
        let visibility_context = PlayerVisibilityContextRef {
            player_entity_id: "11111111-1111-1111-1111-111111111111",
            observer_anchor_position: Some(Vec3::ZERO),
            visibility_sources: &visibility_sources,
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

    #[test]
    fn evaluate_visibility_reuses_authorization_for_candidate_and_delivery() {
        let visibility_sources = vec![(Vec3::new(1000.0, 0.0, 0.0), 200.0)];
        let visibility_context = PlayerVisibilityContextRef {
            player_entity_id: "11111111-1111-1111-1111-111111111111",
            observer_anchor_position: Some(Vec3::new(1000.0, 0.0, 0.0)),
            visibility_sources: &visibility_sources,
            player_faction_id: None,
            view_mode: ClientLocalViewMode::Tactical,
        };

        let visible = evaluate_visibility_for_client(
            visibility_context.player_entity_id,
            None,
            false,
            false,
            false,
            None,
            Some(Vec3::new(1050.0, 0.0, 0.0)),
            0.0,
            0.0,
            &visibility_context,
            DEFAULT_VIEW_RANGE_M,
            false,
        );
        assert_eq!(visible.authorization, Some(VisibilityAuthorization::Range));
        assert!(visible.bypass_candidate);
        assert!(visible.delivery_ok);
        assert!(visible.should_be_visible);

        let hidden = evaluate_visibility_for_client(
            visibility_context.player_entity_id,
            Some(visibility_context.player_entity_id),
            false,
            false,
            false,
            None,
            Some(Vec3::new(5_000.0, 0.0, 0.0)),
            0.0,
            0.0,
            &visibility_context,
            DEFAULT_VIEW_RANGE_M,
            false,
        );
        assert_eq!(hidden.authorization, Some(VisibilityAuthorization::Owner));
        assert!(hidden.bypass_candidate);
        assert!(!hidden.delivery_ok);
        assert!(!hidden.should_be_visible);

        let map_owner = evaluate_visibility_for_client(
            visibility_context.player_entity_id,
            Some(visibility_context.player_entity_id),
            false,
            false,
            false,
            None,
            Some(Vec3::new(5_000.0, 0.0, 0.0)),
            0.0,
            0.0,
            &PlayerVisibilityContextRef {
                view_mode: ClientLocalViewMode::Map,
                ..visibility_context
            },
            DEFAULT_VIEW_RANGE_M,
            true,
        );
        assert_eq!(
            map_owner.authorization,
            Some(VisibilityAuthorization::Owner)
        );
        assert!(map_owner.bypass_candidate);
        assert!(map_owner.delivery_ok);
        assert!(map_owner.should_be_visible);
    }

    #[test]
    fn prepare_entity_apply_policy_classifies_special_and_conditional_entities() {
        let player_anchor_guid = uuid::Uuid::new_v4();
        let player_anchor = CachedVisibilityEntity {
            guid: Some(player_anchor_guid),
            is_player_tag: true,
            ..Default::default()
        };
        let PreparedEntityApplyPolicy::OwnerOnlyAnchor { owner_player_id } =
            prepare_entity_apply_policy(
                &player_anchor,
                false,
                None,
                None,
                Some(Vec3::ZERO),
                4.0,
                None,
                None,
                None,
            )
        else {
            panic!("expected owner-only anchor policy");
        };
        let expected_owner_id = player_anchor_guid.to_string();
        assert_eq!(owner_player_id.as_deref(), Some(expected_owner_id.as_str()));

        let global = CachedVisibilityEntity {
            is_global_render_config: true,
            ..Default::default()
        };
        assert!(matches!(
            prepare_entity_apply_policy(
                &global,
                false,
                None,
                None,
                Some(Vec3::ZERO),
                4.0,
                None,
                None,
                None,
            ),
            PreparedEntityApplyPolicy::GlobalVisible
        ));

        let public_landmark = CachedVisibilityEntity {
            guid: Some(uuid::Uuid::new_v4()),
            static_landmark: Some(StaticLandmark {
                kind: "Landmark".to_string(),
                discoverable: true,
                always_known: false,
                discovery_radius_m: None,
                use_extent_for_discovery: false,
            }),
            public_visibility: true,
            ..Default::default()
        };
        let PreparedEntityApplyPolicy::PublicVisible(policy) = prepare_entity_apply_policy(
            &public_landmark,
            false,
            None,
            None,
            Some(Vec3::ZERO),
            6.0,
            None,
            None,
            None,
        ) else {
            panic!("expected public-visible policy");
        };
        assert!(matches!(
            policy
                .landmark_delivery
                .as_ref()
                .map(|landmark| landmark.visibility_policy),
            Some(PreparedLandmarkVisibilityPolicy::PlayerDiscovered(_))
        ));
        assert_eq!(policy.common.authorization_extent_m, 6.0);

        let faction_visible = CachedVisibilityEntity {
            faction_visibility: true,
            faction_id: Some("alpha".to_string()),
            ..Default::default()
        };
        let PreparedEntityApplyPolicy::FactionVisible(policy) = prepare_entity_apply_policy(
            &faction_visible,
            false,
            None,
            None,
            Some(Vec3::ZERO),
            8.0,
            None,
            None,
            None,
        ) else {
            panic!("expected faction-visible policy");
        };
        assert_eq!(policy.entity_faction_id.as_deref(), Some("alpha"));
        assert!(policy.landmark_delivery.is_none());

        let discovered_landmark_guid = uuid::Uuid::new_v4();
        let discovered_landmark = CachedVisibilityEntity {
            guid: Some(discovered_landmark_guid),
            static_landmark: Some(StaticLandmark {
                kind: "Landmark".to_string(),
                discoverable: true,
                always_known: false,
                discovery_radius_m: None,
                use_extent_for_discovery: false,
            }),
            ..Default::default()
        };
        let PreparedEntityApplyPolicy::DiscoveredLandmark(policy) = prepare_entity_apply_policy(
            &discovered_landmark,
            false,
            None,
            None,
            Some(Vec3::ZERO),
            5.0,
            None,
            None,
            None,
        ) else {
            panic!("expected discovered-landmark policy");
        };
        assert!(matches!(
            policy.landmark_delivery.visibility_policy,
            PreparedLandmarkVisibilityPolicy::PlayerDiscovered(guid)
                if guid == discovered_landmark_guid
        ));

        let range_checked = CachedVisibilityEntity::default();
        assert!(matches!(
            prepare_entity_apply_policy(
                &range_checked,
                false,
                None,
                None,
                Some(Vec3::ZERO),
                3.0,
                None,
                None,
                None,
            ),
            PreparedEntityApplyPolicy::RangeChecked(_)
        ));
    }

    #[test]
    fn prepared_policy_evaluation_preserves_specialized_authorization_paths() {
        let public_policy = prepare_entity_apply_policy(
            &CachedVisibilityEntity {
                public_visibility: true,
                ..Default::default()
            },
            false,
            None,
            None,
            Some(Vec3::new(100.0, 0.0, 0.0)),
            0.0,
            None,
            None,
            None,
        );
        let public_client =
            cached_client_visibility_context("player-public", None, 300.0, Vec::new(), []);
        let public_visibility_context =
            PlayerVisibilityContextRef::from_cached_client_context(&public_client);
        let public_eval = evaluate_prepared_entity_policy_for_client(
            &public_policy,
            &public_client,
            &public_visibility_context,
            false,
        );
        assert_eq!(
            public_eval.authorization,
            Some(VisibilityAuthorization::Public)
        );
        assert!(public_eval.bypass_candidate);
        assert!(public_eval.should_be_visible);

        let faction_policy = prepare_entity_apply_policy(
            &CachedVisibilityEntity {
                faction_visibility: true,
                faction_id: Some("alpha".to_string()),
                ..Default::default()
            },
            false,
            None,
            None,
            Some(Vec3::new(100.0, 0.0, 0.0)),
            0.0,
            None,
            None,
            None,
        );
        let faction_client = cached_client_visibility_context(
            "player-faction",
            Some("alpha"),
            300.0,
            Vec::new(),
            [],
        );
        let faction_visibility_context =
            PlayerVisibilityContextRef::from_cached_client_context(&faction_client);
        let faction_eval = evaluate_prepared_entity_policy_for_client(
            &faction_policy,
            &faction_client,
            &faction_visibility_context,
            false,
        );
        assert_eq!(
            faction_eval.authorization,
            Some(VisibilityAuthorization::Faction)
        );
        assert!(faction_eval.should_be_visible);

        let other_faction_client = cached_client_visibility_context(
            "player-faction-other",
            Some("beta"),
            300.0,
            Vec::new(),
            [],
        );
        let other_faction_visibility_context =
            PlayerVisibilityContextRef::from_cached_client_context(&other_faction_client);
        let other_faction_eval = evaluate_prepared_entity_policy_for_client(
            &faction_policy,
            &other_faction_client,
            &other_faction_visibility_context,
            false,
        );
        assert_eq!(other_faction_eval.authorization, None);
        assert!(!other_faction_eval.bypass_candidate);
        assert!(!other_faction_eval.should_be_visible);

        let discovered_landmark_guid = uuid::Uuid::new_v4();
        let discovered_policy = prepare_entity_apply_policy(
            &CachedVisibilityEntity {
                guid: Some(discovered_landmark_guid),
                static_landmark: Some(StaticLandmark {
                    kind: "Landmark".to_string(),
                    discoverable: true,
                    always_known: false,
                    discovery_radius_m: None,
                    use_extent_for_discovery: false,
                }),
                ..Default::default()
            },
            false,
            None,
            None,
            Some(Vec3::new(100.0, 0.0, 0.0)),
            0.0,
            None,
            None,
            None,
        );
        let discovered_client = cached_client_visibility_context(
            "player-discovered",
            None,
            300.0,
            Vec::new(),
            [discovered_landmark_guid],
        );
        let discovered_visibility_context =
            PlayerVisibilityContextRef::from_cached_client_context(&discovered_client);
        let discovered_eval = evaluate_prepared_entity_policy_for_client(
            &discovered_policy,
            &discovered_client,
            &discovered_visibility_context,
            false,
        );
        assert_eq!(
            discovered_eval.authorization,
            Some(VisibilityAuthorization::DiscoveredStaticLandmark)
        );
        assert!(discovered_eval.should_be_visible);

        let undiscovered_client =
            cached_client_visibility_context("player-undiscovered", None, 300.0, Vec::new(), []);
        let undiscovered_visibility_context =
            PlayerVisibilityContextRef::from_cached_client_context(&undiscovered_client);
        let undiscovered_eval = evaluate_prepared_entity_policy_for_client(
            &discovered_policy,
            &undiscovered_client,
            &undiscovered_visibility_context,
            false,
        );
        assert_eq!(undiscovered_eval.authorization, None);
        assert!(!undiscovered_eval.should_be_visible);

        let range_policy = prepare_entity_apply_policy(
            &CachedVisibilityEntity::default(),
            false,
            None,
            None,
            Some(Vec3::new(150.0, 0.0, 0.0)),
            10.0,
            None,
            None,
            None,
        );
        let range_client = cached_client_visibility_context(
            "player-range",
            None,
            300.0,
            vec![(Vec3::ZERO, 200.0)],
            [],
        );
        let range_visibility_context =
            PlayerVisibilityContextRef::from_cached_client_context(&range_client);
        let range_eval = evaluate_prepared_entity_policy_for_client(
            &range_policy,
            &range_client,
            &range_visibility_context,
            false,
        );
        assert_eq!(
            range_eval.authorization,
            Some(VisibilityAuthorization::Range)
        );
        assert!(range_eval.should_be_visible);
    }

    #[test]
    fn membership_diff_applies_only_changes() {
        let client_a = Entity::from_raw_u32(1).expect("valid entity id");
        let client_b = Entity::from_raw_u32(2).expect("valid entity id");
        let mut replication_state = ReplicationState::default();
        let mut current_visible_clients = HashSet::from([client_a]);
        replication_state.gain_visibility(client_a);
        let desired_visible_clients = HashSet::from([client_b]);
        let mut visible_gains = 0usize;
        let mut visible_losses = 0usize;

        let gained_count = apply_visibility_membership_diff(
            &mut replication_state,
            &mut current_visible_clients,
            &desired_visible_clients,
            &mut visible_gains,
            &mut visible_losses,
        );
        assert_eq!(gained_count, 1);

        assert_eq!(visible_gains, 1);
        assert_eq!(visible_losses, 1);
        assert!(replication_state.is_visible(client_b));
        assert!(!replication_state.is_visible(client_a));
        assert_eq!(current_visible_clients, desired_visible_clients);
    }

    #[test]
    fn spatial_index_rebuild_check_accepts_matching_cached_entry() {
        let entity = Entity::from_raw_u32(11).expect("valid entity id");
        let guid = uuid::Uuid::new_v4();
        let mut index = VisibilitySpatialIndex::default();
        index.entity_by_guid.insert(guid, entity);
        index.world_position_by_entity.insert(entity, Vec3::ZERO);
        index.base_extent_m_by_entity.insert(entity, 5.0);
        index
            .visibility_position_by_entity
            .insert(entity, Vec3::ZERO);
        index.visibility_extent_m_by_entity.insert(entity, 5.0);
        index.root_entity_by_entity.insert(entity, entity);
        index.cell_by_entity.insert(entity, (0, 0));
        index.entities_by_cell.insert((0, 0), vec![entity]);
        index
            .entities_by_root
            .entry(entity)
            .or_default()
            .insert(entity);

        let mut cache = VisibilityEntityCache::default();
        cache.by_entity.insert(
            entity,
            CachedVisibilityEntity {
                guid: Some(guid),
                entity_extent_m: 5.0,
                ..Default::default()
            },
        );

        assert!(!spatial_index_requires_full_rebuild(&index, &cache, entity));
    }

    #[test]
    fn spatial_index_rebuild_check_rejects_missing_index_entry() {
        let entity = Entity::from_raw_u32(12).expect("valid entity id");
        let guid = uuid::Uuid::new_v4();
        let mut cache = VisibilityEntityCache::default();
        cache.by_entity.insert(
            entity,
            CachedVisibilityEntity {
                guid: Some(guid),
                entity_extent_m: 5.0,
                ..Default::default()
            },
        );

        assert!(spatial_index_requires_full_rebuild(
            &VisibilitySpatialIndex::default(),
            &cache,
            entity,
        ));
    }

    #[test]
    fn resolved_parent_guid_prefers_mounted_on() {
        let mounted_parent = uuid::Uuid::new_v4();
        let fallback_parent = uuid::Uuid::new_v4();
        assert_eq!(
            resolved_parent_guid(
                Some(&MountedOn {
                    parent_entity_id: mounted_parent,
                    hardpoint_id: "test-hardpoint".to_string(),
                }),
                Some(&ParentGuid(fallback_parent))
            ),
            Some(mounted_parent)
        );
    }

    #[test]
    fn fullscreen_phase_runtime_layer_is_global_render_config() {
        let definition = RuntimeRenderLayerDefinition {
            layer_id: "bg_starfield".to_string(),
            phase: RENDER_PHASE_FULLSCREEN_BACKGROUND.to_string(),
            material_domain: RENDER_DOMAIN_FULLSCREEN.to_string(),
            enabled: true,
            ..default()
        };
        assert!(is_global_render_config_entity(false, Some(&definition)));
        assert!(is_global_render_config_entity(true, None));
    }
}
