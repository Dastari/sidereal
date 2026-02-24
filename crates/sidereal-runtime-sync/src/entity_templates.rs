//! Build graph records for entity archetypes (e.g. starter corvette).
//! Single source for persistence shape; uses sidereal-game defaults.

use bevy::prelude::Vec3;
use serde::Serialize;
use sidereal_game::{
    AccountId, ControlledEntityGuid, DisplayName, FocusedEntityGuid, MountedOn, OwnerId, PlayerTag,
    SelectedEntityGuid, default_corvette_asset_id, default_corvette_engine,
    default_corvette_flight_computer, default_corvette_flight_tuning, default_corvette_fuel_tank,
    default_corvette_health_pool, default_corvette_mass_kg, default_corvette_max_velocity_mps,
    default_corvette_size,
};
use sidereal_persistence::{GraphComponentRecord, GraphEntityRecord};
use uuid::Uuid;

use crate::format_component_id;

fn component_record<T: Serialize>(
    entity_id: &str,
    component_kind: &str,
    value: &T,
) -> GraphComponentRecord {
    GraphComponentRecord {
        component_id: format_component_id(entity_id, component_kind),
        component_kind: component_kind.to_string(),
        properties: serde_json::to_value(value).unwrap_or(serde_json::json!({})),
    }
}

