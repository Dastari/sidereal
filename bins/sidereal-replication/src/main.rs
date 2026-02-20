mod visibility;

use avian3d::prelude::*;
use bevy::asset::{AssetApp, AssetPlugin};
use bevy::ecs::reflect::{AppTypeRegistry, ReflectCommandExt};
use bevy::log::LogPlugin;
use bevy::prelude::*;
use bevy::reflect::serde::{TypedReflectDeserializer, TypedReflectSerializer};
use bevy::scene::ScenePlugin;
use bevy_remote::RemotePlugin;
use bevy_remote::http::RemoteHttpPlugin;
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode};
use lightyear::prelude::client::Connected;
use lightyear::prelude::server::ServerPlugins;
use lightyear::prelude::server::{ClientOf, RawServer, Start};
use lightyear::prelude::server::{ServerUdpIo, Stopped};
use lightyear::prelude::{
    ChannelRegistry, LocalAddr, MessageReceiver, RemoteId, Server, ServerMultiMessageSender,
    Transport,
};
use serde::de::DeserializeSeed;
use sidereal_core::remote_inspect::RemoteInspectConfig;
use sidereal_game::{
    ActionCapabilities, ActionQueue, BaseMassKg, CargoMassKg, Engine, EntityAction, EntityGuid,
    FlightComputer, FuelTank, GeneratedComponentRegistry, Hardpoint, HealthPool, Inventory,
    MassDirty, MassKg, ModuleMassKg, MountedOn, OwnerId, PositionM, ScannerComponent,
    ScannerRangeBuff, ScannerRangeM, SiderealGamePlugin, TotalMassKg, VelocityMps,
};
use sidereal_net::{
    ClientAuthMessage, ClientInputMessage, ControlChannel, InputChannel, ReplicationStateMessage,
    StateChannel, WorldComponentDelta, WorldDeltaEntity, WorldStateDelta,
    register_lightyear_protocol,
};
use sidereal_persistence::{
    GraphComponentRecord, GraphPersistence, decode_reflect_component, encode_reflect_component,
};
use sidereal_replication::bootstrap::{BootstrapProcessor, PostgresBootstrapStore};
use sidereal_replication::state::{
    flush_pending_updates, hydrate_known_entity_ids, ingest_world_delta,
};
use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::net::UdpSocket;
use std::sync::{Mutex, mpsc};

use std::thread;
use std::time::{Duration, Instant};
use visibility::{
    ClientControlledEntityPositionMap, ClientVisibilityHistory, ClientVisibilityRegistry,
    apply_visibility_filter, delivery_target_for_session, visibility_context_for_client,
};

#[derive(Debug, Resource, Clone)]
#[allow(dead_code)]
struct BrpAuthToken(String);

#[derive(Debug, Resource, Clone, Copy)]
#[allow(dead_code)]
struct HydratedEntityCount(usize);

#[derive(Debug, Component)]
#[allow(dead_code)]
struct HydratedGraphEntity {
    entity_id: String,
    labels: Vec<String>,
    component_count: usize,
}

#[derive(Resource, Default)]
struct ReplicationOutboundQueue {
    messages: Vec<QueuedReplicationDelta>,
}

#[derive(Debug, Clone)]
struct QueuedReplicationDelta {
    tick: u64,
    world: WorldStateDelta,
}

struct ReplicationRuntime {
    persistence: sidereal_persistence::GraphPersistence,
    known_entities: HashSet<String>,
    pending_updates: HashMap<String, WorldDeltaEntity>,
    last_tick: u64,
    persist_interval: Duration,
    snapshot_interval: Duration,
    last_persist_at: Instant,
    last_snapshot_at: Instant,
    last_persisted_state: HashMap<String, PersistedEntitySnapshot>,
}

#[derive(Debug, Clone)]
struct PersistedEntitySnapshot {
    position: Vec3,
    velocity: Vec3,
    health: f32,
}

const PERSISTENCE_POSITION_THRESHOLD: f32 = 0.05;
const PERSISTENCE_VELOCITY_THRESHOLD: f32 = 0.01;
const PERSISTENCE_HEALTH_THRESHOLD: f32 = 0.1;

#[derive(Resource, Default)]
struct PlayerControlledEntityMap {
    by_player_entity_id: HashMap<String, Entity>,
}

#[derive(Debug, Component)]
struct SimulatedControlledEntity {
    entity_id: String,
    player_entity_id: String,
}

#[derive(Resource, Default)]
struct AuthenticatedClientBindings {
    by_client_entity: HashMap<Entity, String>,
    by_remote_id: HashMap<lightyear::prelude::PeerId, String>,
}

#[derive(Debug, serde::Deserialize)]
struct AccessTokenClaims {
    player_entity_id: String,
}

/// Channel for bootstrap thread to request ship spawning in the Bevy world
#[derive(Resource)]
struct BootstrapShipReceiver(Mutex<mpsc::Receiver<BootstrapShipCommand>>);

#[derive(Debug, Clone)]
struct BootstrapShipCommand {
    #[allow(dead_code)]
    account_id: uuid::Uuid,
    player_entity_id: String,
    ship_entity_id: String,
}

type ConnectedClientFilter = (With<ClientOf>, With<Connected>);

fn main() {
    let remote_cfg = match RemoteInspectConfig::from_env("REPLICATION", 15713) {
        Ok(cfg) => cfg,
        Err(err) => {
            eprintln!("invalid REPLICATION BRP config: {err}");
            std::process::exit(2);
        }
    };

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(AssetPlugin::default());
    app.add_plugins(ScenePlugin);
    app.add_plugins(LogPlugin::default());
    app.add_plugins(SiderealGamePlugin);
    app.add_plugins(PhysicsPlugins::default().with_length_unit(1.0));
    app.add_message::<bevy::asset::AssetEvent<Mesh>>();
    app.init_asset::<Mesh>();
    app.insert_resource(Gravity(Vec3::ZERO));
    app.insert_resource(Time::<Fixed>::from_hz(30.0));
    app.add_plugins(ServerPlugins::default());
    register_lightyear_protocol(&mut app);
    configure_remote(&mut app, &remote_cfg);
    app.add_systems(
        Startup,
        (
            init_replication_runtime,
            hydrate_replication_world,
            hydrate_simulation_entities,
            start_lightyear_server,
        )
            .chain(),
    );
    app.add_systems(Startup, start_replication_control_listener);
    app.add_observer(log_replication_client_connected);
    app.insert_resource(ReplicationOutboundQueue::default());
    app.insert_resource(ClientVisibilityRegistry::default());
    app.insert_resource(ClientControlledEntityPositionMap::default());
    app.insert_resource(ClientVisibilityHistory::default());
    app.insert_resource(PlayerControlledEntityMap::default());
    app.insert_resource(AuthenticatedClientBindings::default());
    app.add_systems(
        Update,
        (
            ensure_server_transport_channels,
            cleanup_client_auth_bindings,
            receive_client_auth_messages,
            receive_client_inputs,
            process_bootstrap_ship_commands,
            sync_simulated_ship_components,
            update_client_controlled_entity_positions,
            compute_controlled_entity_scanner_ranges,
            collect_local_simulation_state,
            refresh_component_payloads_from_reflection,
            broadcast_replication_state,
            flush_replication_persistence,
        )
            .chain(),
    );
    app.add_systems(Startup, || {
        println!("sidereal-replication scaffold");
    });
    app.run();
}

fn configure_remote(app: &mut App, cfg: &RemoteInspectConfig) {
    if !cfg.enabled {
        return;
    }

    app.add_plugins(RemotePlugin::default());
    app.add_plugins(
        RemoteHttpPlugin::default()
            .with_address(cfg.bind_addr)
            .with_port(cfg.port),
    );
    app.insert_resource(BrpAuthToken(
        cfg.auth_token.clone().expect("validated token"),
    ));
}

fn hydrate_replication_world(mut commands: Commands<'_, '_>) {
    let database_url = std::env::var("REPLICATION_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://sidereal:sidereal@127.0.0.1:5432/sidereal".to_string());

    let mut persistence = match GraphPersistence::connect(&database_url) {
        Ok(v) => v,
        Err(err) => {
            eprintln!("replication hydration skipped; connect failed: {err}");
            return;
        }
    };
    if let Err(err) = persistence.ensure_schema() {
        eprintln!("replication hydration skipped; schema ensure failed: {err}");
        return;
    }

    let records = match persistence.load_graph_records() {
        Ok(v) => v,
        Err(err) => {
            eprintln!("replication hydration skipped; graph load failed: {err}");
            return;
        }
    };

    for record in &records {
        commands.spawn(HydratedGraphEntity {
            entity_id: record.entity_id.clone(),
            labels: record.labels.clone(),
            component_count: record.components.len(),
        });
    }
    commands.insert_resource(HydratedEntityCount(records.len()));
    println!(
        "replication hydrated {} graph entities into Bevy world",
        records.len()
    );
}

