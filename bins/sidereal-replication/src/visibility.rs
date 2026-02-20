use bevy::prelude::*;
use lightyear::prelude::NetworkTarget;
use std::collections::{HashMap, HashSet};

use sidereal_net::WorldStateDelta;

pub const DEFAULT_VIEW_RANGE_M: f32 = 300.0;

#[derive(Resource, Default)]
pub struct ClientVisibilityRegistry {
    pub player_entity_id_by_client: HashMap<Entity, String>,
}

impl ClientVisibilityRegistry {
    pub fn register_client(&mut self, client_entity: Entity, player_entity_id: String) {
        self.player_entity_id_by_client
            .insert(client_entity, player_entity_id);
    }

    #[allow(dead_code)]
    pub fn unregister_client(&mut self, client_entity: Entity) {
        self.player_entity_id_by_client.remove(&client_entity);
    }

    pub fn get_player_id(&self, client_entity: Entity) -> Option<&str> {
        self.player_entity_id_by_client
            .get(&client_entity)
            .map(|s| s.as_str())
    }
}

/// Tracks position of each player's currently controlled entity for spatial queries
#[derive(Resource, Default)]
pub struct ClientControlledEntityPositionMap {
    pub position_by_player_entity_id: HashMap<String, Vec3>,
}

impl ClientControlledEntityPositionMap {
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

#[derive(Resource, Default)]
pub struct ClientVisibilityHistory {
    pub visible_entities_by_client: HashMap<Entity, HashSet<String>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisibilityScope {
    Authenticated,
    None,
}

#[derive(Debug, Clone)]
pub struct VisibilityContext {
    pub scope: VisibilityScope,
    pub player_entity_id: Option<String>,
    pub observer_position: Option<Vec3>,
    pub view_range_m: f32,
}

impl VisibilityContext {
    pub fn authenticated(player_entity_id: String, observer_position: Option<Vec3>) -> Self {
        Self {
            scope: VisibilityScope::Authenticated,
            player_entity_id: Some(player_entity_id),
            observer_position,
            view_range_m: DEFAULT_VIEW_RANGE_M,
        }
    }

