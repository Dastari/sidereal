//! Simulation entity hydration and controlled-entity binding (replication server only).
//!
//! Single dependency-ordered pass: for each graph record, spawn one entity with identity
//! (Name, EntityGuid), run the generic component insert. Hierarchy is tracked via
//! `MountedOn` (UUID-based), NOT Bevy `ChildOf`/`Children`, because Lightyear cannot
//! safely replicate Bevy hierarchy (client-side entity mapping order is undefined).
//! Control binding is derived from OwnerId + PlayerTag after the generic pass.

use avian2d::prelude::{AngularVelocity, LinearVelocity, Position, RigidBody, Rotation};
use bevy::ecs::reflect::AppTypeRegistry;
use bevy::log::error;
use bevy::{math::DVec2, prelude::*};
use lightyear::prelude::{InterpolationTarget, NetworkTarget, Replicate};
use sidereal_game::{
    ControlledEntityGuid, DisplayName, EntityGuid, GeneratedComponentRegistry, OwnerId,
    WorldPosition, WorldRotation,
};
use sidereal_net::PlayerEntityId;
use sidereal_persistence::{GraphEntityRecord, GraphPersistence};
use sidereal_runtime_sync::{
    component_record, component_type_path_map, decode_graph_component_payload,
    insert_registered_components_from_graph_records, parse_guid_from_entity_id,
};
use std::collections::{HashMap, HashSet};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::bootstrap_runtime::BootstrapEntityCommandPayload;
use crate::bootstrap_runtime::{self, BootstrapEntityReceiver};
use crate::replication::lifecycle::{HydratedEntityCount, HydratedGraphEntity};
use crate::replication::persistence::PersistenceSchemaInitState;
use crate::replication::scripting::{
    AssetRegistryResource, EntityRegistryResource, ScriptCatalogResource,
    emit_bundle_spawned_event_from_catalog, load_world_init_graph_records_from_catalog,
    scripts_root_dir, spawn_bundle_graph_records_cached,
};

const ADMIN_ALLOWED_OVERRIDE_KEYS: &[&str] = &["display_name", "owner_id"];
const ADMIN_MAX_OVERRIDE_FIELDS: usize = 8;
const ADMIN_MAX_OVERRIDE_JSON_BYTES: usize = 2048;

const WORLD_INIT_STATE_KEY: &str = "world/world_init.lua:phase2";
const STARTUP_PERSISTENCE_RETRY_ATTEMPTS: usize = 20;
const STARTUP_PERSISTENCE_RETRY_DELAY: Duration = Duration::from_millis(500);

#[derive(Resource, Default)]
pub struct PlayerControlledEntityMap {
    pub by_player_entity_id: HashMap<PlayerEntityId, Entity>,
}

/// Marker for entities that are controlled by a player. Derived post-hydration
/// from OwnerId pointing at a player entity. Not persisted; runtime-only.
#[derive(Debug, Component)]
pub struct SimulatedControlledEntity {
    pub player_entity_id: PlayerEntityId,
}

#[derive(Resource, Default)]
pub struct PlayerRuntimeEntityMap {
    pub by_player_entity_id: HashMap<String, Entity>,
}

pub fn init_resources(app: &mut App) {
    app.insert_resource(PlayerControlledEntityMap::default());
    app.insert_resource(PlayerRuntimeEntityMap::default());
}

fn find_runtime_guid_collisions(records: &[GraphEntityRecord]) -> Vec<(String, Vec<String>)> {
    let mut entity_ids_by_guid = HashMap::<String, Vec<String>>::new();
    for record in records {
        let Some(guid) = parse_guid_from_entity_id(&record.entity_id) else {
            continue;
        };
        entity_ids_by_guid
            .entry(guid.to_string())
            .or_default()
            .push(record.entity_id.clone());
    }
    let mut collisions = entity_ids_by_guid
        .into_iter()
        .filter_map(|(guid, mut entity_ids)| {
            entity_ids.sort();
            entity_ids.dedup();
            (entity_ids.len() > 1).then_some((guid, entity_ids))
        })
        .collect::<Vec<_>>();
    collisions.sort_by(|a, b| a.0.cmp(&b.0));
    collisions
}

/// Resolves a human-readable name for an entity from its component records,
/// falling back to the entity_id/UUID.
fn resolve_display_name(
    record: &GraphEntityRecord,
    type_paths: &HashMap<String, String>,
) -> String {
    component_record(&record.components, "display_name")
        .and_then(|c| decode_graph_component_payload(c, type_paths))
        .and_then(|payload| serde_json::from_value::<DisplayName>(payload.clone()).ok())
        .map(|dn| dn.0)
        .unwrap_or_else(|| {
            parse_guid_from_entity_id(&record.entity_id)
                .map(|g| g.to_string())
                .unwrap_or_else(|| record.entity_id.clone())
        })
}

fn vec2_from_value_recursive(value: &serde_json::Value) -> Option<DVec2> {
    if let Some(values) = value.as_array()
        && values.len() == 2
    {
        return Some(DVec2::new(values[0].as_f64()?, values[1].as_f64()?));
    }
    let object = value.as_object()?;
    if let (Some(x), Some(y)) = (object.get("x"), object.get("y")) {
        return Some(DVec2::new(x.as_f64()?, y.as_f64()?));
    }
    for nested in object.values() {
        if let Some(value) = vec2_from_value_recursive(nested) {
            return Some(value);
        }
    }
    None
}

fn rotation_radians_from_value_recursive(value: &serde_json::Value) -> Option<f64> {
    if let Ok(rotation) = serde_json::from_value::<Rotation>(value.clone()) {
        return Some(rotation.as_radians());
    }
    if let Some(radians) = value.as_f64() {
        return Some(radians);
    }
    let object = value.as_object()?;
    if let (Some(cos), Some(sin)) = (object.get("cos"), object.get("sin")) {
        return Some(sin.as_f64()?.atan2(cos.as_f64()?));
    }
    for nested in object.values() {
        if let Some(value) = rotation_radians_from_value_recursive(nested) {
            return Some(value);
        }
    }
    None
}

