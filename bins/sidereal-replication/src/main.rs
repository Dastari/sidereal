mod bootstrap_runtime;
mod replication;
mod visibility;

use crate::replication::assets::{
    receive_client_asset_acks, receive_client_asset_requests,
    send_asset_stream_chunks_paced, stream_bootstrap_assets_to_authenticated_clients,
};
use crate::replication::auth::{cleanup_client_auth_bindings, receive_client_auth_messages};
use crate::replication::hydration_parse::{
    base_mass_from_record, cargo_mass_from_record, engine_from_record, faction_id_from_record,
    flight_computer_from_record, flight_tuning_from_record, fuel_tank_from_record,
    hardpoint_from_record, has_marker_component_record, health_pool_from_record,
    inventory_from_record, mass_kg_from_record, max_velocity_from_record, module_mass_from_record,
    mounted_on_from_record, owner_id_from_record, scanner_component_from_record,
    scanner_range_buff_from_record, scanner_range_from_record, size_m_from_record,
    total_mass_from_record,
};
use crate::replication::input::{
    ClientInputDropMetrics, ClientInputDropMetricsLogState, ClientInputTickTracker,
    drain_native_player_inputs_to_action_queue, report_input_drop_metrics,
};
use crate::replication::lifecycle::{
    configure_remote, hydrate_replication_world, init_replication_runtime,
    log_replication_client_connected, setup_client_replication_sender, start_lightyear_server,
};
use crate::replication::persistence::{
    SimulationPersistenceTimer, flush_player_runtime_view_state_persistence,
    flush_simulation_state_persistence,
};
use crate::replication::physics_runtime::{
    enforce_planar_ship_motion, sync_simulated_ship_components,
};
use crate::replication::runtime_state::{
    compute_controlled_entity_scanner_ranges, update_client_controlled_entity_positions,
};
use crate::replication::transport::ensure_server_transport_channels;
use crate::replication::view::receive_client_view_updates;
use crate::replication::visibility::update_network_visibility;
use avian3d::prelude::*;
use bevy::asset::{AssetApp, AssetPlugin};
use bevy::ecs::reflect::AppTypeRegistry;
use bevy::log::LogPlugin;
use bevy::log::{info, warn};
use bevy::prelude::*;
use bevy::scene::ScenePlugin;
use bootstrap_runtime::BootstrapShipReceiver;
use lightyear::prelude::server::RawServer;
use lightyear::prelude::server::ServerPlugins;
use lightyear::prelude::{
    ControlledBy, Lifetime, NetworkTarget, Replicate, ReplicationBufferSystems,
};
use sidereal_asset_runtime::default_asset_dependencies;
use sidereal_core::remote_inspect::RemoteInspectConfig;
use sidereal_game::{
    ActionQueue, BaseMassKg, CargoMassKg, Engine, EntityGuid, FactionVisibility, FuelTank,
    GeneratedComponentRegistry, Inventory, MassDirty, MassKg, ModuleMassKg, MountedOn, OwnerId,
    PublicVisibility, ScannerRangeM, SiderealGamePlugin, TotalMassKg, angular_inertia_from_size,
    default_corvette_asset_id, default_corvette_flight_computer, default_corvette_flight_tuning,
    default_corvette_health_pool, default_corvette_mass_kg, default_corvette_max_velocity_mps,
    default_corvette_size, default_flight_action_capabilities,
    default_space_background_shader_asset_id, default_starfield_shader_asset_id,
};
use sidereal_net::register_lightyear_protocol;
use sidereal_persistence::{GraphPersistence, PlayerRuntimeViewState};
#[cfg(test)]
use sidereal_replication::state::{GraphDeltaBatch, ingest_graph_batch};
use sidereal_runtime_sync::{
    component_record, component_type_path_map, insert_registered_components_from_graph_records,
    parse_guid_from_entity_id, parse_vec3_value,
};
use std::collections::{HashMap, HashSet};
use std::time::Duration;
use visibility::{ClientControlledEntityPositionMap, ClientVisibilityRegistry};

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

