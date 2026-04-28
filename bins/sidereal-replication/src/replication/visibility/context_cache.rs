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

    pub fn visible_clients(&self, entity: Entity) -> Option<&HashSet<Entity>> {
        self.by_entity.get(&entity)
    }

    pub fn remove_visible_client(&mut self, entity: Entity, client_entity: Entity) {
        let Some(visible_clients) = self.by_entity.get_mut(&entity) else {
            return;
        };
        visible_clients.remove(&client_entity);
        if visible_clients.is_empty() {
            self.by_entity.remove(&entity);
        }
    }

    #[cfg(test)]
    pub fn replace_visible_clients(&mut self, entity: Entity, clients: HashSet<Entity>) {
        self.by_entity.insert(entity, clients);
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

    pub fn entities_under_root(&self, root: Entity) -> Option<Vec<Entity>> {
        let mut entities = self
            .entities_by_root
            .get(&root)?
            .iter()
            .copied()
            .collect::<Vec<_>>();
        entities.sort_by_key(|entity| entity.to_bits());
        Some(entities)
    }

    #[cfg(test)]
    pub fn replace_entities_under_root(&mut self, root: Entity, entities: HashSet<Entity>) {
        self.entities_by_root.insert(root, entities);
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

type StaticLandmarkCacheEntry = (uuid::Uuid, StaticLandmark, Option<SignalSignature>);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LandmarkDiscoveryCause {
    Direct,
    Signal,
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
    static_landmarks_by_entity: HashMap<Entity, StaticLandmarkCacheEntry>,
    max_static_landmark_discovery_padding_m: f32,
    client_states: Vec<ClientVisibilityComputedState>,
}
