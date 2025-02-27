use bevy::prelude::{self, *};
use tracing::{info, error};
use std::sync::Arc;
use bevy_rapier2d::prelude::*;

use crate::database::{DatabaseClient, EntityRecord, DatabaseResult};
use sidereal_core::ecs::components::*;
use sidereal_core::ecs::components::physics::PhysicsData;

/// Plugin for managing the universe scene
pub struct SceneLoaderPlugin;

impl Plugin for SceneLoaderPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<SceneState>()
           .init_resource::<SceneLoadingState>()
           .add_systems(Startup, setup)
           .add_systems(Update, check_db_connection.run_if(in_state(SceneState::Connecting)))
           .add_systems(Update, load_entities_system.run_if(in_state(SceneState::Loading)))
           .add_systems(Update, process_loaded_entities.run_if(in_state(SceneState::Processing)));
    }
}

/// State of the scene loading process
#[derive(States, Debug, Clone, Default, Eq, PartialEq, Hash)]
pub enum SceneState {
    #[default]
    Connecting,
    Loading,
    Processing,
    Ready,
    Error,
}

/// Resource for managing the scene loading state
#[derive(Resource, Default)]
pub struct SceneLoadingState {
    pub loaded_entities: Vec<EntityRecord>,
    // This field may be used in the future for error handling
    pub _error_message: Option<String>,
}

/// Component for marking an entity as an async task
#[derive(Component)]
struct AsyncTask(Arc<std::sync::Mutex<Option<Vec<EntityRecord>>>>);

/// Setup function for initializing the scene
fn setup(_commands: Commands) {
    info!("Initializing scene loader");
}

