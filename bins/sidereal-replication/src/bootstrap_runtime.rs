use bevy::log::info;
use bevy::prelude::{Commands, Resource};
use serde::Serialize;
use sidereal_game::{
    CorvetteSpawnConfig, DisplayName, Engine, FuelTank, MassKg, MountedOn, OwnerId,
    default_corvette_asset_id, default_corvette_flight_computer, default_corvette_flight_tuning,
    default_corvette_health_pool, default_corvette_mass_kg, default_corvette_max_velocity_mps,
    default_corvette_size, default_starfield_shader_asset_id,
};
use sidereal_persistence::GraphPersistence;
use sidereal_persistence::{GraphComponentRecord, GraphEntityRecord};
use sidereal_replication::bootstrap::{BootstrapProcessor, PostgresBootstrapStore};
use sidereal_runtime_sync::format_component_id;
use std::net::UdpSocket;
use std::sync::{Mutex, mpsc};
use std::thread;

/// Channel for bootstrap thread to request ship spawning in the Bevy world.
#[derive(Resource)]
pub struct BootstrapShipReceiver(pub Mutex<mpsc::Receiver<BootstrapShipCommand>>);

#[derive(Debug, Clone)]
pub struct BootstrapShipCommand {
    pub account_id: uuid::Uuid,
    pub player_entity_id: String,
    pub ship_entity_id: String,
}

pub fn start_replication_control_listener(mut commands: Commands<'_, '_>) {
    let bind_addr = std::env::var("REPLICATION_CONTROL_UDP_BIND")
        .unwrap_or_else(|_| "127.0.0.1:9004".to_string());
    let database_url = std::env::var("REPLICATION_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://sidereal:sidereal@127.0.0.1:5432/sidereal".to_string());

    let socket = match UdpSocket::bind(&bind_addr) {
        Ok(socket) => socket,
        Err(err) => {
            eprintln!("failed to bind replication control UDP listener on {bind_addr}: {err}");
            return;
        }
    };
    let store = match PostgresBootstrapStore::connect(&database_url) {
        Ok(store) => store,
        Err(err) => {
            eprintln!("failed to connect replication bootstrap store: {err}");
            return;
        }
    };
    let mut processor = match BootstrapProcessor::new(store) {
        Ok(processor) => processor,
        Err(err) => {
            eprintln!("failed to initialize replication bootstrap processor: {err}");
            return;
        }
    };

    let (tx, rx) = mpsc::channel::<BootstrapShipCommand>();
    commands.insert_resource(BootstrapShipReceiver(Mutex::new(rx)));

    info!("replication control UDP listening on {}", bind_addr);
    thread::spawn(move || {
        let db_url = database_url;
        loop {
            let mut buf = vec![0_u8; 8192];
            let (size, from) = match socket.recv_from(&mut buf) {
                Ok(v) => v,
                Err(err) => {
                    eprintln!("replication control recv error: {err}");
                    continue;
                }
            };
            let payload = &buf[..size];
            match processor.handle_payload(payload) {
                Ok(result) => {
                    println!(
                        "replication bootstrap processed from {from}: account_id={}, player_entity_id={}, applied={}",
                        result.account_id, result.player_entity_id, result.applied
                    );
                    if result.applied {
                        if let Err(err) = bootstrap_starter_ship(
                            &db_url,
                            result.account_id,
                            &result.player_entity_id,
                        ) {
                            eprintln!(
                                "replication bootstrap world-init failed for account {}: {err}",
                                result.account_id
                            );
                        } else {
                            let ship_entity_id = format!("ship:{}", result.account_id);
                            let _ = tx.send(BootstrapShipCommand {
                                account_id: result.account_id,
                                player_entity_id: result.player_entity_id,
                                ship_entity_id,
                            });
                        }
                    }
                }
                Err(err) => {
                    eprintln!("replication control message rejected from {from}: {err}");
                }
            }
        }
    });
}

pub fn drain_bootstrap_ship_commands(
    receiver: &BootstrapShipReceiver,
) -> Vec<BootstrapShipCommand> {
    let Ok(rx) = receiver.0.lock() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    while let Ok(cmd) = rx.try_recv() {
        out.push(cmd);
    }
    out
}

