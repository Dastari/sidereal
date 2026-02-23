use avian3d::prelude::{LinearVelocity, Position, Rotation};
use bevy::ecs::reflect::AppTypeRegistry;
use bevy::math::EulerRot;
use bevy::prelude::*;
use sidereal_game::{EntityGuid, GeneratedComponentRegistry, MountedOn};
use sidereal_persistence::GraphEntityRecord;
use sidereal_runtime_sync::serialize_entity_components_to_graph_records;

use crate::replication::SimulatedControlledEntity;
use crate::{PlayerRuntimeViewDirtySet, PlayerRuntimeViewRegistry, ReplicationRuntime};

pub fn flush_player_runtime_view_state_persistence(
    runtime: Option<NonSendMut<'_, ReplicationRuntime>>,
    view_registry: Res<'_, PlayerRuntimeViewRegistry>,
    mut dirty_view_states: ResMut<'_, PlayerRuntimeViewDirtySet>,
) {
    let Some(mut runtime) = runtime else {
        return;
    };
    if dirty_view_states.player_entity_ids.is_empty() {
        return;
    }

    let pending_player_ids = dirty_view_states
        .player_entity_ids
        .iter()
        .cloned()
        .collect::<Vec<_>>();
    let mut persisted = Vec::<String>::new();
    for player_entity_id in pending_player_ids {
        let Some(view_state) = view_registry.by_player_entity_id.get(&player_entity_id) else {
            persisted.push(player_entity_id);
            continue;
        };
        match runtime.persistence.upsert_player_view_state(view_state) {
            Ok(()) => persisted.push(player_entity_id),
            Err(err) => eprintln!("replication failed persisting player view state: {err}"),
        }
    }
    for player_entity_id in persisted {
        dirty_view_states
            .player_entity_ids
            .remove(&player_entity_id);
    }
}

/// Tick counter for throttling simulation state persistence.
#[derive(Resource)]
pub struct SimulationPersistenceTimer {
    pub interval_ticks: u32,
    pub current_tick: u32,
}

impl Default for SimulationPersistenceTimer {
    fn default() -> Self {
        let interval = std::env::var("SIDEREAL_PERSIST_INTERVAL_TICKS")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(300); // 300 ticks @ 30Hz = 10 seconds
        Self {
            interval_ticks: interval,
            current_tick: 0,
        }
    }
}

/// Exclusive system: collects current simulation state for all controlled entities
/// and their modules, then persists to the graph database on a throttled interval.
///
/// Must be exclusive because `serialize_entity_components_to_graph_records` requires
/// `EntityRef` (immutable world access) while we also need mutable resource access
/// for the timer and persistence runtime.
pub fn flush_simulation_state_persistence(world: &mut World) {
    // Tick guard — bump counter and early-out if not yet time to persist.
    {
        let mut timer = world.resource_mut::<SimulationPersistenceTimer>();
        timer.current_tick += 1;
        if timer.current_tick < timer.interval_ticks {
            return;
        }
        timer.current_tick = 0;
    }

    let component_registry = world.resource::<GeneratedComponentRegistry>().clone();
    let app_type_registry = world.resource::<AppTypeRegistry>().clone();

    // Collect ship data from queries (read-only world borrow).
    let mut ship_data: Vec<(Entity, String, String, Vec3, Vec3, f32)> = Vec::new();
    let mut ship_guids = std::collections::HashSet::<uuid::Uuid>::new();

    let mut ship_query = world.query::<(
        Entity,
        &SimulatedControlledEntity,
        &Position,
        &Rotation,
        &LinearVelocity,
        &EntityGuid,
    )>();

    for (entity, sim, position, rotation, velocity, guid) in ship_query.iter(world) {
        let mut pos = position.0;
        if !pos.is_finite() {
            pos = Vec3::ZERO;
        }
        pos.z = 0.0;

        let mut vel = velocity.0;
        if !vel.is_finite() {
            vel = Vec3::ZERO;
        }
        vel.z = 0.0;

        let heading_rad = if rotation.0.is_finite() {
            let h = rotation.0.to_euler(EulerRot::ZYX).0;
            if h.is_finite() { h } else { 0.0 }
        } else {
            0.0
        };

        ship_data.push((
            entity,
            sim.entity_id.clone(),
            sim.player_entity_id.clone(),
            pos,
            vel,
            heading_rad,
        ));
        ship_guids.insert(guid.0);
    }

    let mut module_data: Vec<(Entity, uuid::Uuid, String, String)> = Vec::new();
    let mut module_query = world.query::<(Entity, &EntityGuid, &MountedOn)>();
    for (entity, guid, mounted_on) in module_query.iter(world) {
        if !ship_guids.contains(&mounted_on.parent_entity_id) {
            continue;
        }
        module_data.push((
            entity,
            guid.0,
            format!("ship:{}", mounted_on.parent_entity_id),
            mounted_on.hardpoint_id.clone(),
        ));
    }

    // Now serialize components using EntityRef (requires immutable world access only).
    let mut records = Vec::new();

    for (entity, entity_id, player_entity_id, pos, vel, heading_rad) in &ship_data {
        let entity_ref = world.entity(*entity);
        let components = serialize_entity_components_to_graph_records(
            entity_id,
            entity_ref,
            &component_registry,
            &app_type_registry,
        );

        records.push(GraphEntityRecord {
            entity_id: entity_id.clone(),
            labels: vec!["Entity".to_string(), "Ship".to_string()],
            properties: serde_json::json!({
                "player_entity_id": player_entity_id,
                "position_m": [pos.x, pos.y, 0.0],
                "velocity_mps": [vel.x, vel.y, 0.0],
                "heading_rad": heading_rad,
            }),
            components,
        });
    }

    for (entity, guid, ship_entity_id, hardpoint_id) in &module_data {
        let module_entity_id = format!("module:{}", guid);
        let entity_ref = world.entity(*entity);
        let components = serialize_entity_components_to_graph_records(
            &module_entity_id,
            entity_ref,
            &component_registry,
            &app_type_registry,
        );

        records.push(GraphEntityRecord {
            entity_id: module_entity_id,
            labels: vec!["Entity".to_string(), "Module".to_string()],
            properties: serde_json::json!({
                "parent_entity_id": ship_entity_id,
                "hardpoint_id": hardpoint_id,
            }),
            components,
        });
    }

    if records.is_empty() {
        return;
    }

    // Mutable access to persistence runtime for the DB write.
    let Some(mut runtime) = world.get_non_send_resource_mut::<ReplicationRuntime>() else {
        return;
    };
    let record_count = records.len();
    match runtime.persistence.persist_graph_records(&records, 0) {
        Ok(()) => {
            info!("persisted simulation state for {} entities", record_count);
        }
        Err(err) => {
            eprintln!("failed to persist simulation state: {err}");
        }
    }
}
