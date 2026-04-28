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