fn bootstrap_starter_ship(
    database_url: &str,
    account_id: uuid::Uuid,
    player_entity_id: &str,
) -> sidereal_persistence::Result<()> {
    let mut persistence = GraphPersistence::connect(database_url)?;
    persistence.ensure_schema()?;

    let ship_entity_id = format!("ship:{account_id}");
    let fc_entity_id = format!("module:{}", uuid::Uuid::new_v4());
    let engine_left_entity_id = format!("module:{}", uuid::Uuid::new_v4());
    let engine_right_entity_id = format!("module:{}", uuid::Uuid::new_v4());
    let fuel_left_entity_id = format!("module:{}", uuid::Uuid::new_v4());
    let fuel_right_entity_id = format!("module:{}", uuid::Uuid::new_v4());

    let spawn_pos = starter_spawn_position(account_id);
    let health_pool = default_corvette_health_pool();
    let flight_computer = default_corvette_flight_computer();
    let flight_tuning = default_corvette_flight_tuning();
    let max_velocity_mps = default_corvette_max_velocity_mps();
    let hull_size = default_corvette_size();
    let hull_mass = default_corvette_mass_kg();
    let owner_id = OwnerId(player_entity_id.to_string());

    let engine = Engine {
        thrust: 1_200_000.0,
        reverse_thrust: 600_000.0,
        torque_thrust: 3_000_000.0,
        burn_rate_kg_s: 0.8,
    };
    let fuel_tank = FuelTank { fuel_kg: 1000.0 };

    let starter_world = vec![
        // Player entity
        GraphEntityRecord {
            entity_id: player_entity_id.to_string(),
            labels: vec!["Entity".to_string(), "Player".to_string()],
            properties: serde_json::json!({
                "owner_account_id": account_id.to_string(),
                "player_entity_id": player_entity_id,
            }),
            components: vec![component_record(
                player_entity_id,
                "display_name",
                &DisplayName("Pilot".to_string()),
            )],
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
                "starfield_shader_asset_id": default_starfield_shader_asset_id(),
                "position_m": [spawn_pos.x, spawn_pos.y, 0.0],
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
                component_record(&ship_entity_id, "mass_kg", &MassKg(hull_mass)),
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
                        parent_entity_id: account_id,
                        hardpoint_id: "computer_core".to_string(),
                    },
                ),
                component_record(&fc_entity_id, "flight_computer", &flight_computer),
                component_record(&fc_entity_id, "mass_kg", &MassKg(50.0)),
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
                        parent_entity_id: account_id,
                        hardpoint_id: "engine_left_aft".to_string(),
                    },
                ),
                component_record(&engine_left_entity_id, "engine", &engine),
                component_record(&engine_left_entity_id, "mass_kg", &MassKg(500.0)),
                component_record(&engine_left_entity_id, "owner_id", &owner_id),
            ],
        },
        // Left fuel tank module (mounted on left engine)
        GraphEntityRecord {
            entity_id: fuel_left_entity_id.clone(),
            labels: vec!["Entity".to_string(), "Module".to_string()],
            properties: serde_json::json!({
                "parent_entity_id": ship_entity_id,
                "hardpoint_id": "fuel_supply",
            }),
            components: vec![
                component_record(&fuel_left_entity_id, "fuel_tank", &fuel_tank),
                component_record(&fuel_left_entity_id, "mass_kg", &MassKg(1100.0)),
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
                        parent_entity_id: account_id,
                        hardpoint_id: "engine_right_aft".to_string(),
                    },
                ),
                component_record(&engine_right_entity_id, "engine", &engine),
                component_record(&engine_right_entity_id, "mass_kg", &MassKg(500.0)),
                component_record(&engine_right_entity_id, "owner_id", &owner_id),
            ],
        },
        // Right fuel tank module (mounted on right engine)
        GraphEntityRecord {
            entity_id: fuel_right_entity_id.clone(),
            labels: vec!["Entity".to_string(), "Module".to_string()],
            properties: serde_json::json!({
                "parent_entity_id": ship_entity_id,
                "hardpoint_id": "fuel_supply",
            }),
            components: vec![
                component_record(&fuel_right_entity_id, "fuel_tank", &fuel_tank),
                component_record(&fuel_right_entity_id, "mass_kg", &MassKg(1100.0)),
                component_record(&fuel_right_entity_id, "owner_id", &owner_id),
            ],
        },
    ];
    persistence.persist_graph_records(&starter_world, 0)?;
    Ok(())
}

pub fn starter_spawn_position(account_id: uuid::Uuid) -> bevy::prelude::Vec3 {
    CorvetteSpawnConfig {
        owner_account_id: account_id,
        player_entity_id: format!("player:{account_id}"),
        spawn_position: None,
        spawn_velocity: bevy::prelude::Vec3::ZERO,
        shard_id: 0,
        display_name: None,
    }
    .get_spawn_position()
}

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
