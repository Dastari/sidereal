use avian3d::prelude::{AngularVelocity, LinearVelocity, Position, Rotation};
use bevy::ecs::reflect::AppTypeRegistry;
use bevy::math::EulerRot;
use bevy::prelude::*;
use sidereal_game::{
    AccountId, ControlledEntityGuid, Engine, EntityGuid, FlightComputer, FuelTank,
    GeneratedComponentRegistry, Hardpoint, HealthPool, Inventory, MassKg, MountedOn, OwnerId,
    PlayerTag, TotalMassKg,
};
use sidereal_persistence::{GraphEntityRecord, GraphPersistence};
use sidereal_runtime_sync::serialize_entity_components_to_graph_records;
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;
use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};
use std::thread;
use std::time::Duration;

use crate::replication::SimulatedControlledEntity;

#[derive(Debug)]
struct PersistenceWriteBatch {
    records: Vec<GraphEntityRecord>,
    tick: u64,
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

#[derive(Resource, Default)]
pub struct PersistenceSchemaInitState(pub bool);

#[derive(Resource)]
pub struct PersistenceDirtyState {
    pub initial_full_snapshot_pending: bool,
    pub dirty_entity_ids: HashSet<String>,
}

impl Default for PersistenceDirtyState {
    fn default() -> Self {
        Self {
            initial_full_snapshot_pending: true,
            dirty_entity_ids: HashSet::default(),
        }
    }
}

#[derive(Resource, Default)]
pub struct PersistenceWorkerState {
    sender: Option<SyncSender<PersistenceWriteBatch>>,
    latest_pending_batch: Option<PersistenceWriteBatch>,
    next_batch_tick: u64,
    enqueued_batches: u64,
    queue_full_events: u64,
    coalesced_replacements: u64,
    disconnected_events: u64,
    last_logged_at_s: f64,
}

pub fn start_persistence_worker(world: &mut World) {
    let queue_capacity = std::env::var("SIDEREAL_PERSIST_QUEUE_CAPACITY")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(4)
        .max(1);
    let database_url = std::env::var("REPLICATION_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://sidereal:sidereal@127.0.0.1:5432/sidereal".to_string());

    let (sender, receiver) = sync_channel::<PersistenceWriteBatch>(queue_capacity);
    let schema_initialized = world
        .get_resource::<PersistenceSchemaInitState>()
        .is_some_and(|state| state.0);
    thread::Builder::new()
        .name("replication-persistence-writer".to_string())
        .spawn(move || persistence_worker_loop(receiver, database_url, schema_initialized))
        .expect("failed to start replication persistence worker thread");

    let mut state = world.resource_mut::<PersistenceWorkerState>();
    state.sender = Some(sender);
    info!(
        "replication persistence worker started with queue_capacity={}",
        queue_capacity
    );
}

pub fn report_persistence_worker_metrics(
    time: Res<'_, Time>,
    mut state: ResMut<'_, PersistenceWorkerState>,
) {
    if !persistence_summary_logging_enabled() {
        return;
    }
    const LOG_INTERVAL_S: f64 = 5.0;
    let now = time.elapsed_secs_f64();
    if now - state.last_logged_at_s < LOG_INTERVAL_S {
        return;
    }
    state.last_logged_at_s = now;

    if state.enqueued_batches == 0
        && state.queue_full_events == 0
        && state.coalesced_replacements == 0
        && state.disconnected_events == 0
    {
        return;
    }

    info!(
        "replication persistence queue summary enqueued={} queue_full={} coalesced_replacements={} disconnected={} pending_latest={}",
        state.enqueued_batches,
        state.queue_full_events,
        state.coalesced_replacements,
        state.disconnected_events,
        state.latest_pending_batch.is_some(),
    );
}

fn persistence_summary_logging_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("SIDEREAL_REPLICATION_SUMMARY_LOGS")
            .is_ok_and(|v| v == "1" || v.eq_ignore_ascii_case("true"))
    })
}

fn mark_dirty_runtime_entity_id(
    dirty: &mut PersistenceDirtyState,
    identity: (
        &'_ EntityGuid,
        Option<&'_ SimulatedControlledEntity>,
        Option<&'_ PlayerTag>,
        Option<&'_ MountedOn>,
    ),
) {
    let (guid, simulated, player_tag, mounted_on) = identity;
    dirty.dirty_entity_ids.insert(runtime_entity_id_for(
        guid, simulated, player_tag, mounted_on,
    ));
}

