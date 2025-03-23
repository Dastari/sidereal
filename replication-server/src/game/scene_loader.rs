use crate::database::{DatabaseClient, EntityRecord};
use bevy::prelude::*;
use std::sync::Arc;
use tracing::{error, info, warn};
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
                (process_pending_deserializations, apply_deferred)
                    .run_if(in_state(SceneState::Ready)),
            );
    }
}

/// Component to mark entities that need component deserialization
#[derive(Component)]
struct PendingDeserialization(Vec<String>);

/// Component to mark entities that have been successfully deserialized
#[derive(Component)]
struct DeserializedEntity;

/// State of the scene loading process
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SceneState {
    #[default]
    Connecting,
    Loading,
    Processing,
    Ready,
    Error,
}

/// Resource holding the state of scene loading
#[derive(Resource)]

pub struct SceneLoadingState {
    pub loaded_entities: Vec<EntityRecord>,
    pub _error_message: Option<String>,
}

impl Default for SceneLoadingState {
    fn default() -> Self {
        Self {
            loaded_entities: Vec::new(),
            _error_message: None,
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

            // Here we spawn the entities into the scene
            for entity_record in &scene_loading_state.loaded_entities {

            }

            scene_state.set(SceneState::Ready);
            commands.entity(task_entity).despawn();
        }
    }
}

/// Process pending deserializations - this system now uses a simpler approach
fn process_pending_deserializations(
    mut commands: Commands,
    query: Query<(Entity, &PendingDeserialization), Without<DeserializedEntity>>,
) {
    for (entity, _pending) in query.iter() {
        let entity_id = entity;

        // Use the entity itself to store serialized data
        // Then in the next frame, we'll have a separate system that deserializes
        // Mark this entity as deserializing so we can process it separately
        commands.entity(entity_id).insert(DeserializedEntity);

        debug!("Marked entity for deserialization in next frame");
    }
}

/// Apply deferred deserialization
fn apply_deferred(world: &mut World) {
    // Query for entities with PendingDeserialization and DeserializedEntity components
    let mut entities_to_process = Vec::new();

    // Get all entity IDs that need processing
    {
        let mut query =
            world.query_filtered::<(Entity, &PendingDeserialization), With<DeserializedEntity>>();
        for (entity, pending) in query.iter(world) {
            entities_to_process.push((entity, pending.0.clone()));
        }
    }

    // Process each entity
    for (entity, serialized_data) in entities_to_process {

    }
}