fn initial_transform_from_graph_record(
    record: &GraphEntityRecord,
    type_paths: &HashMap<String, String>,
) -> Transform {
    let planar_position = component_record(&record.components, "avian_position")
        .and_then(|component| decode_graph_component_payload(component, type_paths))
        .and_then(vec2_from_value_recursive)
        .or_else(|| {
            component_record(&record.components, "world_position")
                .and_then(|component| decode_graph_component_payload(component, type_paths))
                .and_then(vec2_from_value_recursive)
        })
        .filter(|value| value.is_finite())
        .unwrap_or(DVec2::ZERO);
    let heading = component_record(&record.components, "avian_rotation")
        .and_then(|component| decode_graph_component_payload(component, type_paths))
        .and_then(rotation_radians_from_value_recursive)
        .or_else(|| {
            component_record(&record.components, "world_rotation")
                .and_then(|component| decode_graph_component_payload(component, type_paths))
                .and_then(rotation_radians_from_value_recursive)
        })
        .filter(|value| value.is_finite())
        .unwrap_or(0.0);

    Transform {
        translation: Vec3::new(planar_position.x as f32, planar_position.y as f32, 0.0),
        rotation: Quat::from_rotation_z(heading as f32),
        ..Default::default()
    }
}

/// General-purpose entity hydration: takes a set of graph records and spawns them
/// into the Bevy world with full component and hierarchy support.
///
/// Entity-agnostic and reusable for:
/// 1. Startup bulk hydration (entire graph DB)
/// 2. Bootstrap deferred hydration (player's entities loaded post-startup)
/// 3. Future spatial streaming (entity subgraphs entering visibility range)
///
/// The `existing_guids` set prevents double-spawning entities already in the world.
/// Returns a map of entity_id -> spawned Bevy Entity.
pub fn hydrate_records_into_world(
    commands: &mut Commands<'_, '_>,
    records: &[GraphEntityRecord],
    component_registry: &GeneratedComponentRegistry,
    app_type_registry: &AppTypeRegistry,
    existing_guids: &HashSet<uuid::Uuid>,
    player_entity_map: &mut PlayerRuntimeEntityMap,
    controlled_entity_map: &mut PlayerControlledEntityMap,
) -> HashMap<String, Entity> {
    let type_paths = component_type_path_map(component_registry);

    let mut parentless_records = Vec::new();
    let mut parented_records = Vec::new();
    for record in records {
        if record
            .properties
            .get("parent_entity_id")
            .and_then(|v| v.as_str())
            .is_some()
        {
            parented_records.push(record);
        } else {
            parentless_records.push(record);
        }
    }

    let mut spawned_entity_by_id = HashMap::<String, Entity>::new();
    let mut hydrated_count = 0usize;

    for record in parentless_records.iter().chain(parented_records.iter()) {
        let entity_guid =
            parse_guid_from_entity_id(&record.entity_id).unwrap_or_else(uuid::Uuid::new_v4);

        if existing_guids.contains(&entity_guid) {
            let guid_key = entity_guid.to_string();
            spawned_entity_by_id.insert(record.entity_id.clone(), Entity::PLACEHOLDER);
            if guid_key != record.entity_id {
                spawned_entity_by_id.insert(guid_key, Entity::PLACEHOLDER);
            }
            continue;
        }

        let display_name = resolve_display_name(record, &type_paths);

        let initial_transform = initial_transform_from_graph_record(record, &type_paths);
        let entity_commands = commands.spawn((
            Name::new(display_name),
            EntityGuid(entity_guid),
            initial_transform,
            Visibility::default(),
        ));
        let entity = entity_commands.id();

        insert_registered_components_from_graph_records(
            commands,
            entity,
            &record.components,
            &type_paths,
            app_type_registry,
        );
        let is_player_anchor = component_record(&record.components, "player_tag").is_some();
        let has_position = component_record(&record.components, "position").is_some();
        // Insert replication targets only after gameplay components are hydrated so
        // initial spawn snapshots do not leak default/uninitialized component values.
        let mut entity_commands = commands.entity(entity);
        if is_player_anchor {
            // Player anchor entities are private observer/runtime state and must never
            // be broadcast globally. Auth binding later assigns owner-only replication.
            entity_commands.insert(Replicate::to_clients(NetworkTarget::None));
        } else {
            entity_commands.insert(Replicate::to_clients(NetworkTarget::All));
        }
        if !is_player_anchor && has_position {
            entity_commands.insert(InterpolationTarget::to_clients(NetworkTarget::All));
        }
        let has_action_queue = component_record(&record.components, "action_queue").is_some();
        let has_flight_control = component_record(&record.components, "flight_computer").is_some();
        if !has_action_queue && (is_player_anchor || has_flight_control) {
            bevy::log::warn!(
                "hydration missing action_queue for entity_id={} (player/flight entity must provide canonical action_queue payload)",
                record.entity_id
            );
        }

        spawned_entity_by_id.insert(record.entity_id.clone(), entity);
        let guid_key = entity_guid.to_string();
        if guid_key != record.entity_id {
            spawned_entity_by_id.insert(guid_key, entity);
        }
        hydrated_count += 1;
    }

    derive_control_bindings(
        commands,
        &spawned_entity_by_id,
        records,
        &type_paths,
        controlled_entity_map,
        player_entity_map,
    );
    ensure_simulated_controlled_entities(
        commands,
        &spawned_entity_by_id,
        records,
        &type_paths,
        player_entity_map,
    );

    if hydrated_count > 0 {
        bevy::log::info!("hydrated {hydrated_count} entities into world");
    }

    spawned_entity_by_id
}

