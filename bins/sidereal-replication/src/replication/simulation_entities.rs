//! Simulation entity spawn, hydration, and controlled-entity binding.
//! Keeps generic entity terminology; domain-specific labels (e.g. graph "Ship") stay at persistence boundary.

use avian3d::prelude::*;
use bevy::ecs::reflect::AppTypeRegistry;
use bevy::prelude::*;
use lightyear::prelude::{ControlledBy, Lifetime, NetworkTarget, Replicate};
use sidereal_game::{
    ActionQueue, BaseMassKg, CargoMassKg, Engine, EntityGuid, FactionVisibility, FuelTank,
    GeneratedComponentRegistry, Inventory, MassDirty, MassKg, ModuleMassKg, MountedOn, OwnerId,
    PublicVisibility, ScannerRangeM, TotalMassKg, angular_inertia_from_size,
    default_corvette_flight_computer, default_corvette_flight_tuning, default_corvette_health_pool,
    default_corvette_mass_kg, default_corvette_max_velocity_mps, default_corvette_size,
    default_flight_action_capabilities,
};
use sidereal_persistence::GraphPersistence;
use sidereal_runtime_sync::{
    component_record, component_type_path_map, insert_registered_components_from_graph_records,
    parse_guid_from_entity_id, parse_vec3_value,
};
use std::collections::HashMap;

use super::hydration_parse::{
    base_mass_from_record, cargo_mass_from_record, engine_from_record, faction_id_from_record,
    flight_computer_from_record, flight_tuning_from_record, fuel_tank_from_record,
    hardpoint_from_record, has_marker_component_record, health_pool_from_record,
    inventory_from_record, mass_kg_from_record, max_velocity_from_record, module_mass_from_record,
    mounted_on_from_record, owner_id_from_record, scanner_component_from_record,
    scanner_range_buff_from_record, scanner_range_from_record, size_m_from_record,
    total_mass_from_record,
};
use crate::AuthenticatedClientBindings;
use crate::bootstrap_runtime::{self, BootstrapShipReceiver};

#[derive(Resource, Default)]
pub struct PlayerControlledEntityMap {
    pub by_player_entity_id: HashMap<String, Entity>,
}

#[derive(Debug, Component)]
pub struct SimulatedControlledEntity {
    pub entity_id: String,
    pub player_entity_id: String,
}

/// Deferred (client_entity, controlled_entity) bindings so ControlledBy is applied in PostUpdate,
/// avoiding same-frame entity/hierarchy ordering issues during replication.
#[derive(Resource, Default)]
pub struct PendingControlledByBindings {
    pub bindings: Vec<(Entity, Entity)>,
}

pub fn spawn_simulation_entity(
    commands: &mut Commands<'_, '_>,
    controlled_entity_map: &mut PlayerControlledEntityMap,
    entity_id: &str,
    player_entity_id: &str,
    mut pos: Vec3,
    mut vel: Vec3,
) {
    pos.z = 0.0;
    vel.z = 0.0;
    let hull_guid = parse_guid_from_entity_id(entity_id).unwrap_or_else(uuid::Uuid::new_v4);

    let hull_mass = default_corvette_mass_kg();
    let hull_size = default_corvette_size();
    let mut entity_commands = commands.spawn((
        Name::new(entity_id.to_string()),
        SimulatedControlledEntity {
            entity_id: entity_id.to_string(),
            player_entity_id: player_entity_id.to_string(),
        },
        EntityGuid(hull_guid),
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
            parent_entity_id: hull_guid,
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
            parent_entity_id: hull_guid,
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
            parent_entity_id: hull_guid,
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

pub fn hydrate_simulation_entities(
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
    let mut hull_guid_by_entity_id = HashMap::<String, uuid::Uuid>::new();
    let mut spawned_entity_by_entity_id = HashMap::<String, Entity>::new();
    let mut hull_records = Vec::new();
    let mut hardpoint_records = Vec::new();
    let mut module_records = Vec::new();

    for record in records {
        if record.labels.iter().any(|label| label == "Ship") {
            hull_records.push(record);
        } else if record.labels.iter().any(|label| label == "Hardpoint")
            || component_record(&record.components, "hardpoint").is_some()
        {
            hardpoint_records.push(record);
        } else if component_record(&record.components, "mounted_on").is_some() {
            module_records.push(record);
        }
    }

    let mut hydrated_hulls = 0usize;
    let mut hydrated_hardpoints = 0usize;
    let mut hydrated_modules = 0usize;

    // Pass 1: hull entities first so module relationships can resolve parent GUIDs.
    for record in &hull_records {
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

        let hull_guid =
            parse_guid_from_entity_id(&record.entity_id).unwrap_or_else(uuid::Uuid::new_v4);
        hull_guid_by_entity_id.insert(record.entity_id.clone(), hull_guid);

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
            EntityGuid(hull_guid),
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
        hydrated_hulls = hydrated_hulls.saturating_add(1);
    }

    if hydrated_hulls > 0 && hydrated_modules == 0 {
        bevy::log::warn!(
            "replication hydration restored hulls without modules; keeping authoritative no-module state"
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

    // Pass 3: module entities after parent hull GUIDs are indexed.
    for record in &module_records {
        let Some(mounted_on) = mounted_on_from_record(record, &type_paths) else {
            continue;
        };
        let parent_entity_id = format!("ship:{}", mounted_on.parent_entity_id);
        if !hull_guid_by_entity_id.contains_key(&parent_entity_id) {
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

    println!(
        "replication simulation hydrated {hydrated_hulls} entities, {hydrated_hardpoints} hardpoints and {hydrated_modules} modules"
    );
}

pub fn process_bootstrap_entity_commands(
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
            "spawning bootstrapped controlled entity {} for {}",
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
        if let Some(&controlled_entity) = controlled_entity_map
            .by_player_entity_id
            .get(&cmd.player_entity_id)
        {
            let client_entity = bindings
                .by_client_entity
                .iter()
                .find(|(_, player_id)| *player_id == &cmd.player_entity_id)
                .map(|(entity, _)| *entity);
            if let Some(client_entity) = client_entity {
                pending_controlled_by
                    .bindings
                    .push((client_entity, controlled_entity));
            }
        }
    }
}

pub fn apply_pending_controlled_by_bindings(
    mut commands: Commands<'_, '_>,
    mut pending: ResMut<'_, PendingControlledByBindings>,
) {
    for (client_entity, controlled_entity) in pending.bindings.drain(..) {
        commands.entity(controlled_entity).insert(ControlledBy {
            owner: client_entity,
            lifetime: Lifetime::SessionBased,
        });
    }
}