struct ReplicationRuntime {
    persistence: sidereal_persistence::GraphPersistence,
}

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

/// Chunk queued for paced sending to avoid UDP send-buffer overflow (EAGAIN).
pub(crate) struct PendingAssetChunk {
    pub(crate) asset_id: String,
    pub(crate) relative_cache_path: String,
    pub(crate) chunk_index: u32,
    pub(crate) chunk_count: u32,
    pub(crate) bytes: Vec<u8>,
}

#[derive(Resource, Default)]
struct AssetStreamServerState {
    sent_asset_ids_by_remote: HashMap<lightyear::prelude::PeerId, HashSet<String>>,
    pending_requested_asset_ids_by_remote: HashMap<lightyear::prelude::PeerId, HashSet<String>>,
    acked_assets_by_remote: HashMap<lightyear::prelude::PeerId, HashMap<String, u64>>,
    /// Chunks to send per remote; drained at a fixed rate per frame to avoid EAGAIN.
    pub(crate) pending_chunks_by_remote:
        HashMap<lightyear::prelude::PeerId, std::collections::VecDeque<PendingAssetChunk>>,
}

#[derive(Resource, Default)]
struct AssetDependencyMap {
    dependencies_by_asset_id: HashMap<String, Vec<String>>,
}

/// Deferred (client_entity, ship_entity) bindings so ControlledBy is applied in PostUpdate,
/// avoiding same-frame entity/hierarchy ordering issues during replication.
#[derive(Resource, Default)]
pub(crate) struct PendingControlledByBindings {
    pub(crate) bindings: Vec<(Entity, Entity)>,
}

#[derive(Resource, Default)]
struct PlayerRuntimeViewRegistry {
    by_player_entity_id: HashMap<String, PlayerRuntimeViewState>,
}

#[derive(Resource, Default)]
struct PlayerRuntimeViewDirtySet {
    player_entity_ids: HashSet<String>,
}

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
    app.add_plugins(
        PhysicsPlugins::default()
            .with_length_unit(1.0)
            .build()
            .disable::<PhysicsTransformPlugin>()
            .disable::<PhysicsInterpolationPlugin>(),
    );
    app.add_message::<bevy::asset::AssetEvent<Mesh>>();
    app.init_asset::<Mesh>();
    app.insert_resource(Gravity(Vec3::ZERO));
    app.add_plugins(ServerPlugins {
        tick_duration: Duration::from_secs_f64(1.0 / 30.0),
    });
    register_lightyear_protocol(&mut app);
    configure_remote(&mut app, &remote_cfg);
    info!("replication running native world-sync runtime path");
    // Lightyear/Bevy plugins can initialize Fixed time; enforce authoritative 30 Hz after plugin wiring.
    app.insert_resource(Time::<Fixed>::from_hz(30.0));
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
    app.add_systems(
        Startup,
        bootstrap_runtime::start_replication_control_listener,
    );
    app.add_observer(log_replication_client_connected);
    app.add_observer(setup_client_replication_sender);
    app.insert_resource(ClientVisibilityRegistry::default());
    app.insert_resource(ClientControlledEntityPositionMap::default());
    app.insert_resource(PlayerControlledEntityMap::default());
    app.insert_resource(AuthenticatedClientBindings::default());
    app.insert_resource(AssetStreamServerState::default());
    app.insert_resource(PlayerRuntimeViewRegistry::default());
    app.insert_resource(PlayerRuntimeViewDirtySet::default());
    app.insert_resource(ClientInputTickTracker::default());
    app.insert_resource(ClientInputDropMetrics::default());
    app.insert_resource(ClientInputDropMetricsLogState::default());
    app.insert_resource(AssetDependencyMap {
        dependencies_by_asset_id: default_asset_dependencies(),
    });
    app.insert_resource(SimulationPersistenceTimer::default());
    app.insert_resource(PendingControlledByBindings::default());
    app.add_systems(
        Update,
        (
            ensure_server_transport_channels,
            cleanup_client_auth_bindings,
            receive_client_auth_messages,
            receive_client_view_updates,
            receive_client_asset_requests,
            receive_client_asset_acks,
            stream_bootstrap_assets_to_authenticated_clients,
            send_asset_stream_chunks_paced.after(stream_bootstrap_assets_to_authenticated_clients),
            report_input_drop_metrics,
            process_bootstrap_ship_commands,
        )
            .chain(),
    );
    app.add_systems(
        FixedUpdate,
        (
            sync_simulated_ship_components,
            update_client_controlled_entity_positions,
            compute_controlled_entity_scanner_ranges,
            update_network_visibility,
            flush_player_runtime_view_state_persistence,
        )
            .chain()
            .after(PhysicsSystems::Writeback),
    );
    app.add_systems(
        FixedUpdate,
        flush_simulation_state_persistence.after(flush_player_runtime_view_state_persistence),
    );
    app.add_systems(
        FixedUpdate,
        enforce_planar_ship_motion.before(PhysicsSystems::Prepare),
    );
    app.add_systems(
        FixedUpdate,
        drain_native_player_inputs_to_action_queue.before(PhysicsSystems::Prepare),
    );
    app.add_systems(
        PostUpdate,
        apply_pending_controlled_by_bindings.after(ReplicationBufferSystems::AfterBuffer),
    );
    app.run();
}

