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
    local_view_delivery_metrics: Res<'w, ClientLocalViewDeliveryMetrics>,
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
        (
            Entity,
            Option<&'static Position>,
            Option<&'static WorldPosition>,
            &'static GlobalTransform,
        ),
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
    notification_queue: ResMut<'w, NotificationCommandQueue>,
    player_landmark_state:
        Query<'w, 's, Option<&'static mut DiscoveredStaticLandmarks>, With<PlayerTag>>,
    landmark_notification_meta: Query<
        'w,
        's,
        (
            Option<&'static DisplayName>,
            Option<&'static MapIcon>,
            Option<&'static WorldPosition>,
        ),
    >,
    all_replicated: Query<
        'w,
        's,
        (
            Entity,
            Option<&'static Position>,
            Option<&'static WorldPosition>,
            &'static GlobalTransform,
            Option<&'static SignalSignature>,
        ),
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
        (
            Entity,
            Option<&'static Position>,
            Option<&'static WorldPosition>,
            &'static GlobalTransform,
        ),
        With<Replicate>,
    >,
    changed_replicated: Query<
        'w,
        's,
        (
            Entity,
            Option<&'static Position>,
            Option<&'static WorldPosition>,
            &'static GlobalTransform,
        ),
        (
            With<Replicate>,
            Or<(
                Added<Replicate>,
                Changed<GlobalTransform>,
                Changed<Position>,
                Changed<WorldPosition>,
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
    delivery_range_max_m: f32,
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
    pub delivery_range_clamped_requests_total: u64,
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

#[derive(Debug, Resource, Default, Clone)]
pub struct ClientLocalViewDeliveryMetrics {
    pub clamped_requests_total: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct ClientLocalViewSettings {
    pub view_mode: ClientLocalViewMode,
    pub delivery_range_m: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct SanitizedDeliveryRange {
    range_m: f32,
    was_clamped: bool,
}

fn delivery_range_cap_for_view_mode(
    _view_mode: ClientLocalViewMode,
    runtime_cfg: &VisibilityRuntimeConfig,
) -> f32 {
    runtime_cfg
        .delivery_range_max_m
        .max(CLIENT_DELIVERY_RANGE_MIN_M)
}

fn sanitize_client_delivery_range_m(
    requested_range_m: f32,
    view_mode: ClientLocalViewMode,
    runtime_cfg: &VisibilityRuntimeConfig,
) -> SanitizedDeliveryRange {
    let fallback_range_m = runtime_cfg.delivery_range_m.clamp(
        CLIENT_DELIVERY_RANGE_MIN_M,
        delivery_range_cap_for_view_mode(view_mode, runtime_cfg),
    );
    let sanitized = if requested_range_m.is_finite() {
        requested_range_m.clamp(
            CLIENT_DELIVERY_RANGE_MIN_M,
            delivery_range_cap_for_view_mode(view_mode, runtime_cfg),
        )
    } else {
        fallback_range_m
    };

    SanitizedDeliveryRange {
        range_m: sanitized,
        was_clamped: sanitized.to_bits() != requested_range_m.to_bits(),
    }
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
    let delivery_range_max_m = delivery_range_max_m_from_env();
    let delivery_range_m = delivery_range_m_from_env().clamp(
        CLIENT_DELIVERY_RANGE_MIN_M,
        delivery_range_max_m.max(CLIENT_DELIVERY_RANGE_MIN_M),
    );
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
        delivery_range_max_m,
        cell_size_m,
        landmark_discovery_interval_s: DEFAULT_LANDMARK_DISCOVERY_INTERVAL_S,
        bypass_all_filters: bypass_all_visibility_filters_from_env(),
    });
    app.insert_resource(VisibilityTelemetryLogState::default());
    app.insert_resource(MotionReplicationDiagnosticsLogState::default());
    app.insert_resource(VisibilityPreparationMetrics::default());
    app.insert_resource(VisibilityLandmarkDiscoveryMetrics::default());
    app.insert_resource(VisibilityRuntimeMetrics::default());
    app.insert_resource(ClientLocalViewModeRegistry::default());
    app.insert_resource(ClientLocalViewDeliveryMetrics::default());
}

#[derive(Resource, Default)]
pub struct MotionReplicationDiagnosticsLogState {
    last_logged_at_s: f64,
}

