use bevy::ecs::reflect::AppTypeRegistry;
use bevy::prelude::*;
use sidereal_game::{EntityGuid, EntityLabels, GeneratedComponentRegistry, MountedOn};
use sidereal_persistence::{GraphEntityRecord, GraphPersistence};
use sidereal_runtime_sync::serialize_entity_components_to_graph_records;
use std::collections::HashSet;
use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};
use std::thread;
use std::time::Duration;

use crate::replication::debug_env;

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

pub fn init_resources(app: &mut App) {
    app.insert_resource(PersistenceWorkerState::default());
    app.insert_resource(PersistenceDirtyState::default());
    app.insert_resource(PersistenceSchemaInitState::default());
    app.insert_resource(SimulationPersistenceTimer::default());
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
    debug_env("SIDEREAL_REPLICATION_SUMMARY_LOGS")
}

fn mark_dirty_entity(dirty: &mut PersistenceDirtyState, guid: &EntityGuid) {
    dirty.dirty_entity_ids.insert(guid.0.to_string());
}

/// Marks any entity whose registered components changed as dirty for persistence.
/// This is intentionally broad: any `EntityGuid`-bearing entity with a changed
/// component triggers a persistence pass for that entity.
pub fn mark_dirty_persistable_entities(
    mut dirty: ResMut<'_, PersistenceDirtyState>,
    changed: Query<'_, '_, &'_ EntityGuid, Changed<EntityGuid>>,
) {
    for guid in &changed {
        mark_dirty_entity(&mut dirty, guid);
    }
}

/// Catches spatial/physics component changes on persistable entities.
#[allow(clippy::type_complexity)]
pub fn mark_dirty_persistable_entities_spatial(
    mut dirty: ResMut<'_, PersistenceDirtyState>,
    changed: Query<
        '_,
        '_,
        &'_ EntityGuid,
        Or<(
            Added<EntityGuid>,
            Changed<Transform>,
            Changed<avian2d::prelude::Position>,
            Changed<avian2d::prelude::Rotation>,
            Changed<avian2d::prelude::LinearVelocity>,
            Changed<avian2d::prelude::AngularVelocity>,
        )>,
    >,
) {
    for guid in &changed {
        mark_dirty_entity(&mut dirty, guid);
    }
}

/// Catches gameplay/module component changes on persistable entities.
#[allow(clippy::type_complexity)]
pub fn mark_dirty_persistable_entities_components(
    mut dirty: ResMut<'_, PersistenceDirtyState>,
    changed: Query<
        '_,
        '_,
        &'_ EntityGuid,
        Or<(
            Changed<MountedOn>,
            Changed<sidereal_game::OwnerId>,
            Changed<sidereal_game::AccountId>,
            Changed<sidereal_game::ControlledEntityGuid>,
            Changed<sidereal_game::Hardpoint>,
            Changed<sidereal_game::Engine>,
            Changed<sidereal_game::FuelTank>,
            Changed<sidereal_game::Inventory>,
            Changed<sidereal_game::MassKg>,
            Changed<sidereal_game::FlightComputer>,
            Changed<sidereal_game::HealthPool>,
            Changed<sidereal_game::TotalMassKg>,
            Changed<sidereal_game::PlayerTag>,
        )>,
    >,
) {
    for guid in &changed {
        mark_dirty_entity(&mut dirty, guid);
    }
}

/// Exclusive system: collects current simulation state for dirty persistable entities,
/// then enqueues writes to a dedicated worker thread.
///
/// Fully entity-type-agnostic: any entity with EntityGuid is persisted. Labels come
/// from EntityLabels component. All component data (spatial, gameplay, physics) flows
/// through the generic component registry. Entity-level properties contain only
/// structural metadata (parent_entity_id for graph relationship traversal).
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

    let mut records = Vec::<GraphEntityRecord>::new();
    let mut entity_query = world.query::<(
        Entity,
        &'_ EntityGuid,
        Option<&'_ EntityLabels>,
        Option<&'_ MountedOn>,
    )>();

    for (entity, guid, entity_labels, mounted_on) in entity_query.iter(world) {
        let entity_id = guid.0.to_string();
        if !persist_all && !dirty_entity_ids.contains(&entity_id) {
            continue;
        }

        let mut labels = vec!["Entity".to_string()];
        if let Some(el) = entity_labels {
            for label in &el.0 {
                if label != "Entity" {
                    labels.push(label.clone());
                }
            }
        }

        let mut properties = serde_json::Map::<String, serde_json::Value>::new();

        if let Some(mounted_on) = mounted_on {
            properties.insert(
                "parent_entity_id".to_string(),
                serde_json::Value::String(mounted_on.parent_entity_id.to_string()),
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
