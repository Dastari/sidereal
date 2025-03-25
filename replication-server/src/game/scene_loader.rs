use crate::database::{DatabaseClient, EntityRecord};
use sidereal::serialization::update_entity;
use sidereal::ecs::components::id::Id;
use bevy::prelude::*;
use std::sync::Arc;
use tracing::{debug, error, info, warn};
use serde_json;
/// Plugin for loading the game scene from the database
pub struct SceneLoaderPlugin;

impl Plugin for SceneLoaderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SceneLoadingState>()
            .init_state::<SceneState>()
            .add_systems(OnEnter(SceneState::Connecting), setup)
            .add_systems(
                Update,
                check_db_connection.run_if(in_state(SceneState::Connecting)),
            )
            .add_systems(OnEnter(SceneState::Loading), load_entities_system)
            .add_systems(
                Update,
                process_loaded_entities.run_if(in_state(SceneState::Processing)),
            )
            .add_systems(
                Update,
                (
                    batch_processing,
                    process_deserialization_tasks,
                ).run_if(in_state(SceneState::Ready)),
            )
            // Add apply_pending_deserializations as a separate system that runs after other systems
            .add_systems(PostUpdate, apply_pending_deserializations.run_if(in_state(SceneState::Ready)));
    }
}

/// State of the scene loading process
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SceneState {
    #[default]
    Connecting,
    Loading,
    Processing,
    Ready,
    Completed,
    Error,
}

/// Resource holding the state of scene loading
#[derive(Resource)]
pub struct SceneLoadingState {
    pub loaded_entities: Vec<EntityRecord>,
    pub _error_message: Option<String>,
    pub batch_size: usize,
    pub current_batch_index: usize,
    pub total_entities: usize,
    pub is_processing_batch: bool,
    pub current_batch: Vec<usize>,
    pub pending_deserializations: Vec<String>,
}

impl Default for SceneLoadingState {
    fn default() -> Self {
        Self {
            loaded_entities: Vec::new(),
            _error_message: None,
            batch_size: 100,
            current_batch_index: 0,
            total_entities: 0,
            is_processing_batch: false,
            current_batch: Vec::new(),
            pending_deserializations: Vec::new(),
        }
    }
}

/// Component to hold the async task for loading entities
#[derive(Component)]
struct AsyncTask(Arc<std::sync::Mutex<Option<Vec<EntityRecord>>>>);

/// Setup system for scene loading
fn setup(_commands: Commands) {
    // Any initial setup needed before connecting to the database
}

fn check_db_connection(
    mut commands: Commands,
    mut scene_state: ResMut<NextState<SceneState>>,
    db_client: Option<Res<DatabaseClient>>,
) {
    if db_client.is_some() {
        scene_state.set(SceneState::Loading);
        return;
    }

    match DatabaseClient::new() {
        Ok(client) => {
            info!("Connected to database");
            commands.insert_resource(client);
            commands.insert_resource(SceneLoadingState::default());
            scene_state.set(SceneState::Loading);
        }
        Err(e) => {
            error!("Failed to connect to database: {}", e);
            scene_state.set(SceneState::Error);
        }
    }
}

fn load_entities_system(
    mut commands: Commands,
    mut scene_state: ResMut<NextState<SceneState>>,
    query: Query<Entity, With<AsyncTask>>,
) {
    // Only start one task at a time
    if !query.is_empty() {
        return;
    }

    info!("Starting to load entities from database");
    let task_result = Arc::new(std::sync::Mutex::new(None));
    let task_result_clone = task_result.clone();

    std::thread::spawn(move || {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let result = runtime.block_on(async {
            match DatabaseClient::new() {
                Ok(client) => match client.fetch_all_entities().await {
                    Ok(entities) => {
                        info!("Loaded {} entities from database", entities.len());
                        Some(entities)
                    }
                    Err(e) => {
                        error!("Failed to load entities: {}", e);
                        None
                    }
                },
                Err(e) => {
                    error!("Failed to create database client in async task: {}", e);
                    None
                }
            }
        });

        if let Some(entities) = result {
            let mut task_data = task_result_clone.lock().unwrap();
            *task_data = Some(entities);
        }
    });

    commands.spawn((
        AsyncTask(task_result),
        bevy::core::Name::new("Entity Loader Task"),
    ));

    scene_state.set(SceneState::Processing);
}