#[allow(clippy::type_complexity)]
pub fn mark_dirty_persistable_entities_spatial(
    mut dirty: ResMut<'_, PersistenceDirtyState>,
    changed: Query<
        '_,
        '_,
        (
            &'_ EntityGuid,
            Option<&'_ SimulatedControlledEntity>,
            Option<&'_ PlayerTag>,
            Option<&'_ MountedOn>,
        ),
        Or<(
            Added<EntityGuid>,
            Added<SimulatedControlledEntity>,
            Added<PlayerTag>,
            Added<MountedOn>,
            Changed<Transform>,
            Changed<Position>,
            Changed<Rotation>,
            Changed<LinearVelocity>,
            Changed<AngularVelocity>,
        )>,
    >,
) {
    for identity in &changed {
        mark_dirty_runtime_entity_id(&mut dirty, identity);
    }
}

#[allow(clippy::type_complexity)]
pub fn mark_dirty_persistable_entities_runtime_state(
    mut dirty: ResMut<'_, PersistenceDirtyState>,
    changed: Query<
        '_,
        '_,
        (
            &'_ EntityGuid,
            Option<&'_ SimulatedControlledEntity>,
            Option<&'_ PlayerTag>,
            Option<&'_ MountedOn>,
        ),
        Or<(
            Changed<OwnerId>,
            Changed<AccountId>,
            Changed<ControlledEntityGuid>,
            Added<Hardpoint>,
        )>,
    >,
) {
    for identity in &changed {
        mark_dirty_runtime_entity_id(&mut dirty, identity);
    }
}

#[allow(clippy::type_complexity)]
pub fn mark_dirty_persistable_entities_modules(
    mut dirty: ResMut<'_, PersistenceDirtyState>,
    changed: Query<
        '_,
        '_,
        (
            &'_ EntityGuid,
            Option<&'_ SimulatedControlledEntity>,
            Option<&'_ PlayerTag>,
            Option<&'_ MountedOn>,
        ),
        Or<(
            Changed<MountedOn>,
            Changed<Engine>,
            Changed<FuelTank>,
            Changed<Inventory>,
            Changed<MassKg>,
        )>,
    >,
) {
    for identity in &changed {
        mark_dirty_runtime_entity_id(&mut dirty, identity);
    }
}

#[allow(clippy::type_complexity)]
pub fn mark_dirty_persistable_entities_gameplay(
    mut dirty: ResMut<'_, PersistenceDirtyState>,
    changed: Query<
        '_,
        '_,
        (
            &'_ EntityGuid,
            Option<&'_ SimulatedControlledEntity>,
            Option<&'_ PlayerTag>,
            Option<&'_ MountedOn>,
        ),
        Or<(
            Changed<FlightComputer>,
            Changed<HealthPool>,
            Changed<TotalMassKg>,
        )>,
    >,
) {
    for identity in &changed {
        mark_dirty_runtime_entity_id(&mut dirty, identity);
    }
}

