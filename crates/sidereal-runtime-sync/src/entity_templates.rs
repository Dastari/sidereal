//! Build graph records for new-player starter entities.
//! Single source for persistence shape; uses sidereal-game defaults.

use serde::Serialize;
use sidereal_game::{
    AccountId, ActionQueue, AfterburnerState, AmmoCount, BallisticWeapon, BaseMassKg, CargoMassKg,
    CharacterMovementController, CollisionProfile, ControlledEntityGuid, CorvetteModuleKind,
    DisplayName, EntityLabels, MassDirty, ModuleMassKg, MountedOn, OwnerId, ParentGuid, PlayerTag,
    ShipTag, ThrusterPlumeShaderSettings, TotalMassKg, VisualAssetId, WeaponTag,
    default_character_movement_action_capabilities, default_corvette_afterburner_capability,
    default_corvette_asset_id, default_corvette_collision_aabb, default_corvette_collision_outline,
    default_corvette_engine, default_corvette_flight_computer, default_corvette_flight_tuning,
    default_corvette_fuel_tank, default_corvette_hardpoint_specs, default_corvette_health_pool,
    default_corvette_mass_kg, default_corvette_max_velocity_mps, default_corvette_module_specs,
    default_corvette_size, default_flight_action_capabilities,
};
use sidereal_persistence::{GraphComponentRecord, GraphEntityRecord};
use std::collections::HashMap;
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
    let owner_id = OwnerId(player_id.clone());

    let health_pool = default_corvette_health_pool();
    let flight_computer = default_corvette_flight_computer();
    let flight_tuning = default_corvette_flight_tuning();
    let max_velocity_mps = default_corvette_max_velocity_mps();
    let hull_size = default_corvette_size();
    let hull_mass = default_corvette_mass_kg();
    let engine = default_corvette_engine();
    let afterburner_capability = default_corvette_afterburner_capability();
    let plume_settings = ThrusterPlumeShaderSettings::default();
    let fuel_tank = default_corvette_fuel_tank();
    let hardpoint_specs = default_corvette_hardpoint_specs();
    let module_specs = default_corvette_module_specs();

    let mut hardpoint_ids = HashMap::<&'static str, (Uuid, String)>::new();
    for spec in hardpoint_specs {
        let guid = Uuid::new_v4();
        hardpoint_ids.insert(spec.hardpoint_id, (guid, guid.to_string()));
    }

    let mut module_ids = HashMap::<&'static str, (Uuid, String)>::new();
    for spec in module_specs {
        let guid = Uuid::new_v4();
        module_ids.insert(spec.module_id, (guid, guid.to_string()));
    }

    let module_labels = |kind: CorvetteModuleKind| match kind {
        CorvetteModuleKind::FlightComputer => vec!["Module".to_string()],
        CorvetteModuleKind::Engine => vec!["Module".to_string(), "Engine".to_string()],
        CorvetteModuleKind::FuelTank => vec!["Module".to_string(), "FuelTank".to_string()],
        CorvetteModuleKind::BallisticGatling => {
            vec![
                "Module".to_string(),
                "Weapon".to_string(),
                "BallisticWeapon".to_string(),
            ]
        }
    };

    let mut records = vec![
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
                    component_record(&ship_id, "afterburner_state", &AfterburnerState::default()),
                    component_record(&ship_id, "flight_tuning", &flight_tuning),
                    component_record(&ship_id, "max_velocity_mps", &max_velocity_mps),
                    component_record(&ship_id, "health_pool", &health_pool),
                    component_record(&ship_id, "owner_id", &owner_id),
                    component_record(&ship_id, "mass_kg", &sidereal_game::MassKg(hull_mass)),
                    component_record(&ship_id, "size_m", &hull_size),
                    component_record(
                        &ship_id,
                        "collision_profile",
                        &CollisionProfile::solid_aabb(),
                    ),
                    component_record(
                        &ship_id,
                        "collision_outline_m",
                        &default_corvette_collision_outline(),
                    ),
                    component_record(
                        &ship_id,
                        "collision_aabb_m",
                        &default_corvette_collision_aabb(),
                    ),
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
    ];

    for spec in hardpoint_specs {
        let (_, hp_id) = hardpoint_ids
            .get(spec.hardpoint_id)
            .expect("missing hardpoint id");
        records.push(GraphEntityRecord {
            entity_id: hp_id.clone(),
            labels: vec!["Entity".to_string(), "Hardpoint".to_string()],
            properties: serde_json::json!({
                "parent_entity_id": ship_id,
            }),
            components: vec![
                component_record(
                    hp_id,
                    "display_name",
                    &DisplayName(spec.display_name.to_string()),
                ),
                component_record(
                    hp_id,
                    "entity_labels",
                    &EntityLabels(vec!["Hardpoint".to_string()]),
                ),
                component_record(
                    hp_id,
                    "hardpoint",
                    &sidereal_game::Hardpoint {
                        hardpoint_id: spec.hardpoint_id.to_string(),
                        offset_m: spec.offset_m,
                        local_rotation: spec.local_rotation,
                    },
                ),
                component_record(hp_id, "parent_guid", &ParentGuid(ship_guid)),
                component_record(hp_id, "owner_id", &owner_id),
            ],
        });
    }

    for spec in module_specs {
        let (_, module_id) = module_ids.get(spec.module_id).expect("missing module id");
        let (hardpoint_guid, hardpoint_id) = hardpoint_ids
            .get(spec.hardpoint_id)
            .expect("missing module hardpoint id");

        let mut components = vec![
            component_record(
                module_id,
                "display_name",
                &DisplayName(spec.display_name.to_string()),
            ),
            component_record(
                module_id,
                "entity_labels",
                &EntityLabels(module_labels(spec.kind)),
            ),
            component_record(
                module_id,
                "mounted_on",
                &MountedOn {
                    parent_entity_id: ship_guid,
                    hardpoint_id: spec.hardpoint_id.to_string(),
                },
            ),
            component_record(module_id, "parent_guid", &ParentGuid(*hardpoint_guid)),
            component_record(module_id, "mass_kg", &sidereal_game::MassKg(spec.mass_kg)),
            component_record(module_id, "owner_id", &owner_id),
        ];
        match spec.kind {
            CorvetteModuleKind::FlightComputer => {
                components.push(component_record(
                    module_id,
                    "flight_computer",
                    &flight_computer,
                ));
            }
            CorvetteModuleKind::Engine => {
                components.push(component_record(module_id, "engine", &engine));
                components.push(component_record(
                    module_id,
                    "afterburner_capability",
                    &afterburner_capability,
                ));
                components.push(component_record(
                    module_id,
                    "thruster_plume_shader_settings",
                    &plume_settings,
                ));
            }
            CorvetteModuleKind::FuelTank => {
                components.push(component_record(module_id, "fuel_tank", &fuel_tank));
            }
            CorvetteModuleKind::BallisticGatling => {
                components.push(component_record(module_id, "weapon_tag", &WeaponTag));
                components.push(component_record(
                    module_id,
                    "ballistic_weapon",
                    &BallisticWeapon::corvette_ballistic_gatling(),
                ));
                components.push(component_record(
                    module_id,
                    "ammo_count",
                    &AmmoCount::new(500, 500),
                ));
            }
        }

        records.push(GraphEntityRecord {
            entity_id: module_id.clone(),
            labels: vec!["Entity".to_string(), "Module".to_string()],
            properties: serde_json::json!({
                "parent_entity_id": hardpoint_id,
            }),
            components,
        });
    }

    records
}