fn process_loaded_entities(
    mut commands: Commands,
    task_query: Query<(Entity, &AsyncTask)>,
    mut scene_state: ResMut<NextState<SceneState>>,
    mut scene_loading_state: ResMut<SceneLoadingState>,
) {
    for (task_entity, task) in task_query.iter() {
        let has_entities = {
            let lock = task.0.lock().unwrap();
            lock.is_some()
        };

        if has_entities {
            let entities = {
                let mut lock = task.0.lock().unwrap();
                lock.take().unwrap()
            };

            info!("Processing {} loaded entities", entities.len());
            scene_loading_state.loaded_entities = entities;
            scene_loading_state.total_entities = scene_loading_state.loaded_entities.len();
            scene_loading_state.current_batch_index = 0;
            scene_loading_state.is_processing_batch = false;

            scene_state.set(SceneState::Ready);
            commands.entity(task_entity).despawn();
        }
    }
}

/// Handle batch processing of loaded entities
fn batch_processing(
    mut scene_loading_state: ResMut<SceneLoadingState>,
    mut scene_state: ResMut<NextState<SceneState>>,
) {
    // Skip if we're already processing a batch
    if scene_loading_state.is_processing_batch {
        return;
    }

    // Check if we have processed all entities
    if scene_loading_state.current_batch_index >= scene_loading_state.total_entities {
        if scene_loading_state.total_entities > 0 {
            info!("Finished processing all {} entities - moving to Completed state", scene_loading_state.total_entities);
            scene_state.set(SceneState::Completed);
        }
        return;
    }

    let start_idx = scene_loading_state.current_batch_index;
    let end_idx = (start_idx + scene_loading_state.batch_size).min(scene_loading_state.total_entities);
    let batch_size = end_idx - start_idx;

    info!("Processing batch of {} entities ({}-{})", batch_size, start_idx, end_idx);
    
    // Mark that we're processing a batch
    scene_loading_state.is_processing_batch = true;
    scene_loading_state.current_batch.clear();

    // Just store indices for this batch
    for i in start_idx..end_idx {
        scene_loading_state.current_batch.push(i);
    }

    // Update batch index for next iteration
    scene_loading_state.current_batch_index = end_idx;
}

/// System for processing the deserialization of entities in batches
fn process_deserialization_tasks(
    mut scene_loading_state: ResMut<SceneLoadingState>,
) {
    // Skip if not processing a batch or batch is empty
    if !scene_loading_state.is_processing_batch || scene_loading_state.current_batch.is_empty() {
        return;
    }

    // Track how many entities were processed
    let mut success_count = 0;
    let mut error_count = 0;
    
    // Process each entity in the current batch
    let indices_to_process = scene_loading_state.current_batch.clone();
    for idx in indices_to_process {
        if let Some(record) = scene_loading_state.loaded_entities.get(idx) {
            match serde_json::to_string(&record.components) {
                Ok(json) => {
                    // Store JSON in the resource instead of creating an entity
                    scene_loading_state.pending_deserializations.push(json);
                    success_count += 1;
                }
                Err(e) => {
                    error!("Failed to serialize components to JSON: {}", e);
                    error_count += 1;
                }
            }
        } else {
            error!("Invalid entity index: {}", idx);
            error_count += 1;
        }
    }
    
    // Log summary and mark batch as complete
    if success_count > 0 || error_count > 0 {
        info!("Batch processed: {} successful, {} failed", success_count, error_count);
    }
    
    scene_loading_state.is_processing_batch = false;
    scene_loading_state.current_batch.clear();
}

// System to apply pending deserializations
fn apply_pending_deserializations(world: &mut World) {
    let mut scene_loading_state = world.resource_mut::<SceneLoadingState>();
    let pending = std::mem::take(&mut scene_loading_state.pending_deserializations);
    
    // Process each serialized entity
    for json in pending {
        match update_entity(&json, world) {
            Ok(_) => {
                debug!("Successfully deserialized entity");
            }
            Err(e) => {
                error!("Failed to deserialize entity: {}", e);
            }
        }
    }
}

/// Apply deferred function is no longer needed
fn apply_deferred(_world: &mut World) {
    // Empty placeholder - this can be removed in future refactoring
}