fn spawn_simulation_entity(
    commands: &mut Commands<'_, '_>,
    controlled_entity_map: &mut PlayerControlledEntityMap,
    entity_id: &str,
    player_entity_id: &str,
    mut pos: Vec3,
    mut vel: Vec3,
) {
    pos.z = 0.0;
    vel.z = 0.0;
    let ship_guid = parse_guid_from_entity_id(entity_id).unwrap_or_else(uuid::Uuid::new_v4);

    let hull_mass = default_corvette_mass_kg();
    let hull_size = default_corvette_size();
    let mut entity_commands = commands.spawn((
        Name::new(entity_id.to_string()),
        SimulatedControlledEntity {
            entity_id: entity_id.to_string(),
            player_entity_id: player_entity_id.to_string(),
        },
        EntityGuid(ship_guid),
        OwnerId(player_entity_id.to_string()),
        ActionQueue::default(),
        default_flight_action_capabilities(),
        default_corvette_flight_computer(),
        default_corvette_flight_tuning(),
        default_corvette_max_velocity_mps(),
        default_corvette_health_pool(),
        hull_size,
        Transform::from_translation(pos),
    ));
    entity_commands.insert(Replicate::to_clients(NetworkTarget::All));
    let entity = entity_commands
        .insert((
            MassKg(hull_mass),
            BaseMassKg(hull_mass),
            CargoMassKg(0.0),
            ModuleMassKg(0.0),
            TotalMassKg(hull_mass),
            MassDirty,
            Inventory::default(),
        ))
        .insert((
            RigidBody::Dynamic,
            Collider::cuboid(
                hull_size.width * 0.5,
                hull_size.length * 0.5,
                hull_size.height * 0.5,
            ),
            Mass(hull_mass),
            angular_inertia_from_size(hull_mass, &hull_size),
            Position(pos),
            Rotation::default(),
            LinearVelocity(vel),
            AngularVelocity::default(),
            LockedAxes::new()
                .lock_translation_z()
                .lock_rotation_x()
                .lock_rotation_y(),
            LinearDamping(0.0),
            AngularDamping(0.0),
        ))
        .id();
    controlled_entity_map
        .by_player_entity_id
        .insert(player_entity_id.to_string(), entity);

    // Flight computer module
    let fc_guid = uuid::Uuid::new_v4();
    let mut fc_commands = commands.spawn((
        Name::new(format!("{}:flight_computer", entity_id)),
        EntityGuid(fc_guid),
        default_corvette_flight_computer(),
        MountedOn {
            parent_entity_id: ship_guid,
            hardpoint_id: "computer_core".to_string(),
        },
        MassKg(50.0),
        OwnerId(player_entity_id.to_string()),
    ));
    fc_commands.insert(Replicate::to_clients(NetworkTarget::All));

    // Left engine + fuel tank
    let engine_left_guid = uuid::Uuid::new_v4();
    let mut engine_left_commands = commands.spawn((
        Name::new(format!("{}:engine_left", entity_id)),
        EntityGuid(engine_left_guid),
        MountedOn {
            parent_entity_id: ship_guid,
            hardpoint_id: "engine_left_aft".to_string(),
        },
        Engine {
            thrust: 1_200_000.0,
            reverse_thrust: 600_000.0,
            torque_thrust: 3_000_000.0,
            burn_rate_kg_s: 0.8,
        },
        FuelTank { fuel_kg: 1000.0 },
        MassKg(500.0),
        OwnerId(player_entity_id.to_string()),
    ));
    engine_left_commands.insert(Replicate::to_clients(NetworkTarget::All));

    // Right engine + fuel tank
    let engine_right_guid = uuid::Uuid::new_v4();
    let mut engine_right_commands = commands.spawn((
        Name::new(format!("{}:engine_right", entity_id)),
        EntityGuid(engine_right_guid),
        MountedOn {
            parent_entity_id: ship_guid,
            hardpoint_id: "engine_right_aft".to_string(),
        },
        Engine {
            thrust: 1_200_000.0,
            reverse_thrust: 600_000.0,
            torque_thrust: 3_000_000.0,
            burn_rate_kg_s: 0.8,
        },
        FuelTank { fuel_kg: 1000.0 },
        MassKg(500.0),
        OwnerId(player_entity_id.to_string()),
    ));
    engine_right_commands.insert(Replicate::to_clients(NetworkTarget::All));
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

        let mut pos = record
            .properties
            .get("position_m")
            .and_then(parse_vec3_value)
            .unwrap_or(Vec3::ZERO);
        let mut vel = record
            .properties
            .get("velocity_mps")
            .and_then(parse_vec3_value)
            .unwrap_or(Vec3::ZERO);
        pos.z = 0.0;
        vel.z = 0.0;
        let heading_rad = record
            .properties
            .get("heading_rad")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0) as f32;
        let health_pool = health_pool_from_record(record, &type_paths)
            .unwrap_or_else(default_corvette_health_pool);
        let flight_computer = flight_computer_from_record(record, &type_paths)
            .unwrap_or_else(default_corvette_flight_computer);
        let flight_tuning = flight_tuning_from_record(record, &type_paths)
            .unwrap_or_else(default_corvette_flight_tuning);
        let max_velocity_mps = max_velocity_from_record(record, &type_paths)
            .unwrap_or_else(default_corvette_max_velocity_mps);
        let scanner_range =
            scanner_range_from_record(record, &type_paths).unwrap_or(ScannerRangeM(0.0));
        let scanner_component = scanner_component_from_record(record, &type_paths);
        let scanner_buff = scanner_range_buff_from_record(record, &type_paths);
        let faction_id = faction_id_from_record(record, &type_paths);
        let faction_visibility =
            has_marker_component_record(record, "faction_visibility").then_some(FactionVisibility);
        let public_visibility =
            has_marker_component_record(record, "public_visibility").then_some(PublicVisibility);
        let mass_kg =
            mass_kg_from_record(record, &type_paths).unwrap_or(MassKg(default_corvette_mass_kg()));
        let base_mass = base_mass_from_record(record, &type_paths)
            .unwrap_or(BaseMassKg(default_corvette_mass_kg()));
        let cargo_mass = cargo_mass_from_record(record, &type_paths).unwrap_or(CargoMassKg(0.0));
        let module_mass = module_mass_from_record(record, &type_paths).unwrap_or(ModuleMassKg(0.0));
        let total_mass =
            total_mass_from_record(record, &type_paths).unwrap_or(TotalMassKg(base_mass.0));
        let inventory = inventory_from_record(record, &type_paths).unwrap_or_default();

        let hull_size =
            size_m_from_record(record, &type_paths).unwrap_or_else(default_corvette_size);
        let hull_mass_for_physics = total_mass.0.max(1.0);
        let collider_half_extents = Vec3::new(
            hull_size.width * 0.5,
            hull_size.length * 0.5,
            hull_size.height * 0.5,
        )
        .max(Vec3::splat(0.1));
        let mut entity_commands = commands.spawn((
            Name::new(record.entity_id.clone()),
            SimulatedControlledEntity {
                entity_id: record.entity_id.clone(),
                player_entity_id: player_entity_id.clone(),
            },
            EntityGuid(ship_guid),
            OwnerId(player_entity_id.clone()),
            ActionQueue::default(),
            default_flight_action_capabilities(),
            flight_computer,
            flight_tuning,
            max_velocity_mps,
            health_pool,
            scanner_range,
            Transform::from_translation(pos).with_rotation(Quat::from_rotation_z(heading_rad)),
        ));
        entity_commands.insert(Replicate::to_clients(NetworkTarget::All));
        entity_commands.insert(hull_size);
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
        if let Some(faction_id) = faction_id {
            entity_commands.insert(faction_id);
        }
        if let Some(faction_visibility) = faction_visibility {
            entity_commands.insert(faction_visibility);
        }
        if let Some(public_visibility) = public_visibility {
            entity_commands.insert(public_visibility);
        }
        let entity = entity_commands
            .insert((
                RigidBody::Dynamic,
                Collider::cuboid(
                    collider_half_extents.x,
                    collider_half_extents.y,
                    collider_half_extents.z,
                ),
                Mass(hull_mass_for_physics),
                angular_inertia_from_size(hull_mass_for_physics, &hull_size),
                Position(pos),
                Rotation(Quat::from_rotation_z(heading_rad)),
                LinearVelocity(vel),
                AngularVelocity::default(),
                LockedAxes::new()
                    .lock_translation_z()
                    .lock_rotation_x()
                    .lock_rotation_y(),
                LinearDamping(0.0),
                AngularDamping(0.0),
            ))
            .id();
        insert_registered_components_from_graph_records(
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

    if hydrated_ships > 0 && hydrated_modules == 0 {
        warn!(
            "replication hydration restored ships without modules; keeping authoritative no-module state"
        );
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
        entity_commands.insert(Replicate::to_clients(NetworkTarget::All));
        if let Some(owner) = owner_id_from_record(record, &type_paths) {
            entity_commands.insert(owner);
        }
        if let Some(faction_id) = faction_id_from_record(record, &type_paths) {
            entity_commands.insert(faction_id);
        }
        if has_marker_component_record(record, "faction_visibility") {
            entity_commands.insert(FactionVisibility);
        }
        if has_marker_component_record(record, "public_visibility") {
            entity_commands.insert(PublicVisibility);
        }
        if let Some(mass_kg) = mass_kg_from_record(record, &type_paths) {
            entity_commands.insert(mass_kg);
        }
        if let Some(inventory) = inventory_from_record(record, &type_paths) {
            entity_commands.insert(inventory);
        }
        let hardpoint_entity = entity_commands.id();
        insert_registered_components_from_graph_records(
            &mut commands,
            hardpoint_entity,
            &record.components,
            &type_paths,
            &app_type_registry,
        );
        spawned_entity_by_entity_id.insert(record.entity_id.clone(), hardpoint_entity);
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
        entity_commands.insert(Replicate::to_clients(NetworkTarget::All));
        if let Some(owner) = owner_id_from_record(record, &type_paths) {
            entity_commands.insert(owner);
        }
        if let Some(faction_id) = faction_id_from_record(record, &type_paths) {
            entity_commands.insert(faction_id);
        }
        if has_marker_component_record(record, "faction_visibility") {
            entity_commands.insert(FactionVisibility);
        }
        if has_marker_component_record(record, "public_visibility") {
            entity_commands.insert(PublicVisibility);
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
        insert_registered_components_from_graph_records(
            &mut commands,
            module_entity,
            &record.components,
            &type_paths,
            &app_type_registry,
        );
        spawned_entity_by_entity_id.insert(record.entity_id.clone(), module_entity);
        hydrated_modules = hydrated_modules.saturating_add(1);
    }

    // Do not use set_parent_in_place: Bevy Parent/ChildOf would be replicated by Lightyear's
    // HierarchySendPlugin and can cause "Entity not yet spawned: PLACEHOLDER" on the client when
    // a child is applied before its parent. Logical hierarchy is expressed via MountedOn only.

    println!(
        "replication simulation hydrated {hydrated_ships} entities, {hydrated_hardpoints} hardpoints and {hydrated_modules} modules"
    );
}

fn process_bootstrap_ship_commands(
    mut commands: Commands<'_, '_>,
    mut controlled_entity_map: ResMut<'_, PlayerControlledEntityMap>,
    mut pending_controlled_by: ResMut<'_, PendingControlledByBindings>,
    bindings: Res<'_, AuthenticatedClientBindings>,
    receiver: Option<Res<'_, BootstrapShipReceiver>>,
) {
    let Some(receiver) = receiver else { return };
    for cmd in bootstrap_runtime::drain_bootstrap_ship_commands(receiver.as_ref()) {
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
            bootstrap_runtime::starter_spawn_position(cmd.account_id),
            Vec3::ZERO,
        );
        // Defer ControlledBy to PostUpdate so replication send sees the ship before we add the binding.
        if let Some(&ship_entity) = controlled_entity_map
            .by_player_entity_id
            .get(&cmd.player_entity_id)
        {
            let client_entity = bindings
                .by_client_entity
                .iter()
                .find(|(_, player_id)| *player_id == &cmd.player_entity_id)
                .map(|(entity, _)| *entity);
            if let Some(client_entity) = client_entity {
                pending_controlled_by.bindings.push((client_entity, ship_entity));
            }
        }
    }
}