/// Exclusive system: collects current simulation state for dirty persistable entities,
/// then enqueues writes to a dedicated worker thread.
///
/// This is entity-agnostic: any entity with EntityGuid is eligible. Current labels/properties
/// preserve compatibility for existing player/controlled/module/hardpoint hydration paths.
pub fn flush_simulation_state_persistence(world: &mut World) {
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

    let (persist_all, dirty_entity_ids) = {
        let mut dirty = world.resource_mut::<PersistenceDirtyState>();
        let persist_all = dirty.initial_full_snapshot_pending;
        let dirty_entity_ids = std::mem::take(&mut dirty.dirty_entity_ids);
        (persist_all, dirty_entity_ids)
    };

    if !persist_all && dirty_entity_ids.is_empty() {
        return;
    }

    let mut entity_id_by_guid = HashMap::<uuid::Uuid, String>::new();
    {
        let mut id_query = world.query::<(
            &'_ EntityGuid,
            Option<&'_ SimulatedControlledEntity>,
            Option<&'_ PlayerTag>,
            Option<&'_ MountedOn>,
        )>();
        for (guid, simulated, player_tag, mounted_on) in id_query.iter(world) {
            entity_id_by_guid.insert(
                guid.0,
                runtime_entity_id_for(guid, simulated, player_tag, mounted_on),
            );
        }
    }

    let mut records = Vec::<GraphEntityRecord>::new();
    let mut entity_query = world.query::<(
        Entity,
        &'_ EntityGuid,
        Option<&'_ SimulatedControlledEntity>,
        Option<&'_ PlayerTag>,
        Option<&'_ MountedOn>,
        Option<&'_ Hardpoint>,
        Option<&'_ AccountId>,
        Option<&'_ OwnerId>,
        Option<&'_ Transform>,
        Option<&'_ Position>,
        Option<&'_ Rotation>,
        Option<&'_ LinearVelocity>,
    )>();

    for (
        entity,
        guid,
        simulated,
        player_tag,
        mounted_on,
        hardpoint,
        account_id,
        owner_id,
        transform,
        position,
        rotation,
        velocity,
    ) in entity_query.iter(world)
    {
        let entity_id = runtime_entity_id_for(guid, simulated, player_tag, mounted_on);
        if !persist_all && !dirty_entity_ids.contains(&entity_id) {
            continue;
        }

        let labels = runtime_labels_for(simulated, player_tag, mounted_on, hardpoint);
        let mut properties = serde_json::Map::<String, serde_json::Value>::new();

        if let Some(simulated) = simulated {
            properties.insert(
                "player_entity_id".to_string(),
                serde_json::Value::String(simulated.player_entity_id.clone()),
            );
            let mut pos = position.map(|p| p.0).unwrap_or_else(|| Vec3::ZERO);
            if !pos.is_finite() {
                pos = Vec3::ZERO;
            }
            pos.z = 0.0;
            properties.insert(
                "position_m".to_string(),
                serde_json::json!([pos.x, pos.y, 0.0]),
            );

            let mut vel = velocity.map(|v| v.0).unwrap_or_else(|| Vec3::ZERO);
            if !vel.is_finite() {
                vel = Vec3::ZERO;
            }
            vel.z = 0.0;
            properties.insert(
                "velocity_mps".to_string(),
                serde_json::json!([vel.x, vel.y, 0.0]),
            );

            let heading_rad = if let Some(rotation) = rotation {
                if rotation.0.is_finite() {
                    let h = rotation.0.to_euler(EulerRot::ZYX).0;
                    if h.is_finite() { h } else { 0.0 }
                } else {
                    0.0
                }
            } else {
                0.0
            };
            properties.insert("heading_rad".to_string(), serde_json::json!(heading_rad));
        }

        if player_tag.is_some() {
            if let Some(account_id) = account_id {
                properties.insert(
                    "owner_account_id".to_string(),
                    serde_json::Value::String(account_id.0.clone()),
                );
            }
            properties.insert(
                "player_entity_id".to_string(),
                serde_json::Value::String(entity_id.clone()),
            );
            let camera = transform.map(|t| t.translation).unwrap_or(Vec3::ZERO);
            properties.insert(
                "position_m".to_string(),
                serde_json::json!([camera.x, camera.y, camera.z]),
            );
        }

        if let Some(owner_id) = owner_id {
            properties.insert(
                "owner_id".to_string(),
                serde_json::Value::String(owner_id.0.clone()),
            );
        }

        if let Some(mounted_on) = mounted_on {
            let parent_entity_id = entity_id_by_guid
                .get(&mounted_on.parent_entity_id)
                .cloned()
                .unwrap_or_else(|| format!("entity:{}", mounted_on.parent_entity_id));
            properties.insert(
                "parent_entity_id".to_string(),
                serde_json::Value::String(parent_entity_id),
            );
            properties.insert(
                "hardpoint_id".to_string(),
                serde_json::Value::String(mounted_on.hardpoint_id.clone()),
            );
        }

        if let Some(hardpoint) = hardpoint {
            properties.insert(
                "hardpoint_id".to_string(),
                serde_json::Value::String(hardpoint.hardpoint_id.clone()),
            );
            properties.insert(
                "hardpoint_offset_m".to_string(),
                serde_json::json!([
                    hardpoint.offset_m.x,
                    hardpoint.offset_m.y,
                    hardpoint.offset_m.z
                ]),
            );
        }

        let entity_ref = world.entity(entity);
        let components = serialize_entity_components_to_graph_records(
            &entity_id,
            entity_ref,
            &component_registry,
            &app_type_registry,
        );

        records.push(GraphEntityRecord {
            entity_id,
            labels,
            properties: serde_json::Value::Object(properties),
            components,
        });
    }

    if records.is_empty() {
        return;
    }

    let mut worker_state = world.resource_mut::<PersistenceWorkerState>();
    let tick = worker_state.next_batch_tick;
    worker_state.next_batch_tick = worker_state.next_batch_tick.saturating_add(1);
    let batch = PersistenceWriteBatch { records, tick };
    enqueue_batch(&mut worker_state, batch);

    if persist_all {
        world
            .resource_mut::<PersistenceDirtyState>()
            .initial_full_snapshot_pending = false;
    }
}