/// Startup system: loads all entities from the graph database and hydrates them.
#[allow(clippy::too_many_arguments)]
pub fn hydrate_simulation_entities(
    mut commands: Commands<'_, '_>,
    mut controlled_entity_map: ResMut<'_, PlayerControlledEntityMap>,
    mut player_entity_map: ResMut<'_, PlayerRuntimeEntityMap>,
    mut schema_init_state: ResMut<'_, PersistenceSchemaInitState>,
    script_catalog: Res<'_, ScriptCatalogResource>,
    entity_registry: Res<'_, EntityRegistryResource>,
    asset_registry: Res<'_, AssetRegistryResource>,
    component_registry: Res<'_, GeneratedComponentRegistry>,
    app_type_registry: Res<'_, AppTypeRegistry>,
) {
    let records = match load_runtime_world_records_with_retry(
        &mut schema_init_state,
        script_catalog.as_ref(),
        entity_registry.as_ref(),
        asset_registry.as_ref(),
    ) {
        Err(err) => {
            error!("replication simulation hydration skipped: {err}");
            return;
        }
        Ok(records) => records,
    };

    for record in &records {
        commands.spawn(HydratedGraphEntity {
            _entity_id: record.entity_id.clone(),
            _labels: record.labels.clone(),
            _component_count: record.components.len(),
        });
    }
    commands.insert_resource(HydratedEntityCount {
        _count: records.len(),
    });
    let collisions = find_runtime_guid_collisions(&records);
    if !collisions.is_empty() {
        let formatted = collisions
            .iter()
            .map(|(guid, entity_ids)| format!("guid {} reused by {:?}", guid, entity_ids))
            .collect::<Vec<_>>()
            .join("; ");
        error!(
            "replication simulation hydration aborted: runtime GUID collisions detected: {formatted}"
        );
        return;
    }

    let existing_guids = HashSet::new();
    hydrate_records_into_world(
        &mut commands,
        &records,
        &component_registry,
        &app_type_registry,
        &existing_guids,
        &mut player_entity_map,
        &mut controlled_entity_map,
    );
}

#[allow(clippy::too_many_arguments)]
pub fn reload_runtime_world_from_persistence(
    commands: &mut Commands<'_, '_>,
    script_catalog: &ScriptCatalogResource,
    entity_registry: &EntityRegistryResource,
    asset_registry: &AssetRegistryResource,
    component_registry: &GeneratedComponentRegistry,
    app_type_registry: &AppTypeRegistry,
    controlled_entity_map: &mut PlayerControlledEntityMap,
    player_entity_map: &mut PlayerRuntimeEntityMap,
    schema_init_state: &mut PersistenceSchemaInitState,
) -> Result<usize, String> {
    let records = load_runtime_world_records_with_retry(
        schema_init_state,
        script_catalog,
        entity_registry,
        asset_registry,
    )?;
    let collisions = find_runtime_guid_collisions(&records);
    if !collisions.is_empty() {
        let formatted = collisions
            .iter()
            .map(|(guid, entity_ids)| format!("guid {} reused by {:?}", guid, entity_ids))
            .collect::<Vec<_>>()
            .join("; ");
        return Err(format!(
            "reload world aborted: runtime GUID collisions detected: {formatted}"
        ));
    }

    for record in &records {
        commands.spawn(HydratedGraphEntity {
            _entity_id: record.entity_id.clone(),
            _labels: record.labels.clone(),
            _component_count: record.components.len(),
        });
    }
    commands.insert_resource(HydratedEntityCount {
        _count: records.len(),
    });
    let existing_guids = HashSet::new();
    hydrate_records_into_world(
        commands,
        &records,
        component_registry,
        app_type_registry,
        &existing_guids,
        player_entity_map,
        controlled_entity_map,
    );
    Ok(records.len())
}

fn load_runtime_world_records_with_retry(
    schema_init_state: &mut PersistenceSchemaInitState,
    script_catalog: &ScriptCatalogResource,
    entity_registry: &EntityRegistryResource,
    asset_registry: &AssetRegistryResource,
) -> Result<Vec<GraphEntityRecord>, String> {
    retry_startup_persistence("runtime world load", || {
        let mut persistence = GraphPersistence::connect(&replication_database_url())
            .map_err(|err| format!("connect failed: {err}"))?;
        if !schema_init_state.0 {
            persistence
                .ensure_schema()
                .map_err(|err| format!("schema init failed: {err}"))?;
            schema_init_state.0 = true;
        }
        apply_scripted_world_init_once(
            &mut persistence,
            script_catalog,
            entity_registry,
            asset_registry,
        )
        .map_err(|err| format!("scripted world init failed: {err}"))?;
        persistence
            .load_graph_records()
            .map_err(|err| format!("graph load failed: {err}"))
    })
}

fn retry_startup_persistence<T, F>(label: &str, mut operation: F) -> Result<T, String>
where
    F: FnMut() -> Result<T, String>,
{
    let mut last_err = None;
    for attempt in 1..=STARTUP_PERSISTENCE_RETRY_ATTEMPTS {
        match operation() {
            Ok(value) => return Ok(value),
            Err(err) => {
                if attempt == STARTUP_PERSISTENCE_RETRY_ATTEMPTS {
                    return Err(format!(
                        "{label} failed after {} attempts: {err}",
                        STARTUP_PERSISTENCE_RETRY_ATTEMPTS
                    ));
                }
                warn!(
                    "{label} attempt {attempt}/{} failed: {err}; retrying in {}ms",
                    STARTUP_PERSISTENCE_RETRY_ATTEMPTS,
                    STARTUP_PERSISTENCE_RETRY_DELAY.as_millis()
                );
                last_err = Some(err);
                thread::sleep(STARTUP_PERSISTENCE_RETRY_DELAY);
            }
        }
    }
    Err(format!(
        "{label} failed after {} attempts: {}",
        STARTUP_PERSISTENCE_RETRY_ATTEMPTS,
        last_err.unwrap_or_else(|| "retry loop exited without an error".to_string())
    ))
}