/// System for checking database connection
fn check_db_connection(
    mut commands: Commands,
    mut scene_state: ResMut<NextState<SceneState>>,
    db_client: Option<Res<DatabaseClient>>,
) {
    if db_client.is_some() {
        scene_state.set(SceneState::Loading);
        return;
    }
    
    // Try to create a database client
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

/// System for loading entities from the database
fn load_entities_system(
    mut commands: Commands,
    db_client: Res<DatabaseClient>,
    mut scene_state: ResMut<NextState<SceneState>>,
    query: Query<Entity, With<AsyncTask>>,
) {
    // Only start one task
    if !query.is_empty() {
        return;
    }
    
    info!("Starting to load entities from database");
    
    // Clone necessary data for the async task (currently unused but might be needed in the future)
    let _db_url = db_client.base_url.clone();
    
    // Create a new database client inside the task
    let task_result = Arc::new(std::sync::Mutex::new(None));
    let task_result_clone = task_result.clone();
    
    // Spawn a thread to handle the database query
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let result = runtime.block_on(async {
            match DatabaseClient::new() {
                Ok(client) => {
                    match client.fetch_all_entities().await {
                        Ok(entities) => {
                            info!("Loaded {} entities from database", entities.len());
                            Some(entities)
                        }
                        Err(e) => {
                            error!("Failed to load entities: {}", e);
                            None
                        }
                    }
                }
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
    
    // Spawn an entity to track the task
    commands.spawn((
        AsyncTask(task_result),
        bevy::core::Name::new("Entity Loader Task"),
    ));
    
    scene_state.set(SceneState::Processing);
}

/// System for processing loaded entities
fn process_loaded_entities(
    mut commands: Commands,
    task_query: Query<(Entity, &AsyncTask)>,
    mut scene_state: ResMut<NextState<SceneState>>,
    mut scene_loading_state: ResMut<SceneLoadingState>,
) {
    for (task_entity, task) in task_query.iter() {
        // Check if the task has completed
        let has_entities = {
            let lock = task.0.lock().unwrap();
            lock.is_some()
        };
        
        if has_entities {
            // Get the entities data
            let entities = {
                let mut lock = task.0.lock().unwrap();
                lock.take().unwrap()
            };
            
            info!("Processing {} loaded entities", entities.len());
            scene_loading_state.loaded_entities = entities;
            
            // Spawn the entities into the scene
            for entity_record in &scene_loading_state.loaded_entities {
                spawn_entity_from_record_cmd(&mut commands, entity_record);
            }
            
            scene_state.set(SceneState::Ready);
            
            // Remove the task entity
            commands.entity(task_entity).despawn();
        }
    }
}

/// Spawn a single entity from a database record (using Commands)
fn spawn_entity_from_record_cmd(commands: &mut Commands, record: &EntityRecord) {
    let id = record.id.clone();
    let entity_name = record.name.clone().unwrap_or_else(|| "Unnamed".to_string());
    let entity_type = record.type_.clone();
    
    // Spawn entity with basic components
    let mut entity_commands = commands.spawn_empty();
    entity_commands
        .insert(prelude::Name::new(format!("{} ({})", entity_name, id)));
    
    // Add position/transform
    let mut transform = Transform::default();
    transform.translation.x = record.position_x;
    transform.translation.y = record.position_y;
    entity_commands.insert(transform);
    
    // Try to extract physics data from the JSON blob
    if let Some(physics_data) = PhysicsData::from_json(&record.components) {
        // Apply the physics data to the entity
        physics_data.apply_to_entity(&mut entity_commands);
    } else {
        // Fallback to default physics components if parsing fails
        entity_commands
            .insert(RigidBody::Dynamic)
            .insert(Collider::ball(16.0))
            .insert(Velocity::default());
    }
    
    // Add entity type-specific components
    match entity_type.as_str() {
        "player" => {
            // Add any player-specific components
        },
        "enemy" => {
            // Add any enemy-specific components
        },
        // Add more entity types as needed
        _ => {}
    }
    
    info!("Spawned entity: {} ({})", entity_name, id);
}

/// Load entities from a scene JSON data into a world
/// This is a placeholder function for future use to load entities from JSON data
#[allow(dead_code)]
pub fn _load_entities_from_json(mut commands: Commands, entities_data: Vec<EntityRecord>) {
    info!("Loading {} entities from database", entities_data.len());

    for entity_data in entities_data {
        spawn_entity_from_record_cmd(&mut commands, &entity_data);
    }
}

/// Example function: Save an entity back to the database
#[allow(dead_code)]
pub fn save_entity_to_database(
    query: &Query<(
        &Transform,
        &RigidBody,
        Option<&Velocity>,
        Option<&Collider>,
        Option<&AdditionalMassProperties>,
        Option<&Friction>,
        Option<&Restitution>,
        Option<&GravityScale>,
        &prelude::Name,
    )>,
    entity: Entity,
    _db_client: &DatabaseClient,
) -> DatabaseResult<()> {
    if let Ok((
        transform,
        rigid_body,
        velocity,
        collider,
        mass_props,
        friction,
        restitution,
        gravity_scale,
        name,
    )) = query.get(entity) {
        // Create physics data from components
        let physics_data = PhysicsData::from_components(
            Some(transform),
            Some(rigid_body),
            velocity,
            collider,
            mass_props,
            friction,
            restitution,
            gravity_scale,
        );
        
        // Convert physics data to JSON for storage
        let physics_json = physics_data.to_json();
        
        // Create an entity record
        let record = EntityRecord {
            id: entity.to_bits().to_string(), // Convert entity ID to string
            name: Some(name.to_string()),
            position_x: transform.translation.x,
            position_y: transform.translation.y,
            type_: "default".to_string(), // You would need logic to determine the entity type
            components: physics_json,
            created_at: None,  // These would be handled by the database
            updated_at: None,  // These would be handled by the database
            owner_id: None,    // You would need logic to determine the owner
        };
        
        // Save the record to the database - this is just an example
        info!("Saving entity to database: {:?}", record.name);
        
        // In a real implementation, you would do:
        // db_client.create_entity(&record).await
        
        Ok(())
    } else {
        Err(crate::database::DatabaseError::NotFound)
    }
}

/// Loads mock data for testing
/// This is a placeholder function for adding test entities
#[allow(dead_code)]
fn _load_mock_data(commands: &mut Commands) {
    info!("Loading mock data for testing");
    
    // Create a test ship
    commands.spawn((
        bevy::core::Name::new("Test Ship"),
        Transform::from_xyz(0.0, 0.0, 0.0),
        RigidBody::Dynamic,
        Collider::cuboid(5.0, 2.5), // Half-width and half-height
        Velocity {
            linvel: Vec2::new(0.0, 0.0),
            angvel: 0.0,
        },
        Hull {
            width: 10.0,
            height: 5.0,
            blocks: Vec::new(),
        },
    ));
    
    // Create a test asteroid
    commands.spawn((
        bevy::core::Name::new("Test Asteroid"),
        Transform::from_xyz(100.0, 100.0, 0.0),
        RigidBody::Dynamic,
        Collider::ball(10.0),
        Velocity {
            linvel: Vec2::new(1.0, 0.5),
            angvel: 0.1,
        },
    ));
    
    // Create a test station
    commands.spawn((
        bevy::core::Name::new("Test Station"),
        Transform::from_xyz(-100.0, -100.0, 0.0),
        RigidBody::Fixed,
        Collider::cuboid(10.0, 10.0), // Half-width and half-height
        Hull {
            width: 20.0,
            height: 20.0,
            blocks: Vec::new(),
        },
    ));
} 