#[allow(clippy::too_many_arguments)]
fn spawn_simulation_entity(
    commands: &mut Commands<'_, '_>,
    controlled_entity_map: &mut PlayerControlledEntityMap,
    entity_id: &str,
    player_entity_id: &str,
    pos: Vec3,
    vel: Vec3,
    health: f32,
    max_health: f32,
) {
    let ship_guid = parse_guid_from_entity_id(entity_id).unwrap_or_else(uuid::Uuid::new_v4);

    let entity = commands
        .spawn((
            Name::new(entity_id.to_string()),
            SimulatedControlledEntity {
                entity_id: entity_id.to_string(),
                player_entity_id: player_entity_id.to_string(),
            },
            EntityGuid(ship_guid),
            OwnerId(player_entity_id.to_string()),
            ActionQueue::default(),
            ActionCapabilities {
                supported: vec![
                    EntityAction::ThrustForward,
                    EntityAction::ThrustReverse,
                    EntityAction::ThrustNeutral,
                    EntityAction::Brake,
                    EntityAction::YawLeft,
                    EntityAction::YawRight,
                    EntityAction::YawNeutral,
                ],
            },
            FlightComputer {
                profile: "basic_fly_by_wire".to_string(),
                throttle: 0.0,
                yaw_input: 0.0,
                turn_rate_deg_s: 45.0,
            },
            HealthPool {
                current: health,
                maximum: max_health,
            },
            PositionM(pos),
            VelocityMps(vel),
            Transform::from_translation(pos),
        ))
        .insert((
            MassKg(15_000.0),
            BaseMassKg(15_000.0),
            CargoMassKg(0.0),
            ModuleMassKg(0.0),
            TotalMassKg(15_000.0),
            MassDirty,
            Inventory::default(),
        ))
        .insert((
            RigidBody::Dynamic,
            Collider::cuboid(6.0, 3.0, 2.0),
            Position(pos),
            Rotation::default(),
            LinearVelocity(vel),
            AngularVelocity::default(),
            LinearDamping(0.12),
            AngularDamping(0.35),
        ))
        .id();
    controlled_entity_map
        .by_player_entity_id
        .insert(player_entity_id.to_string(), entity);

    let engine_guid = uuid::Uuid::new_v4();
    commands.spawn((
        Name::new(format!("{}:engine", entity_id)),
        EntityGuid(engine_guid),
        MountedOn {
            parent_entity_id: ship_guid,
            hardpoint_id: "engine_main".to_string(),
        },
        Engine {
            thrust_n: 140_000.0,
            burn_rate_kg_s: 0.4,
            thrust_dir: Vec3::Y,
        },
        FuelTank { fuel_kg: 1000.0 },
        OwnerId(player_entity_id.to_string()),
    ));
}

fn hydrate_simulation_entities(
    mut commands: Commands<'_, '_>,
    mut controlled_entity_map: ResMut<'_, PlayerControlledEntityMap>,
    component_registry: Res<'_, GeneratedComponentRegistry>,
    app_type_registry: Res<'_, AppTypeRegistry>,
) {
    let database_url = std::env::var("REPLICATION_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://sidereal:sidereal@127.0.0.1:5432/sidereal".to_string());

    let mut persistence = match GraphPersistence::connect(&database_url) {
        Ok(v) => v,
        Err(err) => {
            eprintln!("replication simulation hydration skipped; connect failed: {err}");
            return;
        }
    };
    if let Err(err) = persistence.ensure_schema() {
        eprintln!("replication simulation hydration skipped; schema ensure failed: {err}");
        return;
    }
    let records = match persistence.load_graph_records() {
        Ok(v) => v,
        Err(err) => {
            eprintln!("replication simulation hydration skipped; graph load failed: {err}");
            return;
        }
    };

    let type_paths = component_type_path_map(&component_registry);
    let mut ship_guid_by_entity_id = HashMap::<String, uuid::Uuid>::new();
    let mut spawned_entity_by_entity_id = HashMap::<String, Entity>::new();
    let mut pending_parent_links = Vec::<(Entity, String)>::new();
    let mut ship_records = Vec::new();
    let mut hardpoint_records = Vec::new();
    let mut module_records = Vec::new();

    for record in records {
        if record.labels.iter().any(|label| label == "Ship") {
            ship_records.push(record);
        } else if record.labels.iter().any(|label| label == "Hardpoint")
            || component_record(&record.components, "hardpoint").is_some()
        {
            hardpoint_records.push(record);
        } else if component_record(&record.components, "mounted_on").is_some() {
            module_records.push(record);
        }
    }

    let mut hydrated_ships = 0usize;
    let mut hydrated_hardpoints = 0usize;
    let mut hydrated_modules = 0usize;

    // Pass 1: hull entities first so module relationships can resolve parent GUIDs.
    for record in &ship_records {
        let player_entity_id = record
            .properties
            .get("player_entity_id")
            .and_then(|v| v.as_str())
            .map(ToString::to_string)
            .or_else(|| {
                owner_id_from_record(record, &type_paths)
                    .map(|owner| owner.0)
                    .filter(|owner| owner.starts_with("player:"))
            });
        let Some(player_entity_id) = player_entity_id else {
            continue;
        };

        let ship_guid =
            parse_guid_from_entity_id(&record.entity_id).unwrap_or_else(uuid::Uuid::new_v4);
        ship_guid_by_entity_id.insert(record.entity_id.clone(), ship_guid);

        let pos = record
            .properties
            .get("position_m")
            .and_then(parse_vec3_value)
            .unwrap_or(Vec3::ZERO);
        let vel = record
            .properties
            .get("velocity_mps")
            .and_then(parse_vec3_value)
            .unwrap_or(Vec3::ZERO);
        let heading_rad = record
            .properties
            .get("heading_rad")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0) as f32;
        let health_pool = health_pool_from_record(record, &type_paths).unwrap_or(HealthPool {
            current: 100.0,
            maximum: 100.0,
        });
        let flight_computer =
            flight_computer_from_record(record, &type_paths).unwrap_or(FlightComputer {
                profile: "basic_fly_by_wire".to_string(),
                throttle: 0.0,
                yaw_input: 0.0,
                turn_rate_deg_s: 45.0,
            });
        let scanner_range =
            scanner_range_from_record(record, &type_paths).unwrap_or(ScannerRangeM(0.0));
        let scanner_component = scanner_component_from_record(record, &type_paths);
        let scanner_buff = scanner_range_buff_from_record(record, &type_paths);
        let mass_kg = mass_kg_from_record(record, &type_paths).unwrap_or(MassKg(15_000.0));
        let base_mass = base_mass_from_record(record, &type_paths).unwrap_or(BaseMassKg(mass_kg.0));
        let cargo_mass = cargo_mass_from_record(record, &type_paths).unwrap_or(CargoMassKg(0.0));
        let module_mass = module_mass_from_record(record, &type_paths).unwrap_or(ModuleMassKg(0.0));
        let total_mass =
            total_mass_from_record(record, &type_paths).unwrap_or(TotalMassKg(base_mass.0));
        let inventory = inventory_from_record(record, &type_paths).unwrap_or_default();

        let mut entity_commands = commands.spawn((
            Name::new(record.entity_id.clone()),
            SimulatedControlledEntity {
                entity_id: record.entity_id.clone(),
                player_entity_id: player_entity_id.clone(),
            },
            EntityGuid(ship_guid),
            OwnerId(player_entity_id.clone()),
            ActionQueue::default(),
            ActionCapabilities {
                supported: vec![
                    EntityAction::ThrustForward,
                    EntityAction::ThrustReverse,
                    EntityAction::ThrustNeutral,
                    EntityAction::Brake,
                    EntityAction::YawLeft,
                    EntityAction::YawRight,
                    EntityAction::YawNeutral,
                ],
            },
            flight_computer,
            health_pool,
            PositionM(pos),
            VelocityMps(vel),
            scanner_range,
            Transform::from_translation(pos).with_rotation(Quat::from_rotation_z(-heading_rad)),
        ));
        entity_commands.insert((
            mass_kg,
            base_mass,
            cargo_mass,
            module_mass,
            total_mass,
            MassDirty,
            inventory,
        ));
        if let Some(scanner_component) = scanner_component {
            entity_commands.insert(scanner_component);
        }
        if let Some(scanner_buff) = scanner_buff {
            entity_commands.insert(scanner_buff);
        }
        let entity = entity_commands
            .insert((
                RigidBody::Dynamic,
                Collider::cuboid(6.0, 3.0, 2.0),
                Position(pos),
                Rotation(Quat::from_rotation_z(-heading_rad)),
                LinearVelocity(vel),
                AngularVelocity::default(),
                LinearDamping(0.12),
                AngularDamping(0.35),
            ))
            .id();
        insert_registered_components(
            &mut commands,
            entity,
            &record.components,
            &type_paths,
            &app_type_registry,
        );

        controlled_entity_map
            .by_player_entity_id
            .insert(player_entity_id, entity);
        spawned_entity_by_entity_id.insert(record.entity_id.clone(), entity);
        hydrated_ships = hydrated_ships.saturating_add(1);
    }

    // Pass 2: hardpoint entities with Bevy parent-child hierarchy links.
    for record in &hardpoint_records {
        let Some(hardpoint) = hardpoint_from_record(record, &type_paths) else {
            continue;
        };
        let hardpoint_guid =
            parse_guid_from_entity_id(&record.entity_id).unwrap_or_else(uuid::Uuid::new_v4);
        let mut entity_commands = commands.spawn((
            Name::new(record.entity_id.clone()),
            EntityGuid(hardpoint_guid),
            hardpoint.clone(),
            Transform::from_translation(hardpoint.offset_m),
        ));
        if let Some(owner) = owner_id_from_record(record, &type_paths) {
            entity_commands.insert(owner);
        }
        if let Some(mass_kg) = mass_kg_from_record(record, &type_paths) {
            entity_commands.insert(mass_kg);
        }
        if let Some(inventory) = inventory_from_record(record, &type_paths) {
            entity_commands.insert(inventory);
        }
        let hardpoint_entity = entity_commands.id();
        insert_registered_components(
            &mut commands,
            hardpoint_entity,
            &record.components,
            &type_paths,
            &app_type_registry,
        );
        spawned_entity_by_entity_id.insert(record.entity_id.clone(), hardpoint_entity);
        if let Some(parent_entity_id) = record
            .properties
            .get("parent_entity_id")
            .and_then(|v| v.as_str())
            .map(ToString::to_string)
        {
            pending_parent_links.push((hardpoint_entity, parent_entity_id));
        }
        hydrated_hardpoints = hydrated_hardpoints.saturating_add(1);
    }

    // Pass 3: module entities after parent ship GUIDs are indexed.
    for record in &module_records {
        let Some(mounted_on) = mounted_on_from_record(record, &type_paths) else {
            continue;
        };
        let parent_entity_id = format!("ship:{}", mounted_on.parent_entity_id);
        if !ship_guid_by_entity_id.contains_key(&parent_entity_id) {
            continue;
        }

        let module_guid =
            parse_guid_from_entity_id(&record.entity_id).unwrap_or_else(uuid::Uuid::new_v4);
        let mut entity_commands = commands.spawn((
            Name::new(record.entity_id.clone()),
            EntityGuid(module_guid),
            mounted_on,
        ));
        if let Some(owner) = owner_id_from_record(record, &type_paths) {
            entity_commands.insert(owner);
        }
        if let Some(engine) = engine_from_record(record, &type_paths) {
            entity_commands.insert(engine);
        }
        if let Some(fuel_tank) = fuel_tank_from_record(record, &type_paths) {
            entity_commands.insert(fuel_tank);
        }
        if let Some(flight_computer) = flight_computer_from_record(record, &type_paths) {
            entity_commands.insert(flight_computer);
        }
        if let Some(scanner_range) = scanner_range_from_record(record, &type_paths) {
            entity_commands.insert(scanner_range);
        }
        if let Some(scanner_component) = scanner_component_from_record(record, &type_paths) {
            entity_commands.insert(scanner_component);
        }
        if let Some(scanner_buff) = scanner_range_buff_from_record(record, &type_paths) {
            entity_commands.insert(scanner_buff);
        }
        if let Some(mass_kg) = mass_kg_from_record(record, &type_paths) {
            entity_commands.insert(mass_kg);
        }
        if let Some(inventory) = inventory_from_record(record, &type_paths) {
            entity_commands.insert(inventory);
        }
        let module_entity = entity_commands.id();
        insert_registered_components(
            &mut commands,
            module_entity,
            &record.components,
            &type_paths,
            &app_type_registry,
        );
        spawned_entity_by_entity_id.insert(record.entity_id.clone(), module_entity);
        if let Some(parent_entity_id) = record
            .properties
            .get("parent_entity_id")
            .and_then(|v| v.as_str())
            .map(ToString::to_string)
        {
            pending_parent_links.push((module_entity, parent_entity_id));
        }
        hydrated_modules = hydrated_modules.saturating_add(1);
    }

    for (child, parent_entity_id) in pending_parent_links {
        if let Some(parent) = spawned_entity_by_entity_id.get(&parent_entity_id) {
            commands.entity(child).set_parent_in_place(*parent);
        }
    }

    println!(
        "replication simulation hydrated {hydrated_ships} entities, {hydrated_hardpoints} hardpoints and {hydrated_modules} modules"
    );
}

