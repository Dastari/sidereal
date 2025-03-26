use crate::database::{DatabaseClient, EntityRecord};
use bevy::prelude::*;
use serde_json;
use sidereal::serialization::update_entity; // Assuming this function still requires &str
use std::sync::{Arc, Mutex};
use tokio::runtime::Runtime; // Explicit import
use tracing::{debug, error, info, warn};

/// Plugin for loading the game scene from the database
pub struct SceneLoaderPlugin;

impl Plugin for SceneLoaderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SceneLoadingState>()
            .init_state::<SceneState>()
            // Removed empty 'setup' system
            .add_systems(
                Update,
                check_db_connection.run_if(in_state(SceneState::Connecting)),
            )
            .add_systems(OnEnter(SceneState::Loading), load_entities_system)
            .add_systems(
                Update,
                process_loaded_entities.run_if(in_state(SceneState::Processing)),
            )
            // Merged batching and processing into one system
            .add_systems(
                Update,
                process_entity_batch.run_if(in_state(SceneState::Ready)),
            )
            // Apply deserializations in PostUpdate
            .add_systems(
                PostUpdate,
                apply_pending_deserializations.run_if(in_state(SceneState::Ready)),
            );
        // Removed apply_deferred system
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
    pub batch_size: usize,
    pub current_batch_index: usize,
    pub total_entities: usize,
    // Stores JSON strings ready for update_entity
    pub pending_deserializations: Vec<String>,
    // Removed _error_message, is_processing_batch, current_batch
}

// Explicit Default implementation (though derive would work too)
impl Default for SceneLoadingState {
    fn default() -> Self {
        Self {
            loaded_entities: Vec::new(),
            batch_size: 100, // Default batch size
            current_batch_index: 0,
            total_entities: 0,
            pending_deserializations: Vec::new(),
        }
    }
}

/// Resource to track the background entity loading task result
#[derive(Resource)]
struct EntityLoadTask(Arc<Mutex<Option<Result<Vec<EntityRecord>, String>>>>);