/// Builds the full set of graph records for a starter corvette (Player + Ship hull + modules).
/// Uses sidereal-game corvette defaults. Persist with `GraphPersistence::persist_graph_records`.
pub fn corvette_starter_graph_records(
    account_id: Uuid,
    player_entity_id: &str,
    position: Vec3,
) -> Vec<GraphEntityRecord> {
    let player_guid = player_entity_id
        .split(':')
        .nth(1)
        .and_then(|raw| Uuid::parse_str(raw).ok())
        .unwrap_or(account_id);
    let ship_entity_id = format!("ship:{player_guid}");
    let fc_entity_id = format!("module:{}", Uuid::new_v4());
    let engine_left_entity_id = format!("module:{}", Uuid::new_v4());
    let engine_right_entity_id = format!("module:{}", Uuid::new_v4());
    let fuel_left_entity_id = format!("module:{}", Uuid::new_v4());
    let fuel_right_entity_id = format!("module:{}", Uuid::new_v4());

    let health_pool = default_corvette_health_pool();
    let flight_computer = default_corvette_flight_computer();
    let flight_tuning = default_corvette_flight_tuning();
    let max_velocity_mps = default_corvette_max_velocity_mps();
    let hull_size = default_corvette_size();
    let hull_mass = default_corvette_mass_kg();
    let owner_id = OwnerId(player_entity_id.to_string());
    let engine = default_corvette_engine();
    let fuel_tank = default_corvette_fuel_tank();

    vec![
        // Player entity
        GraphEntityRecord {
            entity_id: player_entity_id.to_string(),
            labels: vec!["Entity".to_string(), "Player".to_string()],
            properties: serde_json::json!({
                "owner_account_id": account_id.to_string(),
                "player_entity_id": player_entity_id,
                "position_m": [position.x, position.y, position.z],
            }),
            components: vec![
                component_record(
                    player_entity_id,
                    "display_name",
                    &DisplayName("Pilot".to_string()),
                ),
                component_record(player_entity_id, "player_tag", &PlayerTag),
                component_record(
                    player_entity_id,
                    "account_id",
                    &AccountId(account_id.to_string()),
                ),
                component_record(
                    player_entity_id,
                    "controlled_entity_guid",
                    &ControlledEntityGuid(Some(player_guid.to_string())),
                ),
                component_record(
                    player_entity_id,
                    "selected_entity_guid",
                    &SelectedEntityGuid(None),
                ),
                component_record(
                    player_entity_id,
                    "focused_entity_guid",
                    &FocusedEntityGuid(None),
                ),
            ],
        },
        // Ship hull entity
        GraphEntityRecord {
            entity_id: ship_entity_id.clone(),
            labels: vec!["Entity".to_string(), "Ship".to_string()],
            properties: serde_json::json!({
                "owner_account_id": account_id.to_string(),
                "player_entity_id": player_entity_id,
                "name": "Corvette",
                "asset_id": default_corvette_asset_id(),
                "position_m": [position.x, position.y, position.z],
                "velocity_mps": [0.0, 0.0, 0.0],
                "heading_rad": 0.0,
            }),
            components: vec![
                component_record(
                    &ship_entity_id,
                    "display_name",
                    &DisplayName("Corvette".to_string()),
                ),
                component_record(&ship_entity_id, "flight_computer", &flight_computer),
                component_record(&ship_entity_id, "flight_tuning", &flight_tuning),
                component_record(&ship_entity_id, "max_velocity_mps", &max_velocity_mps),
                component_record(&ship_entity_id, "health_pool", &health_pool),
                component_record(&ship_entity_id, "owner_id", &owner_id),
                component_record(
                    &ship_entity_id,
                    "mass_kg",
                    &sidereal_game::MassKg(hull_mass),
                ),
                component_record(&ship_entity_id, "size_m", &hull_size),
            ],
        },
        // Flight computer module
        GraphEntityRecord {
            entity_id: fc_entity_id.clone(),
            labels: vec!["Entity".to_string(), "Module".to_string()],
            properties: serde_json::json!({
                "parent_entity_id": ship_entity_id,
                "hardpoint_id": "computer_core",
            }),
            components: vec![
                component_record(
                    &fc_entity_id,
                    "mounted_on",
                    &MountedOn {
                        parent_entity_id: player_guid,
                        hardpoint_id: "computer_core".to_string(),
                    },
                ),
                component_record(&fc_entity_id, "flight_computer", &flight_computer),
                component_record(&fc_entity_id, "mass_kg", &sidereal_game::MassKg(50.0)),
                component_record(&fc_entity_id, "owner_id", &owner_id),
            ],
        },
        // Left engine module
        GraphEntityRecord {
            entity_id: engine_left_entity_id.clone(),
            labels: vec!["Entity".to_string(), "Module".to_string()],
            properties: serde_json::json!({
                "parent_entity_id": ship_entity_id,
                "hardpoint_id": "engine_left_aft",
            }),
            components: vec![
                component_record(
                    &engine_left_entity_id,
                    "mounted_on",
                    &MountedOn {
                        parent_entity_id: player_guid,
                        hardpoint_id: "engine_left_aft".to_string(),
                    },
                ),
                component_record(&engine_left_entity_id, "engine", &engine),
                component_record(
                    &engine_left_entity_id,
                    "mass_kg",
                    &sidereal_game::MassKg(500.0),
                ),
                component_record(&engine_left_entity_id, "owner_id", &owner_id),
            ],
        },
        // Left fuel tank module
        GraphEntityRecord {
            entity_id: fuel_left_entity_id.clone(),
            labels: vec!["Entity".to_string(), "Module".to_string()],
            properties: serde_json::json!({
                "parent_entity_id": ship_entity_id,
                "hardpoint_id": "fuel_supply",
            }),
            components: vec![
                component_record(&fuel_left_entity_id, "fuel_tank", &fuel_tank),
                component_record(
                    &fuel_left_entity_id,
                    "mass_kg",
                    &sidereal_game::MassKg(1100.0),
                ),
                component_record(&fuel_left_entity_id, "owner_id", &owner_id),
            ],
        },
        // Right engine module
        GraphEntityRecord {
            entity_id: engine_right_entity_id.clone(),
            labels: vec!["Entity".to_string(), "Module".to_string()],
            properties: serde_json::json!({
                "parent_entity_id": ship_entity_id,
                "hardpoint_id": "engine_right_aft",
            }),
            components: vec![
                component_record(
                    &engine_right_entity_id,
                    "mounted_on",
                    &MountedOn {
                        parent_entity_id: player_guid,
                        hardpoint_id: "engine_right_aft".to_string(),
                    },
                ),
                component_record(&engine_right_entity_id, "engine", &engine),
                component_record(
                    &engine_right_entity_id,
                    "mass_kg",
                    &sidereal_game::MassKg(500.0),
                ),
                component_record(&engine_right_entity_id, "owner_id", &owner_id),
            ],
        },
        // Right fuel tank module
        GraphEntityRecord {
            entity_id: fuel_right_entity_id.clone(),
            labels: vec!["Entity".to_string(), "Module".to_string()],
            properties: serde_json::json!({
                "parent_entity_id": ship_entity_id,
                "hardpoint_id": "fuel_supply",
            }),
            components: vec![
                component_record(&fuel_right_entity_id, "fuel_tank", &fuel_tank),
                component_record(
                    &fuel_right_entity_id,
                    "mass_kg",
                    &sidereal_game::MassKg(1100.0),
                ),
                component_record(&fuel_right_entity_id, "owner_id", &owner_id),
            ],
        },
    ]
}