fn derive_control_bindings(
    commands: &mut Commands<'_, '_>,
    spawned: &HashMap<String, Entity>,
    records: &[GraphEntityRecord],
    type_paths: &HashMap<String, String>,
    controlled_map: &mut PlayerControlledEntityMap,
    player_map: &mut PlayerRuntimeEntityMap,
) {
    let mut player_entities = HashMap::<PlayerEntityId, Entity>::new();
    let mut desired_control_by_player = HashMap::<PlayerEntityId, Option<uuid::Uuid>>::new();

    for record in records {
        let has_player_tag = component_record(&record.components, "player_tag").is_some();
        if !has_player_tag {
            continue;
        }
        let guid = parse_guid_from_entity_id(&record.entity_id);
        let Some(&entity) = spawned.get(&record.entity_id) else {
            continue;
        };
        if entity == Entity::PLACEHOLDER {
            continue;
        }
        let Some(player_id) = guid.map(PlayerEntityId) else {
            warn!(
                "derive_control_bindings skipping player record with non-uuid entity_id={}",
                record.entity_id
            );
            continue;
        };

        player_entities.insert(player_id, entity);
        player_map
            .by_player_entity_id
            .insert(record.entity_id.clone(), entity);
        player_map
            .by_player_entity_id
            .insert(player_id.canonical_wire_id(), entity);

        let control_guid = component_record(&record.components, "controlled_entity_guid")
            .and_then(|c| decode_graph_component_payload(c, type_paths))
            .and_then(|payload| {
                serde_json::from_value::<ControlledEntityGuid>(payload.clone()).ok()
            })
            .and_then(|v| v.0)
            .and_then(|raw| sidereal_net::RuntimeEntityId::parse(raw.as_str()).map(|id| id.0));
        desired_control_by_player.insert(player_id, control_guid);
    }

    for record in records {
        if component_record(&record.components, "player_tag").is_some() {
            continue;
        }
        let owner_id = component_record(&record.components, "owner_id")
            .and_then(|c| decode_graph_component_payload(c, type_paths))
            .and_then(|payload| serde_json::from_value::<OwnerId>(payload.clone()).ok());
        let Some(owner) = owner_id else { continue };

        let Some(player_id) = PlayerEntityId::parse(owner.0.as_str()) else {
            continue;
        };

        if !player_entities.contains_key(&player_id) {
            continue;
        }

        let Some(&entity) = spawned.get(&record.entity_id) else {
            continue;
        };
        if entity == Entity::PLACEHOLDER {
            continue;
        }
        let entity_guid = parse_guid_from_entity_id(&record.entity_id);

        commands.entity(entity).insert(SimulatedControlledEntity {
            player_entity_id: player_id,
        });

        // Check if this entity matches any player's desired control target.
        if let Some(desired) = desired_control_by_player.get(&player_id) {
            let matches = desired
                .as_ref()
                .is_some_and(|guid| entity_guid == Some(*guid));
            if matches {
                controlled_map.by_player_entity_id.insert(player_id, entity);
            }
        }
    }

    // Handle free-roam: player's ControlledEntityGuid points to its own GUID.
    for (player_id, &player_entity) in &player_entities {
        if controlled_map
            .by_player_entity_id
            .values()
            .any(|&e| e == player_entity)
        {
            continue;
        }
        let desired = desired_control_by_player.get(player_id);
        let is_self_control =
            desired.is_some_and(|d| d.as_ref().is_some_and(|guid| *guid == player_id.0));
        if is_self_control {
            controlled_map
                .by_player_entity_id
                .insert(*player_id, player_entity);
        }
    }
}

fn ensure_simulated_controlled_entities(
    commands: &mut Commands<'_, '_>,
    spawned: &HashMap<String, Entity>,
    records: &[GraphEntityRecord],
    type_paths: &HashMap<String, String>,
    player_map: &PlayerRuntimeEntityMap,
) {
    for record in records {
        if component_record(&record.components, "player_tag").is_some() {
            continue;
        }
        let owner_id = component_record(&record.components, "owner_id")
            .and_then(|c| decode_graph_component_payload(c, type_paths))
            .and_then(|payload| serde_json::from_value::<OwnerId>(payload.clone()).ok());
        let Some(owner_id) = owner_id else { continue };
        let Some(player_id) = PlayerEntityId::parse(owner_id.0.as_str()) else {
            continue;
        };
        let owner_key = player_id.canonical_wire_id();
        if !player_map
            .by_player_entity_id
            .contains_key(owner_key.as_str())
        {
            continue;
        }
        let Some(&entity) = spawned.get(&record.entity_id) else {
            continue;
        };
        if entity == Entity::PLACEHOLDER {
            continue;
        }
        commands.entity(entity).insert(SimulatedControlledEntity {
            player_entity_id: player_id,
        });
    }
}

/// Collects the set of EntityGuid UUIDs already present in the Bevy world,
/// used to prevent double-spawning during deferred hydration.
fn collect_existing_guids(guid_query: &Query<'_, '_, &EntityGuid>) -> HashSet<uuid::Uuid> {
    guid_query.iter().map(|g| g.0).collect()
}

/// Filters graph records to those belonging to a specific player: the player
/// entity itself (matched by entity_id) plus all entities whose OwnerId
/// component value matches the player_entity_id.
fn filter_records_for_player(
    all_records: &[GraphEntityRecord],
    player_entity_id: &str,
    type_paths: &HashMap<String, String>,
) -> Vec<GraphEntityRecord> {
    let player_guid = parse_guid_from_entity_id(player_entity_id)
        .map(|g| g.to_string())
        .unwrap_or_else(|| player_entity_id.to_string());

    all_records
        .iter()
        .filter(|record| {
            let record_guid = parse_guid_from_entity_id(&record.entity_id)
                .map(|g| g.to_string())
                .unwrap_or_else(|| record.entity_id.clone());
            if record_guid == player_guid || record.entity_id == player_entity_id {
                return true;
            }
            let owner_id = component_record(&record.components, "owner_id")
                .and_then(|c| decode_graph_component_payload(c, type_paths))
                .and_then(|payload| serde_json::from_value::<OwnerId>(payload.clone()).ok());
            if let Some(owner) = owner_id {
                let owner_guid = parse_guid_from_entity_id(&owner.0)
                    .map(|g| g.to_string())
                    .unwrap_or(owner.0.clone());
                return owner_guid == player_guid || owner.0 == player_entity_id;
            }
            false
        })
        .cloned()
        .collect()
}