    pub fn none() -> Self {
        Self {
            scope: VisibilityScope::None,
            player_entity_id: None,
            observer_position: None,
            view_range_m: 0.0,
        }
    }
}

const ALWAYS_VISIBLE_PROPERTIES: &[&str] = &[
    "entity_id",
    "position_m",
    "velocity_mps",
    "heading_rad",
    "display_name",
    "ship_tag",
    "module_tag",
    "mounted_on_entity_id",
    "parent_entity_id",
    "size_m",
    "collision_aabb_m",
    "mass_kg",
    "asset_id",
    "starfield_shader_asset_id",
];

#[allow(dead_code)]
const OWNER_ONLY_PROPERTIES: &[&str] = &[
    "health",
    "owner_entity_id",
    "shard_assignment",
    "fuel",
    "thrust_mps2",
    "turn_rad_per_sec",
    "hardpoint_id",
];

pub fn visibility_context_for_client(
    client_entity: Entity,
    registry: &ClientVisibilityRegistry,
    positions: &ClientControlledEntityPositionMap,
) -> VisibilityContext {
    if std::env::var("REPLICATION_VISIBILITY_MODE")
        .is_ok_and(|mode| mode.eq_ignore_ascii_case("none"))
    {
        return VisibilityContext::none();
    }

    if let Some(player_id) = registry.get_player_id(client_entity) {
        let obs_pos = positions.get_position(player_id);
        VisibilityContext::authenticated(player_id.to_string(), obs_pos)
    } else {
        VisibilityContext::none()
    }
}

pub fn apply_visibility_filter(
    world: &WorldStateDelta,
    ctx: &VisibilityContext,
) -> Option<WorldStateDelta> {
    match ctx.scope {
        VisibilityScope::None => None,
        VisibilityScope::Authenticated => {
            let player_id = ctx.player_entity_id.as_ref()?;
            Some(filter_world_for_client(world, player_id, ctx))
        }
    }
}

/// Extract position from entity properties JSON
fn extract_position(properties: &serde_json::Value) -> Option<Vec3> {
    let arr = properties.get("position_m")?.as_array()?;
    if arr.len() == 3 {
        Some(Vec3::new(
            arr[0].as_f64()? as f32,
            arr[1].as_f64()? as f32,
            arr[2].as_f64()? as f32,
        ))
    } else {
        None
    }
}

fn filter_world_for_client(
    world: &WorldStateDelta,
    player_entity_id: &str,
    ctx: &VisibilityContext,
) -> WorldStateDelta {
    let mut filtered_updates = Vec::new();
    let ownership = world
        .updates
        .iter()
        .map(|update| {
            (
                update.entity_id.clone(),
                entity_is_owned_by(update, player_entity_id),
            )
        })
        .collect::<HashMap<_, _>>();

    // Authorization scope: union of ranges from all owned entities (ships without scanners get default 300m).
    let mut authorization_anchors = Vec::<(Vec3, f32)>::new();
    for update in &world.updates {
        if !ownership.get(&update.entity_id).copied().unwrap_or(false) {
            continue;
        }
        if let Some(pos) = extract_position(&update.properties) {
            let range = DEFAULT_VIEW_RANGE_M + scanner_extension_m(update);
            authorization_anchors.push((pos, range.max(DEFAULT_VIEW_RANGE_M)));
        }
    }

    for update in &world.updates {
        if update.removed {
            filtered_updates.push(update.clone());
            continue;
        }

        let is_owned = ownership.get(&update.entity_id).copied().unwrap_or(false);
        let entity_pos = extract_position(&update.properties);

        // Authorization scope: what the player is allowed to know.
        let authorized = if is_owned {
            true
        } else if let Some(pos) = entity_pos {
            authorization_anchors
                .iter()
                .any(|(anchor_pos, range)| (pos - *anchor_pos).length() <= *range)
        } else {
            false
        };
        if !authorized {
            continue;
        }

        // Delivery scope: what this active client session receives now (focus stream culling).
        let in_delivery_focus =
            if let (Some(obs_pos), Some(pos)) = (ctx.observer_position, entity_pos) {
                (pos - obs_pos).length() <= ctx.view_range_m
            } else {
                // Keep owned/attached entities with no spatial data available.
                is_owned
            };
        if !in_delivery_focus {
            continue;
        }

        if is_owned {
            filtered_updates.push(update.clone());
        } else {
            let mut redacted = update.clone();
            if let Some(obj) = redacted.properties.as_object_mut() {
                obj.retain(|key, _| is_property_always_visible(key));
            }
            redacted.components.clear();

            if let Some(obj) = redacted.properties.as_object()
                && !obj.is_empty()
            {
                filtered_updates.push(redacted);
            }
        }
    }

    WorldStateDelta {
        updates: filtered_updates,
    }
}

fn entity_is_owned_by(update: &sidereal_net::WorldDeltaEntity, player_entity_id: &str) -> bool {
    update.components.iter().any(|comp| {
        comp.component_kind == "owner_id"
            && owner_id_from_component_properties(&comp.properties)
                .map(|owner_id| owner_id == player_entity_id)
                .unwrap_or(false)
    })
}

fn scanner_extension_m(update: &sidereal_net::WorldDeltaEntity) -> f32 {
    // Scanner components are not yet fully wired in v3, but keep this hook now so
    // authorization range can immediately expand once scanner_range_m is persisted.
    update
        .properties
        .get("scanner_range_m")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0) as f32
}

fn is_property_always_visible(property_name: &str) -> bool {
    ALWAYS_VISIBLE_PROPERTIES.contains(&property_name)
}

fn owner_id_from_component_properties(props: &serde_json::Value) -> Option<&str> {
    if let Some(raw) = props.as_str() {
        return Some(raw);
    }

    let obj = props.as_object()?;
    for value in obj.values() {
        if let Some(raw) = value.as_str() {
            return Some(raw);
        }
        if let Some(inner_obj) = value.as_object()
            && let Some(raw) = inner_obj.get("0").and_then(|v| v.as_str())
        {
            return Some(raw);
        }
    }
    None
}

pub fn delivery_target_for_session(
    ctx: &VisibilityContext,
    peer_id: lightyear::prelude::PeerId,
) -> NetworkTarget {
    match ctx.scope {
        VisibilityScope::Authenticated => NetworkTarget::Single(peer_id),
        VisibilityScope::None => NetworkTarget::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sidereal_net::{WorldComponentDelta, WorldDeltaEntity};

    fn make_test_entity(
        entity_id: &str,
        owner_id: Option<&str>,
        has_health: bool,
        position: [f32; 3],
    ) -> WorldDeltaEntity {
        let mut properties = serde_json::json!({
            "entity_id": entity_id,
            "position_m": position,
        });

        if has_health {
            properties["health"] = serde_json::json!(1000.0);
        }

        let mut components = vec![];

        if let Some(owner) = owner_id {
            components.push(WorldComponentDelta {
                component_id: format!("{}:owner_id", entity_id),
                component_kind: "owner_id".to_string(),
                properties: serde_json::json!(owner),
            });
        }

        WorldDeltaEntity {
            entity_id: entity_id.to_string(),
            labels: vec!["Entity".to_string()],
            properties,
            components,
            removed: false,
        }
    }

    #[test]
    fn authenticated_client_sees_owned_entities_fully() {
        let world = WorldStateDelta {
            updates: vec![
                make_test_entity("ship:1", Some("player:alice"), true, [0.0, 0.0, 0.0]),
                make_test_entity("ship:2", Some("player:bob"), true, [10.0, 0.0, 0.0]),
            ],
        };

        let ctx = VisibilityContext::authenticated("player:alice".to_string(), Some(Vec3::ZERO));
        let filtered = apply_visibility_filter(&world, &ctx).unwrap();

        let own_ship = filtered
            .updates
            .iter()
            .find(|e| e.entity_id == "ship:1")
            .unwrap();
        assert!(own_ship.properties.get("health").is_some());

        let other_ship = filtered
            .updates
            .iter()
            .find(|e| e.entity_id == "ship:2")
            .unwrap();
        assert!(other_ship.properties.get("position_m").is_some());
        assert!(other_ship.properties.get("health").is_none());
    }

    #[test]
    fn range_filter_excludes_distant_entities() {
        let world = WorldStateDelta {
            updates: vec![
                make_test_entity("ship:1", Some("player:alice"), true, [0.0, 0.0, 0.0]),
                make_test_entity("ship:2", Some("player:bob"), true, [10.0, 0.0, 0.0]),
                make_test_entity("ship:3", Some("player:carol"), true, [500.0, 0.0, 0.0]),
            ],
        };

        let ctx = VisibilityContext::authenticated("player:alice".to_string(), Some(Vec3::ZERO));
        let filtered = apply_visibility_filter(&world, &ctx).unwrap();

        assert!(
            filtered.updates.iter().any(|e| e.entity_id == "ship:1"),
            "owned ship always included"
        );
        assert!(
            filtered.updates.iter().any(|e| e.entity_id == "ship:2"),
            "nearby ship included"
        );
        assert!(
            !filtered.updates.iter().any(|e| e.entity_id == "ship:3"),
            "distant ship excluded"
        );
    }

    #[test]
    fn delivery_scope_culls_far_owned_entities() {
        let world = WorldStateDelta {
            updates: vec![make_test_entity(
                "ship:1",
                Some("player:alice"),
                true,
                [9999.0, 0.0, 0.0],
            )],
        };

        let ctx = VisibilityContext::authenticated("player:alice".to_string(), Some(Vec3::ZERO));
        let filtered = apply_visibility_filter(&world, &ctx).unwrap();
        assert!(filtered.updates.is_empty());
    }

    #[test]
    fn authorization_can_differ_from_delivery_scope() {
        let mut anchor = make_test_entity(
            "ship:anchor",
            Some("player:alice"),
            true,
            [1000.0, 0.0, 0.0],
        );
        anchor.properties["scanner_range_m"] = serde_json::json!(900.0);
        let world = WorldStateDelta {
            updates: vec![
                make_test_entity("ship:focus", Some("player:alice"), true, [0.0, 0.0, 0.0]),
                anchor,
                make_test_entity("ship:target", Some("player:bob"), true, [1800.0, 0.0, 0.0]),
            ],
        };

        let ctx = VisibilityContext::authenticated("player:alice".to_string(), Some(Vec3::ZERO));
        let filtered = apply_visibility_filter(&world, &ctx).unwrap();
        assert!(
            !filtered
                .updates
                .iter()
                .any(|e| e.entity_id == "ship:target"),
            "authorized by remote owned scanner anchor, but not delivered to focus stream"
        );
    }

    #[test]
    fn unauthenticated_context_returns_none() {
        let world = WorldStateDelta {
            updates: vec![make_test_entity(
                "ship:1",
                Some("player:alice"),
                true,
                [0.0, 0.0, 0.0],
            )],
        };

        let ctx = VisibilityContext::none();
        let filtered = apply_visibility_filter(&world, &ctx);

        assert!(filtered.is_none());
    }

    #[test]
    fn registry_tracks_client_player_mapping() {
        let mut registry = ClientVisibilityRegistry::default();
        let client = Entity::from_bits(42);

        registry.register_client(client, "player:alice".to_string());
        assert_eq!(registry.get_player_id(client), Some("player:alice"));

        registry.unregister_client(client);
        assert_eq!(registry.get_player_id(client), None);
    }

    #[test]
    fn always_visible_properties_are_recognized() {
        assert!(is_property_always_visible("entity_id"));
        assert!(is_property_always_visible("position_m"));
        assert!(is_property_always_visible("heading_rad"));
        assert!(is_property_always_visible("display_name"));
        assert!(!is_property_always_visible("health"));
        assert!(!is_property_always_visible("fuel"));
    }

    #[test]
    fn owner_id_parses_from_enveloped_payload() {
        let props = serde_json::json!({
            "sidereal_game::generated::components::OwnerId": "player:alice"
        });
        assert_eq!(
            owner_id_from_component_properties(&props),
            Some("player:alice")
        );
    }

    #[test]
    fn visibility_scope_types_exist() {
        let auth = VisibilityContext::authenticated("player:test".to_string(), None);
        assert_eq!(auth.scope, VisibilityScope::Authenticated);

        let none = VisibilityContext::none();
        assert_eq!(none.scope, VisibilityScope::None);
    }

    #[test]
    fn controlled_entity_position_map_tracks_positions() {
        let mut map = ClientControlledEntityPositionMap::default();
        map.update_position("player:alice", Vec3::new(100.0, 200.0, 0.0));

        assert_eq!(
            map.get_position("player:alice"),
            Some(Vec3::new(100.0, 200.0, 0.0))
        );
        assert_eq!(map.get_position("player:bob"), None);
    }
}
