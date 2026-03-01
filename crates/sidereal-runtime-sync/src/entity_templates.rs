//! Build graph records for new-player starter entities.
//! Single source for persistence shape; uses sidereal-game defaults.

use serde::Serialize;
use sidereal_game::{
    AccountId, ActionQueue, BaseMassKg, CargoMassKg, CharacterMovementController,
    ControlledEntityGuid, DisplayName, EntityLabels, MassDirty, ModuleMassKg, MountedOn, OwnerId,
    ParentGuid, PlayerTag, ShipTag, TotalMassKg, VisualAssetId,
    default_character_movement_action_capabilities,
    default_corvette_asset_id, default_corvette_engine, default_corvette_flight_computer,
    default_corvette_flight_tuning, default_corvette_fuel_tank, default_corvette_hardpoint_specs,
    default_corvette_health_pool, default_corvette_mass_kg, default_corvette_max_velocity_mps,
    default_corvette_size, default_flight_action_capabilities,
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

/// Builds the full set of graph records for a new player: Player entity + starter
/// corvette (ship hull + modules). Uses sidereal-game corvette defaults.
/// Persist with `GraphPersistence::persist_graph_records`.
///
/// Entity IDs are bare UUIDs. Spatial/physics data is stored as component records.
/// Type hierarchy is conveyed through EntityLabels components.
pub fn new_player_starter_graph_records(
    account_id: Uuid,
    player_entity_id: &str,
    email: &str,
    position: bevy::prelude::Vec3,
) -> Vec<GraphEntityRecord> {
    let player_guid =
        crate::parse_guid_from_entity_id(player_entity_id).unwrap_or_else(Uuid::new_v4);
    let player_id = player_guid.to_string();

    let ship_guid = Uuid::new_v4();
    let ship_id = ship_guid.to_string();
    let fc_guid = Uuid::new_v4();
    let fc_id = fc_guid.to_string();
    let engine_left_guid = Uuid::new_v4();
    let engine_left_id = engine_left_guid.to_string();
    let engine_right_guid = Uuid::new_v4();
    let engine_right_id = engine_right_guid.to_string();
    let fuel_left_guid = Uuid::new_v4();
    let fuel_left_id = fuel_left_guid.to_string();
    let fuel_right_guid = Uuid::new_v4();
    let fuel_right_id = fuel_right_guid.to_string();
    let hp_computer_guid = Uuid::new_v4();
    let hp_computer_id = hp_computer_guid.to_string();
    let hp_engine_left_guid = Uuid::new_v4();
    let hp_engine_left_id = hp_engine_left_guid.to_string();
    let hp_engine_right_guid = Uuid::new_v4();
    let hp_engine_right_id = hp_engine_right_guid.to_string();
    let hp_fuel_left_guid = Uuid::new_v4();
    let hp_fuel_left_id = hp_fuel_left_guid.to_string();
    let hp_fuel_right_guid = Uuid::new_v4();
    let hp_fuel_right_id = hp_fuel_right_guid.to_string();

    let health_pool = default_corvette_health_pool();
    let flight_computer = default_corvette_flight_computer();
    let flight_tuning = default_corvette_flight_tuning();
    let max_velocity_mps = default_corvette_max_velocity_mps();
    let hull_size = default_corvette_size();
    let hull_mass = default_corvette_mass_kg();
    let owner_id = OwnerId(player_id.clone());
    let engine = default_corvette_engine();
    let fuel_tank = default_corvette_fuel_tank();

    vec![
        // Player entity
        GraphEntityRecord {
            entity_id: player_id.clone(),
            labels: vec!["Entity".to_string(), "Player".to_string()],
            properties: serde_json::json!({}),
            components: vec![
                component_record(&player_id, "display_name", &DisplayName(email.to_string())),
                component_record(&player_id, "player_tag", &PlayerTag),
                component_record(&player_id, "account_id", &AccountId(account_id.to_string())),
                component_record(
                    &player_id,
                    "controlled_entity_guid",
                    &ControlledEntityGuid(Some(player_id.clone())),
                ),
                component_record(
                    &player_id,
                    "entity_labels",
                    &EntityLabels(vec!["Player".to_string()]),
                ),
                component_record(
                    &player_id,
                    "action_capabilities",
                    &default_character_movement_action_capabilities(),
                ),
                component_record(
                    &player_id,
                    "character_movement_controller",
                    &CharacterMovementController::default(),
                ),
                component_record(&player_id, "action_queue", &ActionQueue::default()),
                GraphComponentRecord {
                    component_id: format_component_id(&player_id, "avian_position"),
                    component_kind: "avian_position".to_string(),
                    properties: serde_json::json!([0.0, 0.0]),
                },
                GraphComponentRecord {
                    component_id: format_component_id(&player_id, "avian_rotation"),
                    component_kind: "avian_rotation".to_string(),
                    properties: serde_json::json!({"cos": 1.0, "sin": 0.0}),
                },
                GraphComponentRecord {
                    component_id: format_component_id(&player_id, "avian_linear_velocity"),
                    component_kind: "avian_linear_velocity".to_string(),
                    properties: serde_json::json!([0.0, 0.0]),
                },
                GraphComponentRecord {
                    component_id: format_component_id(&player_id, "avian_rigid_body"),
                    component_kind: "avian_rigid_body".to_string(),
                    properties: serde_json::json!("Dynamic"),
                },
                GraphComponentRecord {
                    component_id: format_component_id(&player_id, "avian_mass"),
                    component_kind: "avian_mass".to_string(),
                    properties: serde_json::json!(1.0),
                },
                GraphComponentRecord {
                    component_id: format_component_id(&player_id, "avian_angular_inertia"),
                    component_kind: "avian_angular_inertia".to_string(),
                    properties: serde_json::json!(1.0),
                },
                GraphComponentRecord {
                    component_id: format_component_id(&player_id, "avian_linear_damping"),
                    component_kind: "avian_linear_damping".to_string(),
                    properties: serde_json::json!(0.0),
                },
                GraphComponentRecord {
                    component_id: format_component_id(&player_id, "avian_angular_damping"),
                    component_kind: "avian_angular_damping".to_string(),
                    properties: serde_json::json!(0.0),
                },
            ],
        },
        // Ship hull entity
        GraphEntityRecord {
            entity_id: ship_id.clone(),
            labels: vec!["Entity".to_string(), "Ship".to_string()],
            properties: serde_json::json!({}),
            components: {
                let mut comps = vec![
                    component_record(
                        &ship_id,
                        "display_name",
                        &DisplayName("Corvette".to_string()),
                    ),
                    component_record(&ship_id, "ship_tag", &ShipTag),
                    component_record(
                        &ship_id,
                        "entity_labels",
                        &EntityLabels(vec!["Ship".to_string(), "Corvette".to_string()]),
                    ),
                    component_record(&ship_id, "flight_computer", &flight_computer),
                    component_record(&ship_id, "flight_tuning", &flight_tuning),
                    component_record(&ship_id, "max_velocity_mps", &max_velocity_mps),
                    component_record(&ship_id, "health_pool", &health_pool),
                    component_record(&ship_id, "owner_id", &owner_id),
                    component_record(&ship_id, "mass_kg", &sidereal_game::MassKg(hull_mass)),
                    component_record(&ship_id, "size_m", &hull_size),
                    component_record(
                        &ship_id,
                        "action_capabilities",
                        &default_flight_action_capabilities(),
                    ),
                    component_record(&ship_id, "action_queue", &ActionQueue::default()),
                    component_record(
                        &ship_id,
                        "visual_asset_id",
                        &VisualAssetId(default_corvette_asset_id().to_string()),
                    ),
                    component_record(&ship_id, "base_mass_kg", &BaseMassKg(hull_mass)),
                    component_record(&ship_id, "cargo_mass_kg", &CargoMassKg(0.0)),
                    component_record(&ship_id, "module_mass_kg", &ModuleMassKg(0.0)),
                    component_record(&ship_id, "total_mass_kg", &TotalMassKg(hull_mass)),
                    component_record(&ship_id, "mass_dirty", &MassDirty),
                ];
                // Spatial/physics as component records (Avian types).
                // These use the registry `component_kind` keys.
                comps.push(GraphComponentRecord {
                    component_id: format_component_id(&ship_id, "avian_position"),
                    component_kind: "avian_position".to_string(),
                    properties: serde_json::json!([position.x, position.y]),
                });
                comps.push(GraphComponentRecord {
                    component_id: format_component_id(&ship_id, "avian_rotation"),
                    component_kind: "avian_rotation".to_string(),
                    properties: serde_json::json!({"cos": 1.0, "sin": 0.0}),
                });
                comps.push(GraphComponentRecord {
                    component_id: format_component_id(&ship_id, "avian_linear_velocity"),
                    component_kind: "avian_linear_velocity".to_string(),
                    properties: serde_json::json!([0.0, 0.0]),
                });
                comps.push(GraphComponentRecord {
                    component_id: format_component_id(&ship_id, "avian_angular_velocity"),
                    component_kind: "avian_angular_velocity".to_string(),
                    properties: serde_json::json!(0.0),
                });
                comps.push(GraphComponentRecord {
                    component_id: format_component_id(&ship_id, "avian_rigid_body"),
                    component_kind: "avian_rigid_body".to_string(),
                    properties: serde_json::json!("Dynamic"),
                });
                comps.push(GraphComponentRecord {
                    component_id: format_component_id(&ship_id, "avian_mass"),
                    component_kind: "avian_mass".to_string(),
                    properties: serde_json::json!(hull_mass),
                });
                comps.push(GraphComponentRecord {
                    component_id: format_component_id(&ship_id, "avian_angular_inertia"),
                    component_kind: "avian_angular_inertia".to_string(),
                    properties: serde_json::json!(hull_mass * 50.0),
                });
                comps.push(GraphComponentRecord {
                    component_id: format_component_id(&ship_id, "avian_linear_damping"),
                    component_kind: "avian_linear_damping".to_string(),
                    properties: serde_json::json!(0.0),
                });
                comps.push(GraphComponentRecord {
                    component_id: format_component_id(&ship_id, "avian_angular_damping"),
                    component_kind: "avian_angular_damping".to_string(),
                    properties: serde_json::json!(0.0),
                });
                comps
            },
        },
        // Ship hardpoint entities
        GraphEntityRecord {
            entity_id: hp_computer_id.clone(),
            labels: vec!["Entity".to_string(), "Hardpoint".to_string()],
            properties: serde_json::json!({
                "parent_entity_id": ship_id,
            }),
            components: vec![
                component_record(
                    &hp_computer_id,
                    "display_name",
                    &DisplayName("Computer Core Hardpoint".to_string()),
                ),
                component_record(
                    &hp_computer_id,
                    "entity_labels",
                    &EntityLabels(vec!["Hardpoint".to_string()]),
                ),
                component_record(
                    &hp_computer_id,
                    "hardpoint",
                    &sidereal_game::Hardpoint {
                        hardpoint_id: "computer_core".to_string(),
                        offset_m: default_corvette_hardpoint_specs()[0].offset_m,
                    },
                ),
                component_record(&hp_computer_id, "parent_guid", &ParentGuid(ship_guid)),
                component_record(&hp_computer_id, "owner_id", &owner_id),
            ],
        },
        GraphEntityRecord {
            entity_id: hp_engine_left_id.clone(),
            labels: vec!["Entity".to_string(), "Hardpoint".to_string()],
            properties: serde_json::json!({
                "parent_entity_id": ship_id,
            }),
            components: vec![
                component_record(
                    &hp_engine_left_id,
                    "display_name",
                    &DisplayName("Engine Left Aft Hardpoint".to_string()),
                ),
                component_record(
                    &hp_engine_left_id,
                    "entity_labels",
                    &EntityLabels(vec!["Hardpoint".to_string()]),
                ),
                component_record(
                    &hp_engine_left_id,
                    "hardpoint",
                    &sidereal_game::Hardpoint {
                        hardpoint_id: "engine_left_aft".to_string(),
                        offset_m: default_corvette_hardpoint_specs()[1].offset_m,
                    },
                ),
                component_record(&hp_engine_left_id, "parent_guid", &ParentGuid(ship_guid)),
                component_record(&hp_engine_left_id, "owner_id", &owner_id),
            ],
        },
        GraphEntityRecord {
            entity_id: hp_engine_right_id.clone(),
            labels: vec!["Entity".to_string(), "Hardpoint".to_string()],
            properties: serde_json::json!({
                "parent_entity_id": ship_id,
            }),
            components: vec![
                component_record(
                    &hp_engine_right_id,
                    "display_name",
                    &DisplayName("Engine Right Aft Hardpoint".to_string()),
                ),
                component_record(
                    &hp_engine_right_id,
                    "entity_labels",
                    &EntityLabels(vec!["Hardpoint".to_string()]),
                ),
                component_record(
                    &hp_engine_right_id,
                    "hardpoint",
                    &sidereal_game::Hardpoint {
                        hardpoint_id: "engine_right_aft".to_string(),
                        offset_m: default_corvette_hardpoint_specs()[2].offset_m,
                    },
                ),
                component_record(&hp_engine_right_id, "parent_guid", &ParentGuid(ship_guid)),
                component_record(&hp_engine_right_id, "owner_id", &owner_id),
            ],
        },
        GraphEntityRecord {
            entity_id: hp_fuel_left_id.clone(),
            labels: vec!["Entity".to_string(), "Hardpoint".to_string()],
            properties: serde_json::json!({
                "parent_entity_id": ship_id,
            }),
            components: vec![
                component_record(
                    &hp_fuel_left_id,
                    "display_name",
                    &DisplayName("Fuel Tank Left Hardpoint".to_string()),
                ),
                component_record(
                    &hp_fuel_left_id,
                    "entity_labels",
                    &EntityLabels(vec!["Hardpoint".to_string()]),
                ),
                component_record(
                    &hp_fuel_left_id,
                    "hardpoint",
                    &sidereal_game::Hardpoint {
                        hardpoint_id: "fuel_left".to_string(),
                        offset_m: default_corvette_hardpoint_specs()[3].offset_m,
                    },
                ),
                component_record(&hp_fuel_left_id, "parent_guid", &ParentGuid(ship_guid)),
                component_record(&hp_fuel_left_id, "owner_id", &owner_id),
            ],
        },
        GraphEntityRecord {
            entity_id: hp_fuel_right_id.clone(),
            labels: vec!["Entity".to_string(), "Hardpoint".to_string()],
            properties: serde_json::json!({
                "parent_entity_id": ship_id,
            }),
            components: vec![
                component_record(
                    &hp_fuel_right_id,
                    "display_name",
                    &DisplayName("Fuel Tank Right Hardpoint".to_string()),
                ),
                component_record(
                    &hp_fuel_right_id,
                    "entity_labels",
                    &EntityLabels(vec!["Hardpoint".to_string()]),
                ),
                component_record(
                    &hp_fuel_right_id,
                    "hardpoint",
                    &sidereal_game::Hardpoint {
                        hardpoint_id: "fuel_right".to_string(),
                        offset_m: default_corvette_hardpoint_specs()[4].offset_m,
                    },
                ),
                component_record(&hp_fuel_right_id, "parent_guid", &ParentGuid(ship_guid)),
                component_record(&hp_fuel_right_id, "owner_id", &owner_id),
            ],
        },
        // Flight computer module
        GraphEntityRecord {
            entity_id: fc_id.clone(),
            labels: vec!["Entity".to_string(), "Module".to_string()],
            properties: serde_json::json!({
                "parent_entity_id": hp_computer_id,
            }),
            components: vec![
                component_record(
                    &fc_id,
                    "display_name",
                    &DisplayName("Computer Core".to_string()),
                ),
                component_record(
                    &fc_id,
                    "entity_labels",
                    &EntityLabels(vec!["Module".to_string()]),
                ),
                component_record(
                    &fc_id,
                    "mounted_on",
                    &MountedOn {
                        parent_entity_id: ship_guid,
                        hardpoint_id: "computer_core".to_string(),
                    },
                ),
                component_record(&fc_id, "parent_guid", &ParentGuid(hp_computer_guid)),
                component_record(&fc_id, "flight_computer", &flight_computer),
                component_record(&fc_id, "mass_kg", &sidereal_game::MassKg(50.0)),
                component_record(&fc_id, "owner_id", &owner_id),
            ],
        },
        // Left engine module
        GraphEntityRecord {
            entity_id: engine_left_id.clone(),
            labels: vec!["Entity".to_string(), "Module".to_string()],
            properties: serde_json::json!({
                "parent_entity_id": hp_engine_left_id,
            }),
            components: vec![
                component_record(
                    &engine_left_id,
                    "display_name",
                    &DisplayName("Engine Left Aft".to_string()),
                ),
                component_record(
                    &engine_left_id,
                    "entity_labels",
                    &EntityLabels(vec!["Module".to_string(), "Engine".to_string()]),
                ),
                component_record(
                    &engine_left_id,
                    "mounted_on",
                    &MountedOn {
                        parent_entity_id: ship_guid,
                        hardpoint_id: "engine_left_aft".to_string(),
                    },
                ),
                component_record(
                    &engine_left_id,
                    "parent_guid",
                    &ParentGuid(hp_engine_left_guid),
                ),
                component_record(&engine_left_id, "engine", &engine),
                component_record(&engine_left_id, "mass_kg", &sidereal_game::MassKg(500.0)),
                component_record(&engine_left_id, "owner_id", &owner_id),
            ],
        },
        // Left fuel tank module
        GraphEntityRecord {
            entity_id: fuel_left_id.clone(),
            labels: vec!["Entity".to_string(), "Module".to_string()],
            properties: serde_json::json!({
                "parent_entity_id": hp_fuel_left_id,
            }),
            components: vec![
                component_record(
                    &fuel_left_id,
                    "display_name",
                    &DisplayName("Fuel Tank Left".to_string()),
                ),
                component_record(
                    &fuel_left_id,
                    "entity_labels",
                    &EntityLabels(vec!["Module".to_string(), "FuelTank".to_string()]),
                ),
                component_record(
                    &fuel_left_id,
                    "mounted_on",
                    &MountedOn {
                        parent_entity_id: ship_guid,
                        hardpoint_id: "fuel_left".to_string(),
                    },
                ),
                component_record(&fuel_left_id, "parent_guid", &ParentGuid(hp_fuel_left_guid)),
                component_record(&fuel_left_id, "fuel_tank", &fuel_tank),
                component_record(&fuel_left_id, "mass_kg", &sidereal_game::MassKg(1100.0)),
                component_record(&fuel_left_id, "owner_id", &owner_id),
            ],
        },
        // Right engine module
        GraphEntityRecord {
            entity_id: engine_right_id.clone(),
            labels: vec!["Entity".to_string(), "Module".to_string()],
            properties: serde_json::json!({
                "parent_entity_id": hp_engine_right_id,
            }),
            components: vec![
                component_record(
                    &engine_right_id,
                    "display_name",
                    &DisplayName("Engine Right Aft".to_string()),
                ),
                component_record(
                    &engine_right_id,
                    "entity_labels",
                    &EntityLabels(vec!["Module".to_string(), "Engine".to_string()]),
                ),
                component_record(
                    &engine_right_id,
                    "mounted_on",
                    &MountedOn {
                        parent_entity_id: ship_guid,
                        hardpoint_id: "engine_right_aft".to_string(),
                    },
                ),
                component_record(
                    &engine_right_id,
                    "parent_guid",
                    &ParentGuid(hp_engine_right_guid),
                ),
                component_record(&engine_right_id, "engine", &engine),
                component_record(&engine_right_id, "mass_kg", &sidereal_game::MassKg(500.0)),
                component_record(&engine_right_id, "owner_id", &owner_id),
            ],
        },
        // Right fuel tank module
        GraphEntityRecord {
            entity_id: fuel_right_id.clone(),
            labels: vec!["Entity".to_string(), "Module".to_string()],
            properties: serde_json::json!({
                "parent_entity_id": hp_fuel_right_id,
            }),
            components: vec![
                component_record(
                    &fuel_right_id,
                    "display_name",
                    &DisplayName("Fuel Tank Right".to_string()),
                ),
                component_record(
                    &fuel_right_id,
                    "entity_labels",
                    &EntityLabels(vec!["Module".to_string(), "FuelTank".to_string()]),
                ),
                component_record(
                    &fuel_right_id,
                    "mounted_on",
                    &MountedOn {
                        parent_entity_id: ship_guid,
                        hardpoint_id: "fuel_right".to_string(),
                    },
                ),
                component_record(
                    &fuel_right_id,
                    "parent_guid",
                    &ParentGuid(hp_fuel_right_guid),
                ),
                component_record(&fuel_right_id, "fuel_tank", &fuel_tank),
                component_record(&fuel_right_id, "mass_kg", &sidereal_game::MassKg(1100.0)),
                component_record(&fuel_right_id, "owner_id", &owner_id),
            ],
        },
    ]
}