fn check_db_connection(
    mut commands: Commands,
    mut scene_state: ResMut<NextState<SceneState>>,
    db_client: Option<Res<DatabaseClient>>,
) {
    // If client already exists, we are connected (or connection assumed valid)
    if db_client.is_some() {
        info!("Database client already exists. Proceeding to Loading state.");
        scene_state.set(SceneState::Loading);
        return;
    }

    // Attempt to create and insert the client resource
    info!("Attempting to connect to database...");
    match DatabaseClient::new() {
        Ok(client) => {
            info!("Connected to database and inserted client resource.");
            commands.insert_resource(client);
            // SceneLoadingState is already initialized via init_resource
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
    // Check if a task resource already exists
    existing_task: Option<Res<EntityLoadTask>>,
) {
    // Only start one task at a time
    if existing_task.is_some() {
        warn!("Entity loading task already in progress.");
        return;
    }

    info!("Starting background task to load entities from database.");
    let task_result_arc = Arc::new(Mutex::new(None));
    let task_result_clone = task_result_arc.clone();

    // Spawn a standard OS thread to run the async code
    std::thread::spawn(move || {
        // Create a Tokio runtime within the new thread
        let runtime = match Runtime::new() {
            Ok(rt) => rt,
            Err(e) => {
                error!("Failed to create Tokio runtime in background thread: {}", e);
                let mut task_data = task_result_clone.lock().unwrap();
                *task_data = Some(Err(format!("Tokio runtime creation failed: {}", e)));
                return;
            }
        };

        // Block on the async database operation
        let result = runtime.block_on(async {
            match DatabaseClient::new() {
                // Creates its own client instance
                Ok(client) => match client.fetch_all_entities().await {
                    Ok(entities) => {
                        info!(
                            "Loaded {} entities from database in background task",
                            entities.len()
                        );
                        Ok(entities)
                    }
                    Err(e) => {
                        error!("Failed to load entities in background task: {}", e);
                        Err(format!("Failed to load entities: {}", e))
                    }
                },
                Err(e) => {
                    error!("Failed to create database client in background task: {}", e);
                    Err(format!("DB client creation failed in task: {}", e))
                }
            }
        });

        // Store the result (Ok or Err) in the Arc<Mutex>
        let mut task_data = task_result_clone.lock().unwrap();
        *task_data = Some(result);
        info!("Background entity loading task finished.");
    });

    // Insert the resource to track the task
    commands.insert_resource(EntityLoadTask(task_result_arc));
    // Move to processing state to wait for the result
    scene_state.set(SceneState::Processing);
}

fn process_loaded_entities(
    mut commands: Commands,
    // Use Option<Res<...>> to check if the task resource exists
    task_resource: Option<ResMut<EntityLoadTask>>,
    mut scene_state: ResMut<NextState<SceneState>>,
    mut scene_loading_state: ResMut<SceneLoadingState>,
) {
    if let Some(task) = task_resource {
        let mut task_data_lock = task.0.lock().unwrap();

        // Check if the task has finished and placed data (Some)
        if let Some(result) = task_data_lock.take() {
            // take() consumes the Some value
            match result {
                Ok(entities) => {
                    info!(
                        "Processing {} loaded entities from background task.",
                        entities.len()
                    );
                    scene_loading_state.loaded_entities = entities;
                    scene_loading_state.total_entities = scene_loading_state.loaded_entities.len();
                    scene_loading_state.current_batch_index = 0;
                    scene_state.set(SceneState::Ready); // Move to Ready state for batch processing
                }
                Err(e) => {
                    error!("Entity loading task failed: {}", e);
                    scene_loading_state.loaded_entities.clear(); // Ensure clean state
                    scene_loading_state.total_entities = 0;
                    scene_state.set(SceneState::Error); // Move to Error state
                }
            }
            // Task finished, remove the resource
            commands.remove_resource::<EntityLoadTask>();
        }
        // else: Task data not ready yet, do nothing this frame
    }
    // else: Task resource doesn't exist (shouldn't happen in Processing state if load_entities_system ran)
}

/// Processes the next batch of loaded entities: calculates range, serializes, adds to pending list.
fn process_entity_batch(
    mut scene_loading_state: ResMut<SceneLoadingState>,
    mut scene_state: ResMut<NextState<SceneState>>,
) {
    // Check if all entities have been processed
    if scene_loading_state.current_batch_index >= scene_loading_state.total_entities {
        if scene_loading_state.total_entities > 0 {
            info!(
                "Finished processing all {} entities. Moving to Completed state.",
                scene_loading_state.total_entities
            );
            scene_state.set(SceneState::Completed);
        } else {
            // If total_entities is 0, also consider it completed.
            info!("No entities to process. Moving to Completed state.");
            scene_state.set(SceneState::Completed);
        }
        return; // Don't process if completed or nothing to process
    }

    // Calculate the range for the current batch
    let start_idx = scene_loading_state.current_batch_index;
    let end_idx =
        (start_idx + scene_loading_state.batch_size).min(scene_loading_state.total_entities);
    let batch_actual_size = end_idx - start_idx;

    if batch_actual_size == 0 {
        // This case should ideally not be reached if the completion check above is correct,
        // but adding it defensively.
        debug!("Batch size is zero, skipping processing.");
        // Update index just in case, to avoid infinite loops if logic is flawed
        scene_loading_state.current_batch_index = end_idx;
        return;
    }

    info!(
        "Processing batch of {} entities (indices {}-{})...",
        batch_actual_size,
        start_idx,
        end_idx - 1 // Use end_idx-1 for inclusive display
    );

    let mut success_count = 0;
    let mut error_count = 0;

    // Iterate through the calculated batch range
    for i in start_idx..end_idx {
        // Get the record directly using the index
        if let Some(record) = scene_loading_state.loaded_entities.get(i) {
            // Serialize components to JSON (assuming update_entity requires String)
            match serde_json::to_string(&record.components) {
                Ok(json) => {
                    // Add the JSON string to the pending list for later application
                    scene_loading_state.pending_deserializations.push(json);
                    success_count += 1;
                }
                Err(e) => {
                    error!(
                        "Failed to serialize components for entity at index {}: {}",
                        i, e
                    );
                    error_count += 1;
                }
            }
        } else {
            // This indicates an issue with indexing logic if it occurs
            error!(
                "Invalid entity index {} encountered during batch processing.",
                i
            );
            error_count += 1;
        }
    }

    if success_count > 0 || error_count > 0 {
        info!(
            "Batch (indices {}-{}) processed: {} successful, {} failed.",
            start_idx,
            end_idx - 1,
            success_count,
            error_count
        );
    }

    // Update the index to point to the start of the next batch
    scene_loading_state.current_batch_index = end_idx;

    // No need to set is_processing_batch = false, as this system runs once per frame in Ready state
    // and processes one batch per run.
}

/// Applies the pending deserialized entity data to the world.
fn apply_pending_deserializations(world: &mut World) {
    // Use take to efficiently drain the pending list without cloning
    let pending_jsons = {
        // Borrow checker requires this scope
        let mut scene_loading_state = world.resource_mut::<SceneLoadingState>();
        std::mem::take(&mut scene_loading_state.pending_deserializations)
    };

    if !pending_jsons.is_empty() {
        debug!(
            "Applying {} pending entity deserializations...",
            pending_jsons.len()
        );
        let mut success_count = 0;
        let mut failure_count = 0;

        for json in pending_jsons {
            // Use update_entity to apply the changes to the world
            match update_entity(&json, world) {
                // Assuming update_entity takes &str
                Ok(_) => {
                    success_count += 1;
                    // Debug log might be too verbose here, consider Trace level if needed
                }
                Err(e) => {
                    error!("Failed to apply deserialized entity: {}", e);
                    // Consider logging the JSON string (potentially large) on error if useful
                    // error!("Failed JSON: {}", json);
                    failure_count += 1;
                }
            }
        }
        debug!(
            "Applied deserializations: {} successful, {} failed.",
            success_count, failure_count
        );
    }
    // If pending_jsons was empty, do nothing.
}