fn parse_vec3_value(value: &serde_json::Value) -> Option<Vec3> {
    let arr = value.as_array()?;
    if arr.len() != 3 {
        return None;
    }
    Some(Vec3::new(
        arr[0].as_f64()? as f32,
        arr[1].as_f64()? as f32,
        arr[2].as_f64()? as f32,
    ))
}

fn parse_guid_from_entity_id(entity_id: &str) -> Option<uuid::Uuid> {
    entity_id
        .split(':')
        .nth(1)
        .and_then(|raw| uuid::Uuid::parse_str(raw).ok())
}

fn component_type_path_map(registry: &GeneratedComponentRegistry) -> HashMap<String, String> {
    registry
        .entries
        .iter()
        .map(|entry| {
            (
                entry.component_kind.to_string(),
                entry.type_path.to_string(),
            )
        })
        .collect::<HashMap<_, _>>()
}

fn wrap_component_payload(
    component_kind: &str,
    payload: serde_json::Value,
    type_paths: &HashMap<String, String>,
) -> serde_json::Value {
    if let Some(type_path) = type_paths.get(component_kind) {
        encode_reflect_component(type_path, payload)
    } else {
        payload
    }
}

fn decode_component_payload<'a>(
    component: &'a GraphComponentRecord,
    type_paths: &HashMap<String, String>,
) -> Option<&'a serde_json::Value> {
    let expected_type_path = type_paths.get(&component.component_kind)?;
    decode_reflect_component(&component.properties, expected_type_path)
        .or(Some(&component.properties))
}

fn component_record<'a>(
    components: &'a [GraphComponentRecord],
    kind: &str,
) -> Option<&'a GraphComponentRecord> {
    components
        .iter()
        .find(|component| component.component_kind == kind)
}

fn insert_registered_components(
    commands: &mut Commands<'_, '_>,
    entity: Entity,
    components: &[GraphComponentRecord],
    type_paths: &HashMap<String, String>,
    app_type_registry: &AppTypeRegistry,
) {
    let type_registry = app_type_registry.read();
    for component in components {
        let Some(type_path) = type_paths.get(&component.component_kind) else {
            continue;
        };
        let Some(type_registration) = type_registry.get_with_type_path(type_path) else {
            continue;
        };
        let Some(payload) = decode_component_payload(component, type_paths) else {
            continue;
        };
        let payload_str = payload.to_string();
        let typed = TypedReflectDeserializer::new(type_registration, &type_registry);
        let mut deserializer = serde_json::Deserializer::from_str(&payload_str);
        let Ok(reflect_component) = typed.deserialize(&mut deserializer) else {
            continue;
        };
        commands.entity(entity).insert_reflect(reflect_component);
    }
}

fn serialize_registered_components_for_entity(
    world: &World,
    entity: Entity,
    entity_id: &str,
    registry: &GeneratedComponentRegistry,
    app_type_registry: &AppTypeRegistry,
    type_paths: &HashMap<String, String>,
) -> Vec<WorldComponentDelta> {
    let entity_ref = world.entity(entity);
    let type_registry = app_type_registry.read();
    let mut components = Vec::new();

    for entry in &registry.entries {
        let Some(type_registration) = type_registry.get_with_type_path(entry.type_path) else {
            continue;
        };
        let Some(reflect_component) =
            type_registration.data::<bevy::ecs::reflect::ReflectComponent>()
        else {
            continue;
        };
        let Some(reflect_value) = reflect_component.reflect(entity_ref) else {
            continue;
        };

        let serializer =
            TypedReflectSerializer::new(reflect_value.as_partial_reflect(), &type_registry);
        let Ok(payload) = serde_json::to_value(serializer) else {
            continue;
        };
        components.push(WorldComponentDelta {
            component_id: format!("{entity_id}:{}", entry.component_kind),
            component_kind: entry.component_kind.to_string(),
            properties: wrap_component_payload(entry.component_kind, payload, type_paths),
        });
    }

    components
}

fn decode_access_token(token: &str, jwt_secret: &str) -> Option<AccessTokenClaims> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_exp = true;
    decode::<AccessTokenClaims>(
        token,
        &DecodingKey::from_secret(jwt_secret.as_bytes()),
        &validation,
    )
    .ok()
    .map(|decoded| decoded.claims)
}

fn apply_range_buff(base_range_m: f32, buff: &ScannerRangeBuff) -> f32 {
    let multiplier = if buff.multiplier <= 0.0 {
        1.0
    } else {
        buff.multiplier
    };
    (base_range_m + buff.additive_m).max(0.0) * multiplier
}

fn compute_scanner_contribution(
    scanner: &ScannerComponent,
    buff: Option<&ScannerRangeBuff>,
) -> f32 {
    let level_multiplier = if scanner.level == 0 {
        1.0
    } else {
        scanner.level as f32
    };
    let base = scanner.base_range_m.max(0.0) * level_multiplier;
    if let Some(buff) = buff {
        apply_range_buff(base, buff)
    } else {
        base
    }
}

fn owner_id_from_record(
    record: &sidereal_persistence::GraphEntityRecord,
    type_paths: &HashMap<String, String>,
) -> Option<OwnerId> {
    let component = component_record(&record.components, "owner_id")?;
    let payload = decode_component_payload(component, type_paths)?;
    serde_json::from_value::<OwnerId>(payload.clone()).ok()
}

fn health_pool_from_record(
    record: &sidereal_persistence::GraphEntityRecord,
    type_paths: &HashMap<String, String>,
) -> Option<HealthPool> {
    let component = component_record(&record.components, "health_pool")?;
    let payload = decode_component_payload(component, type_paths)?;
    serde_json::from_value::<HealthPool>(payload.clone()).ok()
}

fn flight_computer_from_record(
    record: &sidereal_persistence::GraphEntityRecord,
    type_paths: &HashMap<String, String>,
) -> Option<FlightComputer> {
    let component = component_record(&record.components, "flight_computer")?;
    let payload = decode_component_payload(component, type_paths)?;
    serde_json::from_value::<FlightComputer>(payload.clone()).ok()
}

fn mounted_on_from_record(
    record: &sidereal_persistence::GraphEntityRecord,
    type_paths: &HashMap<String, String>,
) -> Option<MountedOn> {
    let component = component_record(&record.components, "mounted_on")?;
    let payload = decode_component_payload(component, type_paths)?;
    serde_json::from_value::<MountedOn>(payload.clone()).ok()
}

fn hardpoint_from_record(
    record: &sidereal_persistence::GraphEntityRecord,
    type_paths: &HashMap<String, String>,
) -> Option<Hardpoint> {
    let component = component_record(&record.components, "hardpoint")?;
    let payload = decode_component_payload(component, type_paths)?;
    serde_json::from_value::<Hardpoint>(payload.clone()).ok()
}

fn engine_from_record(
    record: &sidereal_persistence::GraphEntityRecord,
    type_paths: &HashMap<String, String>,
) -> Option<Engine> {
    let component = component_record(&record.components, "engine")?;
    let payload = decode_component_payload(component, type_paths)?;
    serde_json::from_value::<Engine>(payload.clone()).ok()
}