fn apply_pending_controlled_by_bindings(
    mut commands: Commands<'_, '_>,
    mut pending: ResMut<'_, PendingControlledByBindings>,
) {
    for (client_entity, ship_entity) in pending.bindings.drain(..) {
        commands.entity(ship_entity).insert(ControlledBy {
            owner: client_entity,
            lifetime: Lifetime::SessionBased,
        });
    }
}

fn unix_epoch_now_i64() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|v| v.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sidereal_persistence::GraphEntityRecord;
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
    fn ingest_graph_batch_tracks_add_remove() {
        let mut cache = HashSet::<String>::new();
        let mut pending = HashMap::<String, GraphEntityRecord>::new();
        let mut removals = HashSet::<String>::new();
        let add = GraphEntityRecord {
            entity_id: "ship:1".to_string(),
            labels: vec!["Entity".to_string()],
            properties: serde_json::json!({}),
            components: Vec::new(),
        };
        let has_removals = ingest_graph_batch(
            &mut cache,
            &mut pending,
            &mut removals,
            GraphDeltaBatch {
                upserts: vec![add],
                removals: Vec::new(),
            },
        );
        assert!(!has_removals);
        assert!(cache.contains("ship:1"));
        assert!(pending.contains_key("ship:1"));
        assert!(removals.is_empty());

        let has_removals = ingest_graph_batch(
            &mut cache,
            &mut pending,
            &mut removals,
            GraphDeltaBatch {
                upserts: Vec::new(),
                removals: vec!["ship:1".to_string()],
            },
        );
        assert!(has_removals);
        assert!(!cache.contains("ship:1"));
        assert!(!pending.contains_key("ship:1"));
        assert!(removals.contains("ship:1"));
    }
}
