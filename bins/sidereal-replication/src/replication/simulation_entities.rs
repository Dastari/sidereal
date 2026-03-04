//! Simulation entity hydration and controlled-entity binding (replication server only).
//!
//! Single dependency-ordered pass: for each graph record, spawn one entity with identity
//! (Name, EntityGuid), run the generic component insert. Hierarchy is tracked via
//! `MountedOn` (UUID-based), NOT Bevy `ChildOf`/`Children`, because Lightyear cannot
//! safely replicate Bevy hierarchy (client-side entity mapping order is undefined).
//! Control binding is derived from OwnerId + PlayerTag after the generic pass.

use avian2d::prelude::{AngularVelocity, LinearVelocity, Position, RigidBody, Rotation};
use bevy::ecs::reflect::AppTypeRegistry;
use bevy::prelude::*;
use lightyear::prelude::server::ClientOf;
use lightyear::prelude::{
    ControlledBy, InterpolationTarget, Lifetime, NetworkTarget, PredictionTarget, RemoteId,
    Replicate,
};
use sidereal_game::{
    ActionQueue, ControlledEntityGuid, DisplayName, EntityGuid, GeneratedComponentRegistry, OwnerId,
};
use sidereal_net::PlayerEntityId;
use sidereal_persistence::{GraphEntityRecord, GraphPersistence};
use sidereal_runtime_sync::{
    component_record, component_type_path_map, decode_graph_component_payload,
    insert_registered_components_from_graph_records, parse_guid_from_entity_id,
};
use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::bootstrap_runtime::{self, BootstrapEntityReceiver};
use crate::replication::auth::AuthenticatedClientBindings;
use crate::replication::lifecycle::{HydratedEntityCount, HydratedGraphEntity};
use crate::replication::persistence::PersistenceSchemaInitState;
use crate::replication::scripting::{load_world_init_graph_records, scripts_root_dir};

const WORLD_INIT_STATE_KEY: &str = "world/world_init.lua:phase2";

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

/// Deferred (client_entity, controlled_entity) bindings so ControlledBy is applied in PostUpdate,
/// avoiding same-frame entity/hierarchy ordering issues during replication.
#[derive(Resource, Default)]
pub struct PendingControlledByBindings {
    pub bindings: Vec<(Entity, Entity)>,
}

#[derive(Resource, Default)]
pub struct PlayerRuntimeEntityMap {
    pub by_player_entity_id: HashMap<String, Entity>,
}