fn fuel_tank_from_record(
    record: &sidereal_persistence::GraphEntityRecord,
    type_paths: &HashMap<String, String>,
) -> Option<FuelTank> {
    let component = component_record(&record.components, "fuel_tank")?;
    let payload = decode_component_payload(component, type_paths)?;
    serde_json::from_value::<FuelTank>(payload.clone()).ok()
}

fn mass_kg_from_record(
    record: &sidereal_persistence::GraphEntityRecord,
    type_paths: &HashMap<String, String>,
) -> Option<MassKg> {
    let component = component_record(&record.components, "mass_kg")?;
    let payload = decode_component_payload(component, type_paths)?;
    serde_json::from_value::<MassKg>(payload.clone()).ok()
}

fn base_mass_from_record(
    record: &sidereal_persistence::GraphEntityRecord,
    type_paths: &HashMap<String, String>,
) -> Option<BaseMassKg> {
    let component = component_record(&record.components, "base_mass_kg")?;
    let payload = decode_component_payload(component, type_paths)?;
    serde_json::from_value::<BaseMassKg>(payload.clone()).ok()
}

fn cargo_mass_from_record(
    record: &sidereal_persistence::GraphEntityRecord,
    type_paths: &HashMap<String, String>,
) -> Option<CargoMassKg> {
    let component = component_record(&record.components, "cargo_mass_kg")?;
    let payload = decode_component_payload(component, type_paths)?;
    serde_json::from_value::<CargoMassKg>(payload.clone()).ok()
}

fn module_mass_from_record(
    record: &sidereal_persistence::GraphEntityRecord,
    type_paths: &HashMap<String, String>,
) -> Option<ModuleMassKg> {
    let component = component_record(&record.components, "module_mass_kg")?;
    let payload = decode_component_payload(component, type_paths)?;
    serde_json::from_value::<ModuleMassKg>(payload.clone()).ok()
}

fn total_mass_from_record(
    record: &sidereal_persistence::GraphEntityRecord,
    type_paths: &HashMap<String, String>,
) -> Option<TotalMassKg> {
    let component = component_record(&record.components, "total_mass_kg")?;
    let payload = decode_component_payload(component, type_paths)?;
    serde_json::from_value::<TotalMassKg>(payload.clone()).ok()
}

fn inventory_from_record(
    record: &sidereal_persistence::GraphEntityRecord,
    type_paths: &HashMap<String, String>,
) -> Option<Inventory> {
    let component = component_record(&record.components, "inventory")?;
    let payload = decode_component_payload(component, type_paths)?;
    serde_json::from_value::<Inventory>(payload.clone()).ok()
}

fn scanner_range_from_record(
    record: &sidereal_persistence::GraphEntityRecord,
    type_paths: &HashMap<String, String>,
) -> Option<ScannerRangeM> {
    if let Some(component) = component_record(&record.components, "scanner_range_m") {
        let payload = decode_component_payload(component, type_paths)?;
        if let Ok(range) = serde_json::from_value::<ScannerRangeM>(payload.clone()) {
            return Some(range);
        }
        if let Some(value) = payload.as_f64() {
            return Some(ScannerRangeM(value as f32));
        }
    }
    record
        .properties
        .get("scanner_range_m")
        .and_then(|v| v.as_f64())
        .map(|v| ScannerRangeM(v as f32))
}

fn scanner_component_from_record(
    record: &sidereal_persistence::GraphEntityRecord,
    type_paths: &HashMap<String, String>,
) -> Option<ScannerComponent> {
    let component = component_record(&record.components, "scanner_component")?;
    let payload = decode_component_payload(component, type_paths)?;
    serde_json::from_value::<ScannerComponent>(payload.clone()).ok()
}

fn scanner_range_buff_from_record(
    record: &sidereal_persistence::GraphEntityRecord,
    type_paths: &HashMap<String, String>,
) -> Option<ScannerRangeBuff> {
    let component = component_record(&record.components, "scanner_range_buff")?;
    let payload = decode_component_payload(component, type_paths)?;
    serde_json::from_value::<ScannerRangeBuff>(payload.clone()).ok()
}