fn runtime_entity_id_for(
    guid: &EntityGuid,
    simulated: Option<&SimulatedControlledEntity>,
    player_tag: Option<&PlayerTag>,
    mounted_on: Option<&MountedOn>,
) -> String {
    if let Some(simulated) = simulated {
        return simulated.entity_id.clone();
    }
    if player_tag.is_some() {
        return format!("player:{}", guid.0);
    }
    if mounted_on.is_some() {
        return format!("module:{}", guid.0);
    }
    format!("entity:{}", guid.0)
}

fn runtime_labels_for(
    simulated: Option<&SimulatedControlledEntity>,
    player_tag: Option<&PlayerTag>,
    mounted_on: Option<&MountedOn>,
    hardpoint: Option<&Hardpoint>,
) -> Vec<String> {
    let mut labels = vec!["Entity".to_string()];
    if player_tag.is_some() {
        labels.push("Player".to_string());
    }
    if simulated.is_some() {
        // Compatibility label for existing controlled-entity hydration flow.
        labels.push("Ship".to_string());
    }
    if mounted_on.is_some() {
        labels.push("Module".to_string());
    }
    if hardpoint.is_some() {
        labels.push("Hardpoint".to_string());
    }
    labels
}

fn enqueue_batch(state: &mut PersistenceWorkerState, batch: PersistenceWriteBatch) {
    let Some(sender) = state.sender.as_ref() else {
        state.disconnected_events = state.disconnected_events.saturating_add(1);
        return;
    };

    if let Some(pending) = state.latest_pending_batch.take() {
        match sender.try_send(pending) {
            Ok(()) => {
                state.enqueued_batches = state.enqueued_batches.saturating_add(1);
            }
            Err(TrySendError::Full(pending)) => {
                state.latest_pending_batch = Some(pending);
            }
            Err(TrySendError::Disconnected(pending)) => {
                state.latest_pending_batch = Some(pending);
                state.sender = None;
                state.disconnected_events = state.disconnected_events.saturating_add(1);
                return;
            }
        }
    }

    match sender.try_send(batch) {
        Ok(()) => {
            state.enqueued_batches = state.enqueued_batches.saturating_add(1);
        }
        Err(TrySendError::Full(batch)) => {
            state.queue_full_events = state.queue_full_events.saturating_add(1);
            if state.latest_pending_batch.replace(batch).is_some() {
                state.coalesced_replacements = state.coalesced_replacements.saturating_add(1);
            }
        }
        Err(TrySendError::Disconnected(batch)) => {
            state.latest_pending_batch = Some(batch);
            state.sender = None;
            state.disconnected_events = state.disconnected_events.saturating_add(1);
        }
    }
}

fn persistence_worker_loop(
    receiver: Receiver<PersistenceWriteBatch>,
    database_url: String,
    mut schema_initialized: bool,
) {
    let mut persistence = connect_persistence_with_retry(&database_url, &mut schema_initialized);

    while let Ok(batch) = receiver.recv() {
        if batch.records.is_empty() {
            continue;
        }

        let record_count = batch.records.len();
        let mut pending = Some(batch);
        loop {
            let batch = pending
                .take()
                .expect("pending persistence batch should be present");
            match persistence.persist_graph_records(&batch.records, batch.tick) {
                Ok(()) => {
                    if persistence_summary_logging_enabled() {
                        info!(
                            "persisted simulation state for {} entities (tick={})",
                            record_count, batch.tick
                        );
                    }
                    break;
                }
                Err(err) => {
                    eprintln!("persistence worker write failed: {err}; reconnecting");
                    pending = Some(batch);
                    thread::sleep(Duration::from_millis(250));
                    persistence =
                        connect_persistence_with_retry(&database_url, &mut schema_initialized);
                }
            }
        }
    }

    info!("replication persistence worker exiting: sender disconnected");
}

fn connect_persistence_with_retry(
    database_url: &str,
    schema_initialized: &mut bool,
) -> GraphPersistence {
    loop {
        match GraphPersistence::connect(database_url) {
            Ok(mut persistence) => {
                if !*schema_initialized {
                    match persistence.ensure_schema() {
                        Ok(()) => {
                            *schema_initialized = true;
                        }
                        Err(err) => {
                            eprintln!(
                                "persistence worker schema initialization failed: {err}; retrying"
                            );
                            thread::sleep(Duration::from_secs(1));
                            continue;
                        }
                    }
                }
                return persistence;
            }
            Err(err) => {
                eprintln!("persistence worker connect failed: {err}; retrying");
            }
        }
        thread::sleep(Duration::from_secs(1));
    }
}