fn replication_database_url() -> String {
    std::env::var("REPLICATION_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://sidereal:sidereal@127.0.0.1:5432/sidereal".to_string())
}

fn apply_scripted_world_init_once(
    persistence: &mut GraphPersistence,
    script_catalog: &ScriptCatalogResource,
    entity_registry: &EntityRegistryResource,
    asset_registry: &AssetRegistryResource,
) -> Result<(), String> {
    if persistence
        .script_world_init_state_exists(WORLD_INIT_STATE_KEY)
        .map_err(|err| format!("query world init state failed: {err}"))?
    {
        bevy::log::info!(
            "replication scripted world init skipped (marker exists) key={}",
            WORLD_INIT_STATE_KEY
        );
        return Ok(());
    }

    let records = load_world_init_graph_records_from_catalog(
        script_catalog,
        &entity_registry.entries,
        &asset_registry.entries,
    )?;
    bevy::log::info!(
        "replication applying scripted world init records count={} key={}",
        records.len(),
        WORLD_INIT_STATE_KEY
    );
    persistence
        .persist_graph_records(&records, 0)
        .map_err(|err| format!("persist scripted world init records failed: {err}"))?;

    let now_epoch_s = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    persistence
        .insert_script_world_init_state(WORLD_INIT_STATE_KEY, "world/world_init.lua", now_epoch_s)
        .map_err(|err| format!("insert world init state failed: {err}"))?;
    bevy::log::info!(
        "replication applied scripted world init from data/scripts/world/world_init.lua"
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn process_bootstrap_entity_commands(
    mut commands: Commands<'_, '_>,
    mut controlled_entity_map: ResMut<'_, PlayerControlledEntityMap>,
    mut player_entity_map: ResMut<'_, PlayerRuntimeEntityMap>,
    component_registry: Res<'_, GeneratedComponentRegistry>,
    app_type_registry: Res<'_, AppTypeRegistry>,
    all_guids: Query<'_, '_, &'_ EntityGuid>,
    simulated_entities: Query<
        '_,
        '_,
        (Entity, &'_ EntityGuid, &'_ OwnerId),
        With<SimulatedControlledEntity>,
    >,
    script_catalog: Res<'_, ScriptCatalogResource>,
    entity_registry: Res<'_, EntityRegistryResource>,
    asset_registry: Res<'_, AssetRegistryResource>,
    mut cached_scripts_root: Local<'_, Option<std::path::PathBuf>>,
    receiver: Option<Res<'_, BootstrapEntityReceiver>>,
) {
    let Some(receiver) = receiver else { return };
    let mut processed_players = HashSet::new();
    let scripts_root = cached_scripts_root
        .get_or_insert_with(scripts_root_dir)
        .clone();
    let known_bundle_ids = (!entity_registry.entries.is_empty()).then(|| {
        entity_registry
            .entries
            .iter()
            .map(|entry| entry.entity_id.clone())
            .collect::<HashSet<_>>()
    });

    for cmd in bootstrap_runtime::drain_bootstrap_entity_commands(receiver.as_ref()) {
        match cmd.payload {
            BootstrapEntityCommandPayload::BootstrapPlayer { player_entity_id } => {
                process_bootstrap_player_entity_command(
                    &mut commands,
                    &mut controlled_entity_map,
                    &mut player_entity_map,
                    &component_registry,
                    &app_type_registry,
                    &all_guids,
                    &simulated_entities,
                    &mut processed_players,
                    &player_entity_id,
                );
            }
            BootstrapEntityCommandPayload::AdminSpawnEntity {
                actor_account_id,
                actor_player_entity_id,
                request_id,
                player_entity_id,
                bundle_id,
                requested_entity_id,
                overrides,
            } => {
                let actor_account_id_wire = actor_account_id.to_string();
                let request_id_wire = request_id.to_string();
                process_admin_spawn_command(
                    &mut commands,
                    &mut controlled_entity_map,
                    &mut player_entity_map,
                    &component_registry,
                    &app_type_registry,
                    &all_guids,
                    &scripts_root,
                    known_bundle_ids.as_ref(),
                    &script_catalog,
                    &entity_registry,
                    &asset_registry,
                    actor_account_id_wire.as_str(),
                    actor_player_entity_id.as_str(),
                    request_id_wire.as_str(),
                    player_entity_id.as_str(),
                    bundle_id.as_str(),
                    requested_entity_id.as_str(),
                    overrides,
                );
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn process_bootstrap_player_entity_command(
    commands: &mut Commands<'_, '_>,
    controlled_entity_map: &mut ResMut<'_, PlayerControlledEntityMap>,
    player_entity_map: &mut ResMut<'_, PlayerRuntimeEntityMap>,
    component_registry: &Res<'_, GeneratedComponentRegistry>,
    app_type_registry: &Res<'_, AppTypeRegistry>,
    all_guids: &Query<'_, '_, &'_ EntityGuid>,
    simulated_entities: &Query<
        '_,
        '_,
        (Entity, &'_ EntityGuid, &'_ OwnerId),
        With<SimulatedControlledEntity>,
    >,
    processed_players: &mut HashSet<String>,
    player_entity_id: &str,
) {
    if !processed_players.insert(player_entity_id.to_string()) {
        return;
    }
    let Some(player_id) = PlayerEntityId::parse(player_entity_id) else {
        bevy::log::warn!(
            "bootstrap: invalid player_entity_id={}, skipping",
            player_entity_id
        );
        return;
    };

    // If the player entity isn't in the world yet, attempt deferred hydration.
    if !player_entity_map
        .by_player_entity_id
        .contains_key(player_entity_id)
    {
        bevy::log::info!(
            "bootstrap: player {} not in world; attempting deferred hydration",
            player_entity_id
        );
        let database_url = replication_database_url();
        let mut persistence = match GraphPersistence::connect(&database_url) {
            Ok(v) => v,
            Err(err) => {
                bevy::log::error!("bootstrap deferred hydration failed (connect): {err}");
                return;
            }
        };
        let all_records = match persistence.load_graph_records() {
            Ok(v) => v,
            Err(err) => {
                bevy::log::error!("bootstrap deferred hydration failed (load): {err}");
                return;
            }
        };
        let type_paths = component_type_path_map(component_registry);
        let player_records = filter_records_for_player(&all_records, player_entity_id, &type_paths);
        if player_records.is_empty() {
            bevy::log::warn!(
                "bootstrap: player {} has no records in graph DB; skipping",
                player_entity_id
            );
            return;
        }
        let existing_guids = collect_existing_guids(all_guids);
        hydrate_records_into_world(
            commands,
            &player_records,
            component_registry,
            app_type_registry,
            &existing_guids,
            player_entity_map,
            controlled_entity_map,
        );
        bevy::log::info!(
            "bootstrap: deferred hydration complete for player {} ({} records)",
            player_entity_id,
            player_records.len()
        );
    }

    let Some(&player_entity) = player_entity_map.by_player_entity_id.get(player_entity_id) else {
        bevy::log::warn!(
            "bootstrap: player entity {} still not found after deferred hydration; skipping",
            player_entity_id
        );
        return;
    };

    let has_live_controlled_entity = controlled_entity_map
        .by_player_entity_id
        .get(&player_id)
        .is_some_and(|entity| all_guids.get(*entity).is_ok());

    if !has_live_controlled_entity {
        let mut existing_matches = simulated_entities
            .iter()
            .filter(|(_, _, owner)| {
                PlayerEntityId::parse(owner.0.as_str()).is_some_and(|id| id == player_id)
            })
            .collect::<Vec<_>>();
        if let Some((existing_entity, existing_guid, _)) = existing_matches.pop() {
            if !existing_matches.is_empty() {
                bevy::log::warn!(
                    "bootstrap found duplicate controlled entities for player={}; using latest match",
                    player_entity_id
                );
            }
            controlled_entity_map
                .by_player_entity_id
                .insert(player_id, existing_entity);
            commands
                .entity(player_entity)
                .insert(ControlledEntityGuid(Some(existing_guid.0.to_string())));
        }
    }

    if let Some(&controlled_entity) = controlled_entity_map.by_player_entity_id.get(&player_id)
        && let Ok(control_guid) = all_guids.get(controlled_entity)
    {
        commands
            .entity(player_entity)
            .insert(ControlledEntityGuid(Some(control_guid.0.to_string())));
    }
}

#[allow(clippy::too_many_arguments)]
fn process_admin_spawn_command(
    commands: &mut Commands<'_, '_>,
    controlled_entity_map: &mut ResMut<'_, PlayerControlledEntityMap>,
    player_entity_map: &mut ResMut<'_, PlayerRuntimeEntityMap>,
    component_registry: &Res<'_, GeneratedComponentRegistry>,
    app_type_registry: &Res<'_, AppTypeRegistry>,
    all_guids: &Query<'_, '_, &'_ EntityGuid>,
    scripts_root: &std::path::Path,
    known_bundle_ids: Option<&HashSet<String>>,
    script_catalog: &Res<'_, ScriptCatalogResource>,
    entity_registry: &Res<'_, EntityRegistryResource>,
    asset_registry: &Res<'_, AssetRegistryResource>,
    actor_account_id: &str,
    actor_player_entity_id: &str,
    request_id: &str,
    player_entity_id: &str,
    bundle_id: &str,
    requested_entity_id: &str,
    mut overrides: serde_json::Map<String, serde_json::Value>,
) {
    let Some(player_uuid) = PlayerEntityId::parse(player_entity_id) else {
        bevy::log::warn!(
            "admin spawn rejected request_id={} actor_account_id={} actor_player_entity_id={}: invalid player_entity_id={}",
            request_id,
            actor_account_id,
            actor_player_entity_id,
            player_entity_id
        );
        return;
    };
    let Ok(requested_uuid) = uuid::Uuid::parse_str(requested_entity_id) else {
        bevy::log::warn!(
            "admin spawn rejected request_id={} actor_account_id={} actor_player_entity_id={}: invalid requested_entity_id={}",
            request_id,
            actor_account_id,
            actor_player_entity_id,
            requested_entity_id
        );
        return;
    };
    let canonical_player_id = player_uuid.canonical_wire_id();
    if !player_entity_map
        .by_player_entity_id
        .contains_key(canonical_player_id.as_str())
    {
        bevy::log::warn!(
            "admin spawn rejected request_id={} actor_account_id={} actor_player_entity_id={}: unknown player_entity_id={}",
            request_id,
            actor_account_id,
            actor_player_entity_id,
            canonical_player_id
        );
        return;
    }
    if let Some(known_bundle_ids) = known_bundle_ids
        && !known_bundle_ids.contains(bundle_id)
    {
        bevy::log::warn!(
            "admin spawn rejected request_id={} actor_account_id={} actor_player_entity_id={}: unknown bundle_id={}",
            request_id,
            actor_account_id,
            actor_player_entity_id,
            bundle_id
        );
        return;
    }
    if overrides.len() > ADMIN_MAX_OVERRIDE_FIELDS {
        bevy::log::warn!(
            "admin spawn rejected request_id={} actor_account_id={} actor_player_entity_id={}: too many overrides fields={}",
            request_id,
            actor_account_id,
            actor_player_entity_id,
            overrides.len()
        );
        return;
    }
    let Ok(override_payload_len) = serde_json::to_vec(&overrides).map(|bytes| bytes.len()) else {
        bevy::log::warn!(
            "admin spawn rejected request_id={} actor_account_id={} actor_player_entity_id={}: failed serializing overrides",
            request_id,
            actor_account_id,
            actor_player_entity_id
        );
        return;
    };
    if override_payload_len > ADMIN_MAX_OVERRIDE_JSON_BYTES {
        bevy::log::warn!(
            "admin spawn rejected request_id={} actor_account_id={} actor_player_entity_id={}: override payload too large bytes={}",
            request_id,
            actor_account_id,
            actor_player_entity_id,
            override_payload_len
        );
        return;
    }
    if let Some((key, _)) = overrides
        .iter()
        .find(|(key, _)| !ADMIN_ALLOWED_OVERRIDE_KEYS.contains(&key.as_str()))
    {
        bevy::log::warn!(
            "admin spawn rejected request_id={} actor_account_id={} actor_player_entity_id={}: override key not allowed key={}",
            request_id,
            actor_account_id,
            actor_player_entity_id,
            key
        );
        return;
    }
    if let Some(value) = overrides.get("display_name")
        && !value.is_string()
    {
        bevy::log::warn!(
            "admin spawn rejected request_id={} actor_account_id={} actor_player_entity_id={}: display_name must be a string",
            request_id,
            actor_account_id,
            actor_player_entity_id
        );
        return;
    }
    if let Some(owner_value) = overrides.get("owner_id")
        && !(owner_value.is_null() || owner_value.is_string())
    {
        bevy::log::warn!(
            "admin spawn rejected request_id={} actor_account_id={} actor_player_entity_id={}: owner_id override must be null or string",
            request_id,
            actor_account_id,
            actor_player_entity_id
        );
        return;
    }

    if !overrides.contains_key("owner_id") {
        overrides.insert(
            "owner_id".to_string(),
            serde_json::Value::String(canonical_player_id.clone()),
        );
    }
    overrides.insert(
        "entity_id".to_string(),
        serde_json::Value::String(requested_uuid.to_string()),
    );

    let graph_records = match spawn_bundle_graph_records_cached(
        scripts_root,
        script_catalog.as_ref(),
        entity_registry.as_ref(),
        asset_registry.as_ref(),
        bundle_id,
        &overrides,
    ) {
        Ok(records) => records,
        Err(err) => {
            bevy::log::error!(
                "admin spawn failed request_id={} actor_account_id={} actor_player_entity_id={} target_player_entity_id={} bundle_id={}: bundle spawn failed: {}",
                request_id,
                actor_account_id,
                actor_player_entity_id,
                canonical_player_id,
                bundle_id,
                err
            );
            return;
        }
    };
    if graph_records
        .first()
        .is_none_or(|record| record.entity_id != requested_entity_id)
    {
        bevy::log::error!(
            "admin spawn failed request_id={} actor_account_id={} actor_player_entity_id={} target_player_entity_id={} bundle_id={}: root entity_id mismatch requested={} actual_first={:?}",
            request_id,
            actor_account_id,
            actor_player_entity_id,
            canonical_player_id,
            bundle_id,
            requested_entity_id,
            graph_records.first().map(|r| r.entity_id.as_str())
        );
        return;
    }

    let database_url = replication_database_url();
    let mut persistence = match GraphPersistence::connect(&database_url) {
        Ok(v) => v,
        Err(err) => {
            bevy::log::error!(
                "admin spawn failed request_id={} actor_account_id={} actor_player_entity_id={} target_player_entity_id={} bundle_id={} requested_entity_id={}: db connect failed: {}",
                request_id,
                actor_account_id,
                actor_player_entity_id,
                canonical_player_id,
                bundle_id,
                requested_entity_id,
                err
            );
            return;
        }
    };
    if let Err(err) = persistence.ensure_schema() {
        bevy::log::error!(
            "admin spawn failed request_id={} actor_account_id={} actor_player_entity_id={} target_player_entity_id={} bundle_id={} requested_entity_id={}: ensure schema failed: {}",
            request_id,
            actor_account_id,
            actor_player_entity_id,
            canonical_player_id,
            bundle_id,
            requested_entity_id,
            err
        );
        return;
    }
    if let Err(err) = persistence.persist_graph_records(&graph_records, 0) {
        bevy::log::error!(
            "admin spawn failed request_id={} actor_account_id={} actor_player_entity_id={} target_player_entity_id={} bundle_id={} requested_entity_id={}: persist failed: {}",
            request_id,
            actor_account_id,
            actor_player_entity_id,
            canonical_player_id,
            bundle_id,
            requested_entity_id,
            err
        );
        return;
    }

    let existing_guids = collect_existing_guids(all_guids);
    hydrate_records_into_world(
        commands,
        &graph_records,
        component_registry,
        app_type_registry,
        &existing_guids,
        player_entity_map,
        controlled_entity_map,
    );

    let mut event_payload = serde_json::Map::new();
    event_payload.insert(
        "request_id".to_string(),
        serde_json::Value::String(request_id.to_string()),
    );
    event_payload.insert(
        "actor_account_id".to_string(),
        serde_json::Value::String(actor_account_id.to_string()),
    );
    event_payload.insert(
        "actor_player_entity_id".to_string(),
        serde_json::Value::String(actor_player_entity_id.to_string()),
    );
    event_payload.insert(
        "owner_player_entity_id".to_string(),
        serde_json::Value::String(canonical_player_id.clone()),
    );
    event_payload.insert(
        "bundle_id".to_string(),
        serde_json::Value::String(bundle_id.to_string()),
    );
    event_payload.insert(
        "spawned_entity_id".to_string(),
        serde_json::Value::String(requested_entity_id.to_string()),
    );
    if let Err(err) =
        emit_bundle_spawned_event_from_catalog(script_catalog.as_ref(), &event_payload)
    {
        bevy::log::warn!(
            "admin spawn post-hook failed request_id={} actor_account_id={} actor_player_entity_id={} target_player_entity_id={} bundle_id={} requested_entity_id={}: {}",
            request_id,
            actor_account_id,
            actor_player_entity_id,
            canonical_player_id,
            bundle_id,
            requested_entity_id,
            err
        );
    }

    let display_name = overrides
        .get("display_name")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("Corvette");
    bevy::log::info!(
        "audit spawn command actor_account_id={} actor_player_entity_id={} target_player_entity_id={} bundle_id={} spawned_entity_id={} display_name={} request_id={}",
        actor_account_id,
        actor_player_entity_id,
        canonical_player_id,
        bundle_id,
        requested_entity_id,
        display_name,
        request_id
    );
}

/// Syncs Avian Position/Rotation to Bevy Transform for physics entities (With<RigidBody>).
#[allow(clippy::type_complexity)]
pub fn sync_controlled_entity_transforms(
    mut entities: Query<'_, '_, (&'_ Position, &'_ Rotation, &'_ mut Transform), With<RigidBody>>,
) {
    for (position, rotation, mut transform) in &mut entities {
        let mut planar_position = position.0;
        if !planar_position.is_finite() {
            planar_position = DVec2::ZERO;
        }
        let safe_rotation = if rotation.is_finite() {
            *rotation
        } else {
            Rotation::IDENTITY
        };
        let mut heading = safe_rotation.as_radians();
        if !heading.is_finite() {
            heading = 0.0;
        }
        transform.translation.x = planar_position.x as f32;
        transform.translation.y = planar_position.y as f32;
        transform.translation.z = 0.0;
        transform.rotation = Quat::from_rotation_z(heading as f32);
    }
}

#[allow(clippy::type_complexity)]
pub fn sync_world_entity_transforms_from_world_space(
    mut entities: Query<
        '_,
        '_,
        (
            &'_ WorldPosition,
            Option<&'_ WorldRotation>,
            &'_ mut Transform,
        ),
        Without<RigidBody>,
    >,
) {
    for (position, rotation, mut transform) in &mut entities {
        let planar_position = if position.0.is_finite() {
            position.0
        } else {
            DVec2::ZERO
        };
        let heading = rotation
            .map(|value| value.0)
            .filter(|value| value.is_finite())
            .unwrap_or(0.0);
        transform.translation.x = planar_position.x as f32;
        transform.translation.y = planar_position.y as f32;
        transform.translation.z = 0.0;
        transform.rotation = Quat::from_rotation_z(heading as f32);
    }
}

/// Sanitizes non-finite physics values to prevent NaN propagation through
/// the simulation for physics entities (With<RigidBody>).
#[allow(clippy::type_complexity)]
pub fn enforce_planar_motion(
    mut entities: Query<
        '_,
        '_,
        (
            &'_ mut Position,
            &'_ mut LinearVelocity,
            &'_ mut Rotation,
            &'_ mut AngularVelocity,
        ),
        With<RigidBody>,
    >,
) {
    for (mut position, mut velocity, mut rotation, mut angular_velocity) in &mut entities {
        if !position.0.is_finite() {
            position.0 = DVec2::ZERO;
        }
        if !velocity.0.is_finite() {
            velocity.0 = DVec2::ZERO;
        }
        if !angular_velocity.0.is_finite() {
            angular_velocity.0 = 0.0;
        }
        let mut heading = if rotation.is_finite() {
            rotation.as_radians()
        } else {
            0.0
        };
        if !heading.is_finite() {
            heading = 0.0;
        }
        *rotation = Rotation::radians(heading);
    }
}

#[cfg(test)]
mod tests {
    use super::{initial_transform_from_graph_record, retry_startup_persistence};
    use avian2d::prelude::{Position, Rotation};
    use bevy::prelude::EulerRot;
    use bevy::reflect::TypePath;
    use sidereal_persistence::{GraphComponentRecord, GraphEntityRecord};
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn hydration_initial_transform_uses_avian_spatial_components() {
        let entity_id = uuid::Uuid::new_v4().to_string();
        let mut type_paths = HashMap::new();
        type_paths.insert(
            "avian_position".to_string(),
            Position::type_path().to_string(),
        );
        type_paths.insert(
            "avian_rotation".to_string(),
            Rotation::type_path().to_string(),
        );
        let record = GraphEntityRecord {
            entity_id: entity_id.clone(),
            labels: vec!["Entity".to_string(), "Asteroid".to_string()],
            properties: serde_json::json!({}),
            components: vec![
                GraphComponentRecord {
                    component_id: format!("{entity_id}:avian_position"),
                    component_kind: "avian_position".to_string(),
                    properties: serde_json::json!([128.5, -64.25]),
                },
                GraphComponentRecord {
                    component_id: format!("{entity_id}:avian_rotation"),
                    component_kind: "avian_rotation".to_string(),
                    properties: serde_json::json!({"cos": 0.0, "sin": 1.0}),
                },
            ],
        };

        let transform = initial_transform_from_graph_record(&record, &type_paths);

        assert_eq!(transform.translation.x, 128.5);
        assert_eq!(transform.translation.y, -64.25);
        let heading = transform.rotation.to_euler(EulerRot::XYZ).2;
        assert!((heading - std::f32::consts::FRAC_PI_2).abs() < 0.0001);
    }

    #[test]
    fn startup_retry_returns_first_successful_result() {
        let attempts = AtomicUsize::new(0);
        let result = retry_startup_persistence("test operation", || {
            let attempt = attempts.fetch_add(1, Ordering::SeqCst) + 1;
            if attempt < 3 {
                Err(format!("attempt {attempt} failed"))
            } else {
                Ok(attempt)
            }
        });

        assert_eq!(result.expect("retry should eventually succeed"), 3);
        assert_eq!(attempts.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn startup_retry_returns_last_error_after_exhaustion() {
        let attempts = AtomicUsize::new(0);
        let result: Result<usize, String> = retry_startup_persistence("test operation", || {
            let attempt = attempts.fetch_add(1, Ordering::SeqCst) + 1;
            Err(format!("attempt {attempt} failed"))
        });

        let err = result.expect_err("retry should fail after exhausting attempts");
        assert!(err.contains("failed after 20 attempts"));
        assert!(err.contains("attempt 20 failed"));
        assert_eq!(attempts.load(Ordering::SeqCst), 20);
    }
}