fn start_replication_control_listener(mut commands: Commands<'_, '_>) {
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

    println!("replication control UDP listening on {bind_addr}");
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

fn process_bootstrap_ship_commands(
    mut commands: Commands<'_, '_>,
    mut controlled_entity_map: ResMut<'_, PlayerControlledEntityMap>,
    receiver: Option<Res<'_, BootstrapShipReceiver>>,
) {
    let Some(receiver) = receiver else { return };
    let Ok(rx) = receiver.0.lock() else { return };

    while let Ok(cmd) = rx.try_recv() {
        if controlled_entity_map
            .by_player_entity_id
            .contains_key(&cmd.player_entity_id)
        {
            continue;
        }
        println!(
            "spawning bootstrapped ship {} for {}",
            cmd.ship_entity_id, cmd.player_entity_id
        );
        spawn_simulation_entity(
            &mut commands,
            &mut controlled_entity_map,
            &cmd.ship_entity_id,
            &cmd.player_entity_id,
            Vec3::ZERO,
            Vec3::ZERO,
            100.0,
            100.0,
        );
    }
}

fn bootstrap_starter_ship(
    database_url: &str,
    account_id: uuid::Uuid,
    player_entity_id: &str,
) -> sidereal_persistence::Result<()> {
    let mut persistence = GraphPersistence::connect(database_url)?;
    persistence.ensure_schema()?;

    let ship_entity_id = format!("ship:{account_id}");
    let account_id_s = account_id.to_string();
    let starter_world = vec![
        WorldDeltaEntity {
            entity_id: player_entity_id.to_string(),
            labels: vec!["Entity".to_string(), "Player".to_string()],
            properties: serde_json::json!({
                "owner_account_id": account_id_s,
                "player_entity_id": player_entity_id,
            }),
            components: vec![WorldComponentDelta {
                component_id: format!("{player_entity_id}:display_name"),
                component_kind: "display_name".to_string(),
                properties: serde_json::json!({"value": "Pilot"}),
            }],
            removed: false,
        },
        WorldDeltaEntity {
            entity_id: ship_entity_id.clone(),
            labels: vec!["Entity".to_string(), "Ship".to_string()],
            properties: serde_json::json!({
                "owner_account_id": account_id.to_string(),
                "player_entity_id": player_entity_id,
                "name": "Corvette",
                "asset_id": "corvette_01",
                "starfield_shader_asset_id": "starfield_wgsl",
                "position_m": [0.0, 0.0, 0.0],
                "velocity_mps": [0.0, 0.0, 0.0],
                "heading_rad": 0.0,
                "engine_max_accel_mps2": 171_000.0,
                "engine_ramp_to_max_s": 5.0,
                "health": 100.0,
                "max_health": 100.0
            }),
            components: vec![
                WorldComponentDelta {
                    component_id: format!("{ship_entity_id}:display_name"),
                    component_kind: "display_name".to_string(),
                    properties: serde_json::json!({"value": "Corvette"}),
                },
                WorldComponentDelta {
                    component_id: format!("{ship_entity_id}:flight_computer"),
                    component_kind: "flight_computer".to_string(),
                    properties: serde_json::json!({"profile": "ManualAssist", "throttle": 0.0}),
                },
                WorldComponentDelta {
                    component_id: format!("{ship_entity_id}:health_pool"),
                    component_kind: "health_pool".to_string(),
                    properties: serde_json::json!({"hp": 100.0, "max_hp": 100.0}),
                },
            ],
            removed: false,
        },
    ];
    persistence.persist_world_delta(&starter_world, 0)?;
    Ok(())
}

fn init_replication_runtime(world: &mut World) {
    let database_url = std::env::var("REPLICATION_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://sidereal:sidereal@127.0.0.1:5432/sidereal".to_string());
    let persist_interval_s = std::env::var("REPLICATION_PERSIST_INTERVAL_S")
        .ok()
        .and_then(|v| v.parse::<f32>().ok())
        .filter(|v| *v > 0.0)
        .unwrap_or(15.0);
    let snapshot_interval_s = std::env::var("SNAPSHOT_INTERVAL_S")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(15);

    let mut persistence = match GraphPersistence::connect(&database_url) {
        Ok(v) => v,
        Err(err) => {
            eprintln!("replication runtime init failed to connect persistence: {err}");
            return;
        }
    };
    if let Err(err) = persistence.ensure_schema() {
        eprintln!("replication runtime init failed to ensure schema: {err}");
        return;
    }
    let known_entities = match hydrate_known_entity_ids(&mut persistence) {
        Ok(entity_ids) => entity_ids,
        Err(err) => {
            eprintln!("replication runtime init failed initial graph load: {err}");
            HashSet::new()
        }
    };

    let persist_interval = Duration::from_secs_f32(persist_interval_s);
    let snapshot_interval = Duration::from_secs(snapshot_interval_s);
    world.insert_non_send_resource(ReplicationRuntime {
        persistence,
        known_entities,
        pending_updates: HashMap::new(),
        last_tick: 0,
        persist_interval,
        snapshot_interval,
        last_persist_at: Instant::now() - persist_interval,
        last_snapshot_at: Instant::now(),
        last_persisted_state: HashMap::new(),
    });
}

fn start_lightyear_server(mut commands: Commands<'_, '_>) {
    let bind_addr = std::env::var("REPLICATION_UDP_BIND")
        .unwrap_or_else(|_| "0.0.0.0:7001".to_string())
        .parse::<SocketAddr>();
    let bind_addr = match bind_addr {
        Ok(v) => v,
        Err(err) => {
            eprintln!("invalid REPLICATION_UDP_BIND: {err}");
            return;
        }
    };

    let server = commands
        .spawn((
            Name::new("replication-lightyear-server"),
            RawServer,
            ServerUdpIo::default(),
            LocalAddr(bind_addr),
            Stopped,
        ))
        .id();
    commands.trigger(Start { entity: server });
    println!("replication lightyear UDP server starting on {bind_addr}");
}

fn ensure_server_transport_channels(
    mut transports: Query<'_, '_, &mut Transport, With<ClientOf>>,
    registry: Res<'_, ChannelRegistry>,
) {
    for mut transport in &mut transports {
        if !transport.has_receiver::<ControlChannel>() {
            transport.add_receiver_from_registry::<ControlChannel>(&registry);
        }
        if !transport.has_receiver::<StateChannel>() {
            transport.add_receiver_from_registry::<StateChannel>(&registry);
        }
        if !transport.has_receiver::<InputChannel>() {
            transport.add_receiver_from_registry::<InputChannel>(&registry);
        }
        if !transport.has_sender::<StateChannel>() {
            transport.add_sender_from_registry::<StateChannel>(&registry);
        }
    }
}

fn cleanup_client_auth_bindings(
    clients: Query<'_, '_, (Entity, &RemoteId), With<ClientOf>>,
    mut bindings: ResMut<'_, AuthenticatedClientBindings>,
) {
    let live_clients = clients
        .iter()
        .map(|(entity, _)| entity)
        .collect::<HashSet<_>>();
    let live_remote_ids = clients
        .iter()
        .map(|(_, remote_id)| remote_id.0)
        .collect::<HashSet<_>>();
    bindings
        .by_client_entity
        .retain(|client_entity, _| live_clients.contains(client_entity));
    bindings
        .by_remote_id
        .retain(|remote_id, _| live_remote_ids.contains(remote_id));
}

fn receive_client_auth_messages(
    mut auth_receivers: Query<
        '_,
        '_,
        (Entity, &RemoteId, &mut MessageReceiver<ClientAuthMessage>),
        With<ClientOf>,
    >,
    mut visibility_registry: ResMut<'_, ClientVisibilityRegistry>,
    mut bindings: ResMut<'_, AuthenticatedClientBindings>,
) {
    let jwt_secret = match std::env::var("GATEWAY_JWT_SECRET") {
        Ok(secret) if secret.len() >= 32 => secret,
        _ => return,
    };

    for (client_entity, remote_id, mut receiver) in &mut auth_receivers {
        for message in receiver.receive() {
            let claims = match decode_access_token(&message.access_token, &jwt_secret) {
                Some(claims) => claims,
                None => {
                    eprintln!(
                        "replication rejected client auth: invalid token for client {:?}",
                        client_entity
                    );
                    continue;
                }
            };
            if claims.player_entity_id != message.player_entity_id {
                eprintln!(
                    "replication rejected client auth: token player mismatch for client {:?}",
                    client_entity
                );
                continue;
            }

            if let Some(bound_player) = bindings.by_remote_id.get(&remote_id.0)
                && bound_player != &claims.player_entity_id
            {
                eprintln!(
                    "replication rejected client auth: remote {:?} already bound to {}",
                    remote_id.0, bound_player
                );
                continue;
            }

            bindings
                .by_client_entity
                .insert(client_entity, claims.player_entity_id.clone());
            bindings
                .by_remote_id
                .insert(remote_id.0, claims.player_entity_id.clone());
            visibility_registry.register_client(client_entity, claims.player_entity_id);
        }
    }
}

fn receive_client_inputs(
    mut receivers: Query<
        '_,
        '_,
        (Entity, &mut MessageReceiver<ClientInputMessage>),
        With<ClientOf>,
    >,
    controlled_entity_map: Res<'_, PlayerControlledEntityMap>,
    bindings: Res<'_, AuthenticatedClientBindings>,
    mut actions: Query<'_, '_, &mut ActionQueue, With<SimulatedControlledEntity>>,
) {
    for (client_entity, mut receiver) in &mut receivers {
        for message in receiver.receive() {
            let Some(bound_player) = bindings.by_client_entity.get(&client_entity) else {
                continue;
            };
            if bound_player != &message.player_entity_id {
                eprintln!(
                    "replication dropped spoofed input for client {:?}: claimed={}, bound={}",
                    client_entity, message.player_entity_id, bound_player
                );
                continue;
            }
            if let Some(controlled_entity) =
                controlled_entity_map.by_player_entity_id.get(bound_player)
                && let Ok(mut queue) = actions.get_mut(*controlled_entity)
            {
                for action in &message.actions {
                    queue.push(*action);
                }
            }
        }
    }
}

/// Update controlled-entity positions so visibility filtering can apply delivery culling.
fn update_client_controlled_entity_positions(
    entities: Query<'_, '_, (&SimulatedControlledEntity, &Position)>,
    mut position_map: ResMut<'_, ClientControlledEntityPositionMap>,
) {
    for (entity, position) in &entities {
        position_map.update_position(&entity.player_entity_id, position.0);
    }
}

#[allow(clippy::type_complexity)]
fn compute_controlled_entity_scanner_ranges(
    mut controlled_entities: Query<
        '_,
        '_,
        (
            &EntityGuid,
            &mut ScannerRangeM,
            Option<&ScannerComponent>,
            Option<&ScannerRangeBuff>,
        ),
        With<SimulatedControlledEntity>,
    >,
    scanner_modules: Query<
        '_,
        '_,
        (&MountedOn, &ScannerComponent, Option<&ScannerRangeBuff>),
        Without<SimulatedControlledEntity>,
    >,
) {
    for (entity_guid, mut scanner_range, own_scanner, own_buff) in &mut controlled_entities {
        let mut total_range = visibility::DEFAULT_VIEW_RANGE_M;

        if let Some(scanner) = own_scanner {
            total_range += compute_scanner_contribution(scanner, own_buff);
        } else if let Some(buff) = own_buff {
            total_range = apply_range_buff(total_range, buff);
        }

        for (mounted_on, scanner, buff) in &scanner_modules {
            if mounted_on.parent_entity_id == entity_guid.0 {
                total_range += compute_scanner_contribution(scanner, buff);
            }
        }

        scanner_range.0 = total_range.max(visibility::DEFAULT_VIEW_RANGE_M);
    }
}

fn sync_simulated_ship_components(
    mut ships: Query<
        '_,
        '_,
        (
            &Position,
            &LinearVelocity,
            &mut Transform,
            &mut PositionM,
            &mut VelocityMps,
        ),
        With<SimulatedControlledEntity>,
    >,
) {
    for (position, velocity, mut transform, mut position_m, mut velocity_mps) in &mut ships {
        transform.translation = position.0;
        position_m.0 = position.0;
        velocity_mps.0 = velocity.0;
    }
}

#[allow(clippy::type_complexity)]
#[allow(clippy::too_many_arguments)]
fn collect_local_simulation_state(
    ships: Query<
        '_,
        '_,
        (
            Entity,
            &SimulatedControlledEntity,
            &PositionM,
            &VelocityMps,
            &HealthPool,
            &FlightComputer,
            &OwnerId,
            &Rotation,
            Option<&ScannerRangeM>,
            Option<&ScannerComponent>,
            Option<&ScannerRangeBuff>,
        ),
    >,
    ship_mass_meta: Query<
        '_,
        '_,
        (
            Option<&MassKg>,
            Option<&BaseMassKg>,
            Option<&CargoMassKg>,
            Option<&ModuleMassKg>,
            Option<&TotalMassKg>,
            Option<&Inventory>,
        ),
    >,
    hardpoints: Query<
        '_,
        '_,
        (
            &EntityGuid,
            &Hardpoint,
            Option<&ChildOf>,
            Option<&OwnerId>,
            Option<&MassKg>,
            Option<&Inventory>,
        ),
        Without<SimulatedControlledEntity>,
    >,
    modules: Query<
        '_,
        '_,
        (
            &EntityGuid,
            &MountedOn,
            Option<&Engine>,
            Option<&FuelTank>,
            Option<&FlightComputer>,
            Option<&OwnerId>,
            Option<&ScannerRangeM>,
            Option<&ScannerComponent>,
            Option<&ScannerRangeBuff>,
            Option<&MassKg>,
            Option<&Inventory>,
        ),
        Without<SimulatedControlledEntity>,
    >,
    guid_lookup: Query<'_, '_, (Entity, &EntityGuid)>,
    component_registry: Res<'_, GeneratedComponentRegistry>,
    runtime: Option<NonSendMut<'_, ReplicationRuntime>>,
    mut outbound: ResMut<'_, ReplicationOutboundQueue>,
) {
    let Some(mut runtime) = runtime else {
        return;
    };

    let mut broadcast_updates = Vec::new();
    let mut dirty_updates = Vec::new();
    let type_paths = component_type_path_map(&component_registry);

    for (
        ship_entity,
        controlled_entity,
        position,
        velocity,
        health,
        flight,
        owner,
        rotation,
        scanner_range,
        scanner_component,
        scanner_buff,
    ) in &ships
    {
        let (mass_kg, base_mass, cargo_mass, module_mass, total_mass, inventory) = ship_mass_meta
            .get(ship_entity)
            .ok()
            .map(|(a, b, c, d, e, f)| {
                (
                    a.copied(),
                    b.copied(),
                    c.copied(),
                    d.copied(),
                    e.copied(),
                    f,
                )
            })
            .unwrap_or((None, None, None, None, None, None));
        let heading_rad = rotation.0.to_euler(EulerRot::ZYX).0;

        let mut delta_entity = WorldDeltaEntity {
            entity_id: controlled_entity.entity_id.clone(),
            labels: vec!["Entity".to_string(), "Ship".to_string()],
            properties: serde_json::json!({
                "entity_id": controlled_entity.entity_id.as_str(),
                "player_entity_id": controlled_entity.player_entity_id.as_str(),
                "position_m": [position.0.x, position.0.y, position.0.z],
                "velocity_mps": [velocity.0.x, velocity.0.y, velocity.0.z],
                "heading_rad": heading_rad,
                "health": health.current,
                "max_health": health.maximum,
                "scanner_range_m": scanner_range.map(|r| r.0).unwrap_or(0.0),
                "mass_kg": mass_kg.map(|m| m.0).unwrap_or(0.0),
                "base_mass_kg": base_mass.map(|m| m.0).unwrap_or(0.0),
                "cargo_mass_kg": cargo_mass.map(|m| m.0).unwrap_or(0.0),
                "module_mass_kg": module_mass.map(|m| m.0).unwrap_or(0.0),
                "total_mass_kg": total_mass.map(|m| m.0).unwrap_or(0.0),
            }),
            components: vec![
                WorldComponentDelta {
                    component_id: format!("{}:owner_id", controlled_entity.entity_id),
                    component_kind: "owner_id".to_string(),
                    properties: wrap_component_payload(
                        "owner_id",
                        serde_json::to_value(owner).unwrap_or_else(|_| serde_json::json!(owner.0)),
                        &type_paths,
                    ),
                },
                WorldComponentDelta {
                    component_id: format!("{}:flight_computer", controlled_entity.entity_id),
                    component_kind: "flight_computer".to_string(),
                    properties: wrap_component_payload(
                        "flight_computer",
                        serde_json::to_value(flight).unwrap_or_else(|_| {
                            serde_json::json!({
                                "profile": flight.profile.as_str(),
                                "throttle": flight.throttle,
                                "yaw_input": flight.yaw_input,
                                "turn_rate_deg_s": flight.turn_rate_deg_s,
                            })
                        }),
                        &type_paths,
                    ),
                },
                WorldComponentDelta {
                    component_id: format!("{}:health_pool", controlled_entity.entity_id),
                    component_kind: "health_pool".to_string(),
                    properties: wrap_component_payload(
                        "health_pool",
                        serde_json::to_value(health).unwrap_or_else(|_| {
                            serde_json::json!({
                                "current": health.current,
                                "maximum": health.maximum,
                            })
                        }),
                        &type_paths,
                    ),
                },
                WorldComponentDelta {
                    component_id: format!("{}:scanner_range_m", controlled_entity.entity_id),
                    component_kind: "scanner_range_m".to_string(),
                    properties: wrap_component_payload(
                        "scanner_range_m",
                        serde_json::json!(scanner_range.map(|r| r.0).unwrap_or(0.0)),
                        &type_paths,
                    ),
                },
            ],
            removed: false,
        };
        if let Some(mass_kg) = mass_kg {
            delta_entity.components.push(WorldComponentDelta {
                component_id: format!("{}:mass_kg", controlled_entity.entity_id),
                component_kind: "mass_kg".to_string(),
                properties: wrap_component_payload(
                    "mass_kg",
                    serde_json::to_value(mass_kg).unwrap_or_else(|_| serde_json::json!(mass_kg.0)),
                    &type_paths,
                ),
            });
        }
        if let Some(base_mass) = base_mass {
            delta_entity.components.push(WorldComponentDelta {
                component_id: format!("{}:base_mass_kg", controlled_entity.entity_id),
                component_kind: "base_mass_kg".to_string(),
                properties: wrap_component_payload(
                    "base_mass_kg",
                    serde_json::to_value(base_mass)
                        .unwrap_or_else(|_| serde_json::json!(base_mass.0)),
                    &type_paths,
                ),
            });
        }
        if let Some(cargo_mass) = cargo_mass {
            delta_entity.components.push(WorldComponentDelta {
                component_id: format!("{}:cargo_mass_kg", controlled_entity.entity_id),
                component_kind: "cargo_mass_kg".to_string(),
                properties: wrap_component_payload(
                    "cargo_mass_kg",
                    serde_json::to_value(cargo_mass)
                        .unwrap_or_else(|_| serde_json::json!(cargo_mass.0)),
                    &type_paths,
                ),
            });
        }
        if let Some(module_mass) = module_mass {
            delta_entity.components.push(WorldComponentDelta {
                component_id: format!("{}:module_mass_kg", controlled_entity.entity_id),
                component_kind: "module_mass_kg".to_string(),
                properties: wrap_component_payload(
                    "module_mass_kg",
                    serde_json::to_value(module_mass)
                        .unwrap_or_else(|_| serde_json::json!(module_mass.0)),
                    &type_paths,
                ),
            });
        }
        if let Some(total_mass) = total_mass {
            delta_entity.components.push(WorldComponentDelta {
                component_id: format!("{}:total_mass_kg", controlled_entity.entity_id),
                component_kind: "total_mass_kg".to_string(),
                properties: wrap_component_payload(
                    "total_mass_kg",
                    serde_json::to_value(total_mass)
                        .unwrap_or_else(|_| serde_json::json!(total_mass.0)),
                    &type_paths,
                ),
            });
        }
        if let Some(inventory) = inventory {
            delta_entity.components.push(WorldComponentDelta {
                component_id: format!("{}:inventory", controlled_entity.entity_id),
                component_kind: "inventory".to_string(),
                properties: wrap_component_payload(
                    "inventory",
                    serde_json::to_value(inventory).unwrap_or_else(|_| {
                        serde_json::json!({
                            "entries": []
                        })
                    }),
                    &type_paths,
                ),
            });
        }
        if let Some(scanner_component) = scanner_component {
            delta_entity.components.push(WorldComponentDelta {
                component_id: format!("{}:scanner_component", controlled_entity.entity_id),
                component_kind: "scanner_component".to_string(),
                properties: wrap_component_payload(
                    "scanner_component",
                    serde_json::to_value(scanner_component).unwrap_or_else(|_| {
                        serde_json::json!({
                            "base_range_m": scanner_component.base_range_m,
                            "level": scanner_component.level,
                        })
                    }),
                    &type_paths,
                ),
            });
        }
        if let Some(scanner_buff) = scanner_buff {
            delta_entity.components.push(WorldComponentDelta {
                component_id: format!("{}:scanner_range_buff", controlled_entity.entity_id),
                component_kind: "scanner_range_buff".to_string(),
                properties: wrap_component_payload(
                    "scanner_range_buff",
                    serde_json::to_value(scanner_buff).unwrap_or_else(|_| {
                        serde_json::json!({
                            "additive_m": scanner_buff.additive_m,
                            "multiplier": scanner_buff.multiplier,
                        })
                    }),
                    &type_paths,
                ),
            });
        }

        broadcast_updates.push(delta_entity.clone());

        // Dirty check for persistence: only persist if state materially changed
        let is_dirty = if let Some(last) = runtime
            .last_persisted_state
            .get(&controlled_entity.entity_id)
        {
            (position.0 - last.position).length() > PERSISTENCE_POSITION_THRESHOLD
                || (velocity.0 - last.velocity).length() > PERSISTENCE_VELOCITY_THRESHOLD
                || (health.current - last.health).abs() > PERSISTENCE_HEALTH_THRESHOLD
        } else {
            true
        };

        if is_dirty {
            runtime.last_persisted_state.insert(
                controlled_entity.entity_id.clone(),
                PersistedEntitySnapshot {
                    position: position.0,
                    velocity: velocity.0,
                    health: health.current,
                },
            );
            dirty_updates.push(delta_entity);
        }
    }

    let mut entity_id_by_entity = guid_lookup
        .iter()
        .map(|(entity, guid)| (entity, format!("entity:{}", guid.0)))
        .collect::<HashMap<_, _>>();
    for (ship_entity, controlled_entity, ..) in &ships {
        entity_id_by_entity.insert(ship_entity, controlled_entity.entity_id.clone());
    }
    for (entity_guid, _, _, _, _, _) in &hardpoints {
        if let Some((entity, _)) = guid_lookup.iter().find(|(_, guid)| guid.0 == entity_guid.0) {
            entity_id_by_entity.insert(entity, format!("hardpoint:{}", entity_guid.0));
        }
    }
    for (entity_guid, _, _, _, _, _, _, _, _, _, _) in &modules {
        if let Some((entity, _)) = guid_lookup.iter().find(|(_, guid)| guid.0 == entity_guid.0) {
            entity_id_by_entity.insert(entity, format!("module:{}", entity_guid.0));
        }
    }

    for (entity_guid, hardpoint, child_of, owner_id, mass_kg, inventory) in &hardpoints {
        let hardpoint_entity_id = format!("hardpoint:{}", entity_guid.0);
        let parent_entity_id = child_of
            .and_then(|child| entity_id_by_entity.get(&child.parent()))
            .cloned()
            .unwrap_or_else(|| "entity:unknown".to_string());
        let mut components = vec![WorldComponentDelta {
            component_id: format!("{hardpoint_entity_id}:hardpoint"),
            component_kind: "hardpoint".to_string(),
            properties: wrap_component_payload(
                "hardpoint",
                serde_json::to_value(hardpoint).unwrap_or_else(|_| {
                    serde_json::json!({
                        "hardpoint_id": hardpoint.hardpoint_id,
                        "offset_m": [hardpoint.offset_m.x, hardpoint.offset_m.y, hardpoint.offset_m.z],
                    })
                }),
                &type_paths,
            ),
        }];
        if let Some(owner_id) = owner_id {
            components.push(WorldComponentDelta {
                component_id: format!("{hardpoint_entity_id}:owner_id"),
                component_kind: "owner_id".to_string(),
                properties: wrap_component_payload(
                    "owner_id",
                    serde_json::to_value(owner_id)
                        .unwrap_or_else(|_| serde_json::json!(owner_id.0.clone())),
                    &type_paths,
                ),
            });
        }
        if let Some(mass_kg) = mass_kg {
            components.push(WorldComponentDelta {
                component_id: format!("{hardpoint_entity_id}:mass_kg"),
                component_kind: "mass_kg".to_string(),
                properties: wrap_component_payload(
                    "mass_kg",
                    serde_json::to_value(mass_kg).unwrap_or_else(|_| serde_json::json!(mass_kg.0)),
                    &type_paths,
                ),
            });
        }
        if let Some(inventory) = inventory {
            components.push(WorldComponentDelta {
                component_id: format!("{hardpoint_entity_id}:inventory"),
                component_kind: "inventory".to_string(),
                properties: wrap_component_payload(
                    "inventory",
                    serde_json::to_value(inventory).unwrap_or_else(|_| {
                        serde_json::json!({
                            "entries": []
                        })
                    }),
                    &type_paths,
                ),
            });
        }
        let hardpoint_delta = WorldDeltaEntity {
            entity_id: hardpoint_entity_id,
            labels: vec!["Entity".to_string(), "Hardpoint".to_string()],
            properties: serde_json::json!({
                "hardpoint_id": hardpoint.hardpoint_id,
                "offset_m": [hardpoint.offset_m.x, hardpoint.offset_m.y, hardpoint.offset_m.z],
                "parent_entity_id": parent_entity_id,
                "owner_entity_id": parent_entity_id,
            }),
            components,
            removed: false,
        };
        broadcast_updates.push(hardpoint_delta.clone());
        dirty_updates.push(hardpoint_delta);
    }

    for (
        entity_guid,
        mounted_on,
        engine,
        fuel_tank,
        flight_computer,
        owner_id,
        scanner_range,
        scanner_component,
        scanner_buff,
        mass_kg,
        inventory,
    ) in &modules
    {
        let module_entity_id = format!("module:{}", entity_guid.0);
        let mounted_on_entity_id = format!("ship:{}", mounted_on.parent_entity_id);

        let mut components = vec![WorldComponentDelta {
            component_id: format!("{module_entity_id}:mounted_on"),
            component_kind: "mounted_on".to_string(),
            properties: wrap_component_payload(
                "mounted_on",
                serde_json::to_value(mounted_on).unwrap_or_else(
                    |_| serde_json::json!({"parent_entity_id": mounted_on.parent_entity_id, "hardpoint_id": mounted_on.hardpoint_id}),
                ),
                &type_paths,
            ),
        }];
        if let Some(owner) = owner_id {
            components.push(WorldComponentDelta {
                component_id: format!("{module_entity_id}:owner_id"),
                component_kind: "owner_id".to_string(),
                properties: wrap_component_payload(
                    "owner_id",
                    serde_json::to_value(owner)
                        .unwrap_or_else(|_| serde_json::json!(owner.0.clone())),
                    &type_paths,
                ),
            });
        }
        if let Some(engine) = engine {
            components.push(WorldComponentDelta {
                component_id: format!("{module_entity_id}:engine"),
                component_kind: "engine".to_string(),
                properties: wrap_component_payload(
                    "engine",
                    serde_json::to_value(engine).unwrap_or_else(|_| serde_json::json!({
                        "thrust_n": engine.thrust_n,
                        "burn_rate_kg_s": engine.burn_rate_kg_s,
                        "thrust_dir": [engine.thrust_dir.x, engine.thrust_dir.y, engine.thrust_dir.z],
                    })),
                    &type_paths,
                ),
            });
        }
        if let Some(fuel_tank) = fuel_tank {
            components.push(WorldComponentDelta {
                component_id: format!("{module_entity_id}:fuel_tank"),
                component_kind: "fuel_tank".to_string(),
                properties: wrap_component_payload(
                    "fuel_tank",
                    serde_json::to_value(fuel_tank)
                        .unwrap_or_else(|_| serde_json::json!({"fuel_kg": fuel_tank.fuel_kg})),
                    &type_paths,
                ),
            });
        }
        if let Some(flight_computer) = flight_computer {
            components.push(WorldComponentDelta {
                component_id: format!("{module_entity_id}:flight_computer"),
                component_kind: "flight_computer".to_string(),
                properties: wrap_component_payload(
                    "flight_computer",
                    serde_json::to_value(flight_computer).unwrap_or_else(|_| {
                        serde_json::json!({
                            "profile": flight_computer.profile.as_str(),
                            "throttle": flight_computer.throttle,
                            "yaw_input": flight_computer.yaw_input,
                            "turn_rate_deg_s": flight_computer.turn_rate_deg_s,
                        })
                    }),
                    &type_paths,
                ),
            });
        }
        if let Some(scanner_range) = scanner_range {
            components.push(WorldComponentDelta {
                component_id: format!("{module_entity_id}:scanner_range_m"),
                component_kind: "scanner_range_m".to_string(),
                properties: wrap_component_payload(
                    "scanner_range_m",
                    serde_json::json!(scanner_range.0),
                    &type_paths,
                ),
            });
        }
        if let Some(scanner_component) = scanner_component {
            components.push(WorldComponentDelta {
                component_id: format!("{module_entity_id}:scanner_component"),
                component_kind: "scanner_component".to_string(),
                properties: wrap_component_payload(
                    "scanner_component",
                    serde_json::to_value(scanner_component).unwrap_or_else(|_| {
                        serde_json::json!({
                            "base_range_m": scanner_component.base_range_m,
                            "level": scanner_component.level,
                        })
                    }),
                    &type_paths,
                ),
            });
        }
        if let Some(scanner_buff) = scanner_buff {
            components.push(WorldComponentDelta {
                component_id: format!("{module_entity_id}:scanner_range_buff"),
                component_kind: "scanner_range_buff".to_string(),
                properties: wrap_component_payload(
                    "scanner_range_buff",
                    serde_json::to_value(scanner_buff).unwrap_or_else(|_| {
                        serde_json::json!({
                            "additive_m": scanner_buff.additive_m,
                            "multiplier": scanner_buff.multiplier,
                        })
                    }),
                    &type_paths,
                ),
            });
        }
        if let Some(mass_kg) = mass_kg {
            components.push(WorldComponentDelta {
                component_id: format!("{module_entity_id}:mass_kg"),
                component_kind: "mass_kg".to_string(),
                properties: wrap_component_payload(
                    "mass_kg",
                    serde_json::to_value(mass_kg).unwrap_or_else(|_| serde_json::json!(mass_kg.0)),
                    &type_paths,
                ),
            });
        }
        if let Some(inventory) = inventory {
            components.push(WorldComponentDelta {
                component_id: format!("{module_entity_id}:inventory"),
                component_kind: "inventory".to_string(),
                properties: wrap_component_payload(
                    "inventory",
                    serde_json::to_value(inventory).unwrap_or_else(|_| {
                        serde_json::json!({
                            "entries": []
                        })
                    }),
                    &type_paths,
                ),
            });
        }

        let module_delta = WorldDeltaEntity {
            entity_id: module_entity_id.clone(),
            labels: vec!["Entity".to_string(), "Module".to_string()],
            properties: serde_json::json!({
                "entity_id": module_entity_id,
                "mounted_on_entity_id": mounted_on_entity_id,
                "parent_entity_id": mounted_on_entity_id,
                "hardpoint_id": mounted_on.hardpoint_id,
                "scanner_range_m": scanner_range.map(|r| r.0).unwrap_or(0.0),
            }),
            components,
            removed: false,
        };
        broadcast_updates.push(module_delta.clone());
        dirty_updates.push(module_delta);
    }

    if broadcast_updates.is_empty() {
        return;
    }

    let tick = runtime.last_tick.saturating_add(1);
    runtime.last_tick = tick;

    // Queue broadcast for ALL entities (clients need to see everything in range)
    let broadcast_world = WorldStateDelta {
        updates: broadcast_updates,
    };
    outbound.messages.push(QueuedReplicationDelta {
        tick,
        world: broadcast_world,
    });

    // Only ingest dirty entities for persistence
    if !dirty_updates.is_empty() {
        let dirty_world = WorldStateDelta {
            updates: dirty_updates,
        };
        let has_removals = {
            let ReplicationRuntime {
                known_entities,
                pending_updates,
                ..
            } = &mut *runtime;
            ingest_world_delta(known_entities, pending_updates, dirty_world)
        };

        if has_removals && !runtime.pending_updates.is_empty() {
            let ReplicationRuntime {
                persistence,
                pending_updates,
                ..
            } = &mut *runtime;
            if let Err(err) = flush_pending_updates(persistence, pending_updates, tick) {
                eprintln!("replication failed persisting world delta after removals: {err}");
            } else {
                runtime.last_persist_at = Instant::now();
            }
        }
    }
}

fn refresh_component_payloads_from_reflection(world: &mut World) {
    let Some(component_registry) = world.get_resource::<GeneratedComponentRegistry>().cloned()
    else {
        return;
    };
    let Some(app_type_registry) = world.get_resource::<AppTypeRegistry>().cloned() else {
        return;
    };
    let type_paths = component_type_path_map(&component_registry);

    let mut entity_by_id = HashMap::<String, Entity>::new();
    let mut ships_q = world.query::<(Entity, &SimulatedControlledEntity)>();
    for (entity, controlled) in ships_q.iter(world) {
        entity_by_id.insert(controlled.entity_id.clone(), entity);
    }
    let mut misc_q = world.query::<(
        Entity,
        &EntityGuid,
        Option<&Hardpoint>,
        Option<&MountedOn>,
        Option<&SimulatedControlledEntity>,
    )>();
    for (entity, guid, hardpoint, mounted_on, simulated) in misc_q.iter(world) {
        if simulated.is_some() {
            continue;
        }
        if hardpoint.is_some() {
            entity_by_id.insert(format!("hardpoint:{}", guid.0), entity);
        } else if mounted_on.is_some() {
            entity_by_id.insert(format!("module:{}", guid.0), entity);
        } else {
            entity_by_id.insert(format!("entity:{}", guid.0), entity);
        }
    }

    let mut target_ids = HashSet::<String>::new();
    if let Some(outbound) = world.get_resource::<ReplicationOutboundQueue>() {
        for queued in &outbound.messages {
            for update in &queued.world.updates {
                if !update.removed {
                    target_ids.insert(update.entity_id.clone());
                }
            }
        }
    }
    if let Some(runtime) = world.get_non_send_resource::<ReplicationRuntime>() {
        for (entity_id, update) in &runtime.pending_updates {
            if !update.removed {
                target_ids.insert(entity_id.clone());
            }
        }
    }

    let mut serialized_by_id = HashMap::<String, Vec<WorldComponentDelta>>::new();
    for entity_id in target_ids {
        let Some(entity) = entity_by_id.get(&entity_id).copied() else {
            continue;
        };
        let serialized = serialize_registered_components_for_entity(
            world,
            entity,
            &entity_id,
            &component_registry,
            &app_type_registry,
            &type_paths,
        );
        if !serialized.is_empty() {
            serialized_by_id.insert(entity_id, serialized);
        }
    }

    if let Some(mut outbound) = world.get_resource_mut::<ReplicationOutboundQueue>() {
        for queued in &mut outbound.messages {
            for update in &mut queued.world.updates {
                if let Some(serialized) = serialized_by_id.get(&update.entity_id) {
                    update.components = serialized.clone();
                }
            }
        }
    }

    if let Some(mut runtime) = world.get_non_send_resource_mut::<ReplicationRuntime>() {
        for (entity_id, update) in &mut runtime.pending_updates {
            if let Some(serialized) = serialized_by_id.get(entity_id) {
                update.components = serialized.clone();
            }
        }
    }
}

fn flush_replication_persistence(runtime: Option<NonSendMut<'_, ReplicationRuntime>>) {
    let Some(mut runtime) = runtime else {
        return;
    };

    let should_persist = runtime.last_persist_at.elapsed() >= runtime.persist_interval;
    if should_persist && !runtime.pending_updates.is_empty() {
        let last_tick = runtime.last_tick;
        let ReplicationRuntime {
            persistence,
            pending_updates,
            ..
        } = &mut *runtime;
        if let Err(err) = flush_pending_updates(persistence, pending_updates, last_tick) {
            eprintln!("replication failed persisting world delta: {err}");
        } else {
            runtime.last_persist_at = Instant::now();
        }
    }

    if runtime.last_snapshot_at.elapsed() >= runtime.snapshot_interval {
        let last_tick = runtime.last_tick;
        let entity_count = runtime.known_entities.len();
        if let Err(err) = runtime
            .persistence
            .persist_snapshot_marker(last_tick, entity_count)
        {
            eprintln!("replication failed persisting snapshot marker: {err}");
        } else {
            runtime.last_snapshot_at = Instant::now();
        }
    }
}

fn broadcast_replication_state(
    mut outbound: ResMut<'_, ReplicationOutboundQueue>,
    server_query: Query<'_, '_, &Server, With<RawServer>>,
    clients: Query<'_, '_, (Entity, &RemoteId), ConnectedClientFilter>,
    visibility_registry: Res<'_, ClientVisibilityRegistry>,
    position_map: Res<'_, ClientControlledEntityPositionMap>,
    mut visibility_history: ResMut<'_, ClientVisibilityHistory>,
    mut sender: ServerMultiMessageSender<'_, '_, With<Connected>>,
) {
    if outbound.messages.is_empty() {
        return;
    }
    let Ok(server) = server_query.single() else {
        return;
    };

    let live_clients = clients
        .iter()
        .map(|(entity, _)| entity)
        .collect::<HashSet<_>>();
    visibility_history
        .visible_entities_by_client
        .retain(|client, _| live_clients.contains(client));

    for queued in outbound.messages.drain(..) {
        for (client_entity, remote_id) in &clients {
            let visibility_ctx =
                visibility_context_for_client(client_entity, &visibility_registry, &position_map);
            let Some(mut filtered_world) = apply_visibility_filter(&queued.world, &visibility_ctx)
            else {
                visibility_history
                    .visible_entities_by_client
                    .remove(&client_entity);
                continue;
            };

            let current_visible = filtered_world
                .updates
                .iter()
                .filter(|update| !update.removed)
                .map(|update| update.entity_id.clone())
                .collect::<HashSet<_>>();
            let previous_visible = visibility_history
                .visible_entities_by_client
                .get(&client_entity)
                .cloned()
                .unwrap_or_default();

            for disappeared in previous_visible.difference(&current_visible) {
                filtered_world.updates.push(WorldDeltaEntity {
                    entity_id: disappeared.clone(),
                    labels: Vec::new(),
                    properties: serde_json::json!({}),
                    components: Vec::new(),
                    removed: true,
                });
            }

            visibility_history
                .visible_entities_by_client
                .insert(client_entity, current_visible);

            let target = delivery_target_for_session(&visibility_ctx, remote_id.0);
            let message = match ReplicationStateMessage::from_world(queued.tick, &filtered_world) {
                Ok(message) => message,
                Err(err) => {
                    eprintln!(
                        "replication failed encoding outbound replication state tick={} for Lightyear: {err}",
                        queued.tick
                    );
                    continue;
                }
            };
            if let Err(err) =
                sender.send::<ReplicationStateMessage, StateChannel>(&message, server, &target)
            {
                eprintln!("replication failed broadcasting state message: {err}");
            }
        }
    }
}

fn log_replication_client_connected(
    trigger: On<Add, Connected>,
    clients: Query<'_, '_, (), With<ClientOf>>,
) {
    if clients.get(trigger.entity).is_ok() {
        println!(
            "replication lightyear client connected entity={:?}",
            trigger.entity
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sidereal_net::{WorldComponentDelta, WorldStateDelta};
    use std::net::{IpAddr, Ipv4Addr};

    #[test]
    fn remote_endpoint_registers_when_enabled() {
        let cfg = RemoteInspectConfig {
            enabled: true,
            bind_addr: IpAddr::V4(Ipv4Addr::LOCALHOST),
            port: 15713,
            auth_token: Some("0123456789abcdef".to_string()),
        };
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        configure_remote(&mut app, &cfg);

        assert!(
            app.world()
                .contains_resource::<bevy_remote::http::HostPort>()
        );
        assert!(app.world().contains_resource::<BrpAuthToken>());
    }

    #[test]
    fn ingest_world_delta_tracks_add_remove() {
        let mut cache = HashSet::<String>::new();
        let mut pending = HashMap::<String, WorldDeltaEntity>::new();
        let add = WorldDeltaEntity {
            entity_id: "ship:1".to_string(),
            labels: vec!["Entity".to_string()],
            properties: serde_json::json!({}),
            components: Vec::new(),
            removed: false,
        };
        let has_removals = ingest_world_delta(
            &mut cache,
            &mut pending,
            WorldStateDelta { updates: vec![add] },
        );
        assert!(!has_removals);
        assert!(cache.contains("ship:1"));
        assert!(pending.contains_key("ship:1"));

        let remove = WorldDeltaEntity {
            entity_id: "ship:1".to_string(),
            labels: Vec::new(),
            properties: serde_json::json!({}),
            components: Vec::new(),
            removed: true,
        };
        let has_removals = ingest_world_delta(
            &mut cache,
            &mut pending,
            WorldStateDelta {
                updates: vec![remove],
            },
        );
        assert!(has_removals);
        assert!(!cache.contains("ship:1"));
        assert!(pending.contains_key("ship:1"));
    }

    #[test]
    fn visibility_context_uses_registered_client_player_mapping() {
        let client = Entity::from_bits(42);
        let mut registry = ClientVisibilityRegistry::default();
        registry.register_client(client, "player:abc".to_string());
        let positions = ClientControlledEntityPositionMap::default();

        let auth = visibility_context_for_client(client, &registry, &positions);
        assert_eq!(auth.scope, visibility::VisibilityScope::Authenticated);
        assert_eq!(auth.player_entity_id.as_deref(), Some("player:abc"));

        let unknown = visibility_context_for_client(Entity::from_bits(7), &registry, &positions);
        assert_eq!(unknown.scope, visibility::VisibilityScope::None);
        assert!(unknown.player_entity_id.is_none());
    }

    #[test]
    fn visibility_filter_enforces_ownership() {
        let world = WorldStateDelta {
            updates: vec![
                WorldDeltaEntity {
                    entity_id: "ship:1".to_string(),
                    labels: vec!["Entity".to_string()],
                    properties: serde_json::json!({
                        "entity_id": "ship:1",
                        "position_m": [100.0, 200.0, 0.0],
                        "health": 1000.0,
                    }),
                    components: vec![WorldComponentDelta {
                        component_id: "ship:1:owner_id".to_string(),
                        component_kind: "owner_id".to_string(),
                        properties: serde_json::json!("player:alice"),
                    }],
                    removed: false,
                },
                WorldDeltaEntity {
                    entity_id: "ship:2".to_string(),
                    labels: vec!["Entity".to_string()],
                    properties: serde_json::json!({
                        "entity_id": "ship:2",
                        "position_m": [110.0, 200.0, 0.0],
                        "health": 800.0,
                    }),
                    components: vec![WorldComponentDelta {
                        component_id: "ship:2:owner_id".to_string(),
                        component_kind: "owner_id".to_string(),
                        properties: serde_json::json!("player:bob"),
                    }],
                    removed: false,
                },
            ],
        };

        let ctx = visibility::VisibilityContext::authenticated(
            "player:alice".to_string(),
            Some(Vec3::new(100.0, 200.0, 0.0)),
        );
        let filtered = apply_visibility_filter(&world, &ctx).unwrap();

        let own_ship = filtered
            .updates
            .iter()
            .find(|e| e.entity_id == "ship:1")
            .unwrap();
        assert!(own_ship.properties.get("health").is_some());
        assert_eq!(own_ship.components.len(), 1);

        let other_ship = filtered
            .updates
            .iter()
            .find(|e| e.entity_id == "ship:2")
            .unwrap();
        assert!(other_ship.properties.get("position_m").is_some());
        assert!(other_ship.properties.get("health").is_none());
        assert_eq!(other_ship.components.len(), 0);
    }
}