pub fn init_resources(app: &mut App) {
    app.insert_resource(PlayerControlledEntityMap::default());
    app.insert_resource(PlayerRuntimeEntityMap::default());
    app.insert_resource(PendingControlledByBindings::default());
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

        let entity_commands = commands.spawn((
            Name::new(display_name),
            EntityGuid(entity_guid),
            Transform::default(),
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
                "hydration inserted default action_queue for entity_id={} (script payload missing or invalid action_queue)",
                record.entity_id
            );
            commands.entity(entity).insert(ActionQueue::default());
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

    if hydrated_count > 0 {
        bevy::log::info!("hydrated {hydrated_count} entities into world");
    }

    spawned_entity_by_id
}

/// Startup system: loads all entities from the graph database and hydrates them.
pub fn hydrate_simulation_entities(
    mut commands: Commands<'_, '_>,
    mut controlled_entity_map: ResMut<'_, PlayerControlledEntityMap>,
    mut player_entity_map: ResMut<'_, PlayerRuntimeEntityMap>,
    mut schema_init_state: ResMut<'_, PersistenceSchemaInitState>,
    component_registry: Res<'_, GeneratedComponentRegistry>,
    app_type_registry: Res<'_, AppTypeRegistry>,
) {
    let mut persistence = match GraphPersistence::connect(&replication_database_url()) {
        Ok(v) => v,
        Err(err) => {
            eprintln!("replication simulation hydration skipped; connect failed: {err}");
            return;
        }
    };
    if let Err(err) = persistence.ensure_schema() {
        eprintln!("replication simulation hydration skipped; schema init failed: {err}");
        return;
    }
    schema_init_state.0 = true;

    if let Err(err) = apply_scripted_world_init_once(&mut persistence) {
        eprintln!("replication simulation hydration skipped; scripted world init failed: {err}");
        return;
    }

    let records = match persistence.load_graph_records() {
        Ok(v) => v,
        Err(err) => {
            eprintln!("replication simulation hydration skipped; graph load failed: {err}");
            return;
        }
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
        eprintln!(
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

        // The owner_id value might be a bare UUID or a legacy "player:uuid".
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

fn apply_scripted_world_init_once(persistence: &mut GraphPersistence) -> Result<(), String> {
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

    let scripts_root = scripts_root_dir();
    let records = load_world_init_graph_records(&scripts_root)?;
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
    mut pending_controlled_by: ResMut<'_, PendingControlledByBindings>,
    bindings: Res<'_, AuthenticatedClientBindings>,
    component_registry: Res<'_, GeneratedComponentRegistry>,
    app_type_registry: Res<'_, AppTypeRegistry>,
    all_guids: Query<'_, '_, &'_ EntityGuid>,
    simulated_entities: Query<
        '_,
        '_,
        (Entity, &'_ EntityGuid, &'_ OwnerId),
        With<SimulatedControlledEntity>,
    >,
    receiver: Option<Res<'_, BootstrapEntityReceiver>>,
) {
    let Some(receiver) = receiver else { return };
    let mut processed_players = HashSet::new();
    for cmd in bootstrap_runtime::drain_bootstrap_entity_commands(receiver.as_ref()) {
        if !processed_players.insert(cmd.player_entity_id.clone()) {
            continue;
        }
        let Some(player_id) = PlayerEntityId::parse(cmd.player_entity_id.as_str()) else {
            bevy::log::warn!(
                "bootstrap: invalid player_entity_id={}, skipping",
                cmd.player_entity_id
            );
            continue;
        };

        // If the player entity isn't in the world yet, attempt deferred hydration.
        if !player_entity_map
            .by_player_entity_id
            .contains_key(&cmd.player_entity_id)
        {
            bevy::log::info!(
                "bootstrap: player {} not in world; attempting deferred hydration",
                cmd.player_entity_id
            );
            let database_url = replication_database_url();
            let mut persistence = match GraphPersistence::connect(&database_url) {
                Ok(v) => v,
                Err(err) => {
                    bevy::log::error!("bootstrap deferred hydration failed (connect): {err}");
                    continue;
                }
            };
            let all_records = match persistence.load_graph_records() {
                Ok(v) => v,
                Err(err) => {
                    bevy::log::error!("bootstrap deferred hydration failed (load): {err}");
                    continue;
                }
            };
            let type_paths = component_type_path_map(&component_registry);
            let player_records =
                filter_records_for_player(&all_records, &cmd.player_entity_id, &type_paths);
            if player_records.is_empty() {
                bevy::log::warn!(
                    "bootstrap: player {} has no records in graph DB; skipping",
                    cmd.player_entity_id
                );
                continue;
            }
            let existing_guids = collect_existing_guids(&all_guids);
            hydrate_records_into_world(
                &mut commands,
                &player_records,
                &component_registry,
                &app_type_registry,
                &existing_guids,
                &mut player_entity_map,
                &mut controlled_entity_map,
            );
            bevy::log::info!(
                "bootstrap: deferred hydration complete for player {} ({} records)",
                cmd.player_entity_id,
                player_records.len()
            );
        }

        let Some(&player_entity) = player_entity_map
            .by_player_entity_id
            .get(&cmd.player_entity_id)
        else {
            bevy::log::warn!(
                "bootstrap: player entity {} still not found after deferred hydration; skipping",
                cmd.player_entity_id
            );
            continue;
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
                        cmd.player_entity_id
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
        {
            if let Ok(control_guid) = all_guids.get(controlled_entity) {
                commands
                    .entity(player_entity)
                    .insert(ControlledEntityGuid(Some(control_guid.0.to_string())));
            }
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
    client_remote_ids: Query<'_, '_, &'_ RemoteId, With<ClientOf>>,
    player_tags: Query<'_, '_, (), With<sidereal_game::PlayerTag>>,
) {
    for (client_entity, controlled_entity) in pending.bindings.drain(..) {
        let mut entity_commands = commands.entity(controlled_entity);
        entity_commands.insert(ControlledBy {
            owner: client_entity,
            lifetime: Lifetime::Persistent,
        });
        let is_player_anchor = player_tags.get(controlled_entity).is_ok();
        if let Ok(remote_id) = client_remote_ids.get(client_entity) {
            if is_player_anchor {
                entity_commands.insert(Replicate::to_clients(NetworkTarget::Single(remote_id.0)));
            } else {
                entity_commands.insert(Replicate::to_clients(NetworkTarget::All));
            }
            entity_commands.insert(PredictionTarget::to_clients(NetworkTarget::Single(
                remote_id.0,
            )));
            if is_player_anchor {
                entity_commands.remove::<InterpolationTarget>();
            } else {
                entity_commands.insert(InterpolationTarget::to_clients(
                    NetworkTarget::AllExceptSingle(remote_id.0),
                ));
            }
        } else {
            entity_commands.remove::<PredictionTarget>();
            if is_player_anchor {
                entity_commands.remove::<InterpolationTarget>();
            } else {
                entity_commands.insert(InterpolationTarget::to_clients(NetworkTarget::All));
            }
        }
    }
}

/// Syncs Avian Position/Rotation to Bevy Transform for physics entities (With<RigidBody>).
#[allow(clippy::type_complexity)]
pub fn sync_controlled_entity_transforms(
    mut entities: Query<'_, '_, (&'_ Position, &'_ Rotation, &'_ mut Transform), With<RigidBody>>,
) {
    for (position, rotation, mut transform) in &mut entities {
        let mut planar_position = position.0;
        if !planar_position.is_finite() {
            planar_position = Vec2::ZERO;
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
        transform.translation.x = planar_position.x;
        transform.translation.y = planar_position.y;
        transform.translation.z = 0.0;
        transform.rotation = Quat::from_rotation_z(heading);
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
            position.0 = Vec2::ZERO;
        }
        if !velocity.0.is_finite() {
            velocity.0 = Vec2::ZERO;
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
