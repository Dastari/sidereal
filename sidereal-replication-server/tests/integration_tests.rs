use bevy::prelude::*;
use bevy::app::ScheduleRunnerPlugin;
use bevy::time::TimePlugin;
use bevy_rapier2d::prelude::*;
use serde_json::json;
use uuid::Uuid;
use bevy_state::app::StatesPlugin;

use sidereal_core::ecs::components::physics::{PhysicsData, ColliderShapeData};
use sidereal_replication_server::database::EntityRecord;
use sidereal_replication_server::scene::SceneState;

// A simple resource to track test state
#[derive(Resource, Default)]
struct TestState {
    entities_created: usize,
}

// A test plugin to set up the world and inject test entities
pub struct TestPlugin;

impl Plugin for TestPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(TestState::default())
            .add_systems(Update, inject_test_entities)
            .add_systems(Update, verify_test_entities);
    }
}

// Inject test entities when the scene is ready
fn inject_test_entities(
    mut commands: Commands,
    mut test_state: ResMut<TestState>,
    state: Res<State<SceneState>>,
) {
    if state.get() != &SceneState::Ready || test_state.entities_created > 0 {
        return;
    }

    // Create a player entity
    let player_id = Uuid::new_v4().to_string();
    let player_record = EntityRecord {
        id: player_id.clone(),
        name: Some("Player".to_string()),
        owner_id: Some("test-user".to_string()),
        position_x: 0.0,
        position_y: 0.0,
        type_: "player".to_string(),
        components: json!({
            "physics": {
                "position": [0.0, 0.0],
                "rotation": 0.0,
                "rigid_body_type": "dynamic",
                "velocity": [0.0, 0.0, 0.0],
                "collider_shape": {
                    "Ball": {
                        "radius": 10.0
                    }
                },
                "mass": 10.0,
                "friction": 0.5,
                "restitution": 0.3,
                "gravity_scale": 1.0
            }
        }),
        created_at: None,
        updated_at: None,
    };

    // Create the player entity
    let player_entity = commands.spawn_empty().id();
    
    // Access the entity and apply components
    let mut entity = commands.entity(player_entity);
    
    // Add transform component
    entity.insert(Transform::from_xyz(
        player_record.position_x, 
        player_record.position_y, 
        0.0
    ));
    
    // Add name component
    if let Some(name) = &player_record.name {
        entity.insert(Name::new(name.clone()));
    }
    
    // Extract and apply physics data
    if let Some(physics_json) = player_record.components.get("physics") {
        if let Some(physics_data) = PhysicsData::from_json(physics_json) {
            // Apply position (already covered by the transform above)
            
            // Apply rotation
            if let Some(rotation) = physics_data.rotation {
                let mut transform = Transform::from_xyz(
                    player_record.position_x, 
                    player_record.position_y, 
                    0.0
                );
                transform.rotation = Quat::from_rotation_z(rotation);
                entity.insert(transform);
            }
            
            // Apply rigid body type
            if let Some(rigid_body_type) = &physics_data.rigid_body_type {
                match rigid_body_type.as_str() {
                    "dynamic" => entity.insert(RigidBody::Dynamic),
                    "static" => entity.insert(RigidBody::Fixed),
                    "kinematic" => entity.insert(RigidBody::KinematicPositionBased),
                    _ => entity.insert(RigidBody::Dynamic),
                };
            }
            
            // Apply velocity
            if let Some(velocity) = physics_data.velocity {
                entity.insert(Velocity {
                    linvel: Vec2::new(velocity[0], velocity[1]),
                    angvel: velocity[2],
                });
            }
            
            // Apply collider shape
            if let Some(ref shape) = physics_data.collider_shape {
                let collider = match shape {
                    ColliderShapeData::Ball { radius } => Collider::ball(*radius),
                    ColliderShapeData::Cuboid { hx, hy } => Collider::cuboid(*hx, *hy),
                    ColliderShapeData::Capsule { half_height, radius } => 
                        Collider::capsule_y(*half_height, *radius),
                };
                entity.insert(collider);
            }
            
            // Apply optional physics properties
            if let Some(mass) = physics_data.mass {
                entity.insert(AdditionalMassProperties::Mass(mass));
            }
            
            if let Some(friction) = physics_data.friction {
                entity.insert(Friction::coefficient(friction));
            }
            
            if let Some(restitution) = physics_data.restitution {
                entity.insert(Restitution::coefficient(restitution));
            }
            
            if let Some(gravity_scale) = physics_data.gravity_scale {
                entity.insert(GravityScale(gravity_scale));
            }
        }
    }

    // Create an asteroid entity
    let asteroid_id = Uuid::new_v4().to_string();
    let asteroid_record = EntityRecord {
        id: asteroid_id.clone(),
        name: Some("Asteroid".to_string()),
        owner_id: None,
        position_x: 100.0,
        position_y: 200.0,
        type_: "asteroid".to_string(),
        components: json!({
            "physics": {
                "position": [100.0, 200.0],
                "rotation": 0.3,
                "rigid_body_type": "dynamic",
                "velocity": [1.0, -0.5, 0.1],
                "collider_shape": {
                    "Ball": {
                        "radius": 20.0
                    }
                },
                "mass": 50.0,
                "friction": 0.1,
                "restitution": 0.8,
                "gravity_scale": 0.0
            }
        }),
        created_at: None,
        updated_at: None,
    };

    // Create the asteroid entity
    let asteroid_entity = commands.spawn_empty().id();
    
    // Access the entity and apply components
    let mut entity = commands.entity(asteroid_entity);
    
    // Add transform component
    entity.insert(Transform::from_xyz(
        asteroid_record.position_x, 
        asteroid_record.position_y, 
        0.0
    ));
    
    // Add name component
    if let Some(name) = &asteroid_record.name {
        entity.insert(Name::new(name.clone()));
    }
    
    // Extract and apply physics data
    if let Some(physics_json) = asteroid_record.components.get("physics") {
        if let Some(physics_data) = PhysicsData::from_json(physics_json) {
            // Apply position (already covered by the transform above)
            
            // Apply rotation
            if let Some(rotation) = physics_data.rotation {
                let mut transform = Transform::from_xyz(
                    asteroid_record.position_x, 
                    asteroid_record.position_y, 
                    0.0
                );
                transform.rotation = Quat::from_rotation_z(rotation);
                entity.insert(transform);
            }
            
            // Apply rigid body type
            if let Some(rigid_body_type) = &physics_data.rigid_body_type {
                match rigid_body_type.as_str() {
                    "dynamic" => entity.insert(RigidBody::Dynamic),
                    "static" => entity.insert(RigidBody::Fixed),
                    "kinematic" => entity.insert(RigidBody::KinematicPositionBased),
                    _ => entity.insert(RigidBody::Dynamic),
                };
            }
            
            // Apply velocity
            if let Some(velocity) = physics_data.velocity {
                entity.insert(Velocity {
                    linvel: Vec2::new(velocity[0], velocity[1]),
                    angvel: velocity[2],
                });
            }
            
            // Apply collider shape
            if let Some(ref shape) = physics_data.collider_shape {
                let collider = match shape {
                    ColliderShapeData::Ball { radius } => Collider::ball(*radius),
                    ColliderShapeData::Cuboid { hx, hy } => Collider::cuboid(*hx, *hy),
                    ColliderShapeData::Capsule { half_height, radius } => 
                        Collider::capsule_y(*half_height, *radius),
                };
                entity.insert(collider);
            }
            
            // Apply optional physics properties
            if let Some(mass) = physics_data.mass {
                entity.insert(AdditionalMassProperties::Mass(mass));
            }
            
            if let Some(friction) = physics_data.friction {
                entity.insert(Friction::coefficient(friction));
            }
            
            if let Some(restitution) = physics_data.restitution {
                entity.insert(Restitution::coefficient(restitution));
            }
            
            if let Some(gravity_scale) = physics_data.gravity_scale {
                entity.insert(GravityScale(gravity_scale));
            }
        }
    }

    // Create a station entity
    let station_id = Uuid::new_v4().to_string();
    let station_record = EntityRecord {
        id: station_id.clone(),
        name: Some("Space Station".to_string()),
        owner_id: Some("station-owner".to_string()),
        position_x: -150.0,
        position_y: -50.0,
        type_: "station".to_string(),
        components: json!({
            "physics": {
                "position": [-150.0, -50.0],
                "rotation": 1.0,
                "rigid_body_type": "static",
                "collider_shape": {
                    "Cuboid": {
                        "hx": 30.0,
                        "hy": 20.0
                    }
                }
            }
        }),
        created_at: None,
        updated_at: None,
    };

    // Create the station entity
    let station_entity = commands.spawn_empty().id();
    
    // Access the entity and apply components
    let mut entity = commands.entity(station_entity);
    
    // Add transform component
    entity.insert(Transform::from_xyz(
        station_record.position_x, 
        station_record.position_y, 
        0.0
    ));
    
    // Add name component
    if let Some(name) = &station_record.name {
        entity.insert(Name::new(name.clone()));
    }
    
    // Extract and apply physics data
    if let Some(physics_json) = station_record.components.get("physics") {
        if let Some(physics_data) = PhysicsData::from_json(physics_json) {
            // Apply position (already covered by the transform above)
            
            // Apply rotation
            if let Some(rotation) = physics_data.rotation {
                let mut transform = Transform::from_xyz(
                    station_record.position_x, 
                    station_record.position_y, 
                    0.0
                );
                transform.rotation = Quat::from_rotation_z(rotation);
                entity.insert(transform);
            }
            
            // Apply rigid body type
            if let Some(rigid_body_type) = &physics_data.rigid_body_type {
                match rigid_body_type.as_str() {
                    "dynamic" => entity.insert(RigidBody::Dynamic),
                    "static" => entity.insert(RigidBody::Fixed),
                    "kinematic" => entity.insert(RigidBody::KinematicPositionBased),
                    _ => entity.insert(RigidBody::Dynamic),
                };
            }
            
            // Apply collider shape
            if let Some(ref shape) = physics_data.collider_shape {
                let collider = match shape {
                    ColliderShapeData::Ball { radius } => Collider::ball(*radius),
                    ColliderShapeData::Cuboid { hx, hy } => Collider::cuboid(*hx, *hy),
                    ColliderShapeData::Capsule { half_height, radius } => 
                        Collider::capsule_y(*half_height, *radius),
                };
                entity.insert(collider);
            }
            
            // Apply optional physics properties if present
            if let Some(mass) = physics_data.mass {
                entity.insert(AdditionalMassProperties::Mass(mass));
            }
            
            if let Some(friction) = physics_data.friction {
                entity.insert(Friction::coefficient(friction));
            }
            
            if let Some(restitution) = physics_data.restitution {
                entity.insert(Restitution::coefficient(restitution));
            }
            
            if let Some(gravity_scale) = physics_data.gravity_scale {
                entity.insert(GravityScale(gravity_scale));
            }
        }
    }

    test_state.entities_created = 3;
}

// Verify that the test entities exist and have the correct components
fn verify_test_entities(
    query: Query<(&RigidBody, &Name)>,
    test_state: Res<TestState>,
    state: Res<State<SceneState>>,
) {
    if state.get() != &SceneState::Ready || test_state.entities_created == 0 {
        return;
    }

    // Count entities with RigidBody component (should be at least 3)
    let rigid_body_count = query.iter().count();
    assert!(rigid_body_count >= 3, "Expected at least 3 entities with RigidBody, found {}", rigid_body_count);

    // Verify entity names
    let mut player_found = false;
    let mut asteroid_found = false;
    let mut station_found = false;

    for (_, name) in query.iter() {
        match name.as_str() {
            "Player" => player_found = true,
            "Asteroid" => asteroid_found = true,
            "Space Station" => station_found = true,
            _ => {}
        }
    }

    assert!(player_found, "Player entity not found");
    assert!(asteroid_found, "Asteroid entity not found");
    assert!(station_found, "Space Station entity not found");
}

#[allow(dead_code)]
fn test_full_integration() {
    // Set up test app
    let mut app = App::new();
    
    // Add minimal plugins
    app.add_plugins(MinimalPlugins)
       .add_plugins(RapierPhysicsPlugin::<NoUserData>::default())
       .add_plugins(StatesPlugin::default())
       .add_plugins(TestPlugin)
       .init_state::<SceneState>();
    
    // Transition to the Ready state
    app.update();
    let mut next_state = app.world_mut().resource_mut::<NextState<SceneState>>();
    next_state.set(SceneState::Ready);
    drop(next_state);
    
    // Run the app to process systems
    app.update();
    
    // Verify the entities were created
    let test_state = app.world().resource::<TestState>();
    assert_eq!(test_state.entities_created, 3, "Expected 3 entities to be created");
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use bevy::utils::Duration;

    #[allow(dead_code)]
    fn setup_test_app() -> App {
        let mut app = App::new();
        
        // Add minimal required plugins
        app.add_plugins(MinimalPlugins)
           .add_plugins(RapierPhysicsPlugin::<NoUserData>::default())
           .add_plugins(ScheduleRunnerPlugin::run_loop(Duration::from_millis(100)))
           .add_plugins(TimePlugin::default())
           .add_plugins(TransformPlugin::default())
           .add_plugins(HierarchyPlugin::default())
           .add_plugins(StatesPlugin::default());

        // Add scene state
        app.init_state::<SceneState>();
        let mut next_state = app.world_mut().resource_mut::<NextState<SceneState>>();
        next_state.set(SceneState::Ready);
        drop(next_state);
        
        // Add test plugin
        app.add_plugins(TestPlugin);
        
        app
    }

    #[test]
    fn test_entity_serialization_roundtrip() {
        // Create an entity record with physics data
        let entity_id = Uuid::new_v4().to_string();
        let mut record = EntityRecord {
            id: entity_id.clone(),
            name: Some("Test Entity".to_string()),
            owner_id: Some("test-user".to_string()),
            position_x: 10.0,
            position_y: 20.0,
            type_: "test".to_string(),
            components: json!({}),
            created_at: None,
            updated_at: None,
        };
        
        // Create physics data
        let physics_data = PhysicsData {
            position: Some([10.0, 20.0]),
            rotation: Some(0.5),
            rigid_body_type: Some("dynamic".to_string()),
            velocity: Some([1.0, 2.0, 0.1]),
            collider_shape: Some(ColliderShapeData::Ball { radius: 5.0 }),
            mass: Some(10.0),
            friction: Some(0.5),
            restitution: Some(0.3),
            gravity_scale: Some(1.0),
        };
        
        // Serialize physics data to JSON and add to record
        record.components = json!({
            "physics": physics_data
        });
        
        // Serialize record to JSON
        let record_json = serde_json::to_string(&record).unwrap();
        
        // Deserialize back to record
        let deserialized_record: EntityRecord = serde_json::from_str(&record_json).unwrap();
        
        // Check fields match
        assert_eq!(deserialized_record.id, entity_id);
        assert_eq!(deserialized_record.name, Some("Test Entity".to_string()));
        assert_eq!(deserialized_record.owner_id, Some("test-user".to_string()));
        assert_eq!(deserialized_record.type_, "test");
        assert_eq!(deserialized_record.position_x, 10.0);
        assert_eq!(deserialized_record.position_y, 20.0);
        
        // Extract physics data
        let physics_obj = deserialized_record.components.as_object().unwrap()
            .get("physics").unwrap().as_object().unwrap();
        
        // Check some physics fields
        assert_eq!(physics_obj.get("position").unwrap().as_array().unwrap()[0].as_f64().unwrap(), 10.0);
        assert_eq!(physics_obj.get("position").unwrap().as_array().unwrap()[1].as_f64().unwrap(), 20.0);
        assert_eq!(physics_obj.get("rotation").unwrap().as_f64().unwrap(), 0.5);
        assert_eq!(physics_obj.get("rigid_body_type").unwrap().as_str().unwrap(), "dynamic");
        
        // Check collider shape
        let collider = physics_obj.get("collider_shape").unwrap().as_object().unwrap();
        assert!(collider.contains_key("Ball"));
        assert_eq!(collider.get("Ball").unwrap().as_object().unwrap().get("radius").unwrap().as_f64().unwrap(), 5.0);
    }

    #[test]
    fn test_entity_creation() {
        // Set up test app
        let mut app = App::new();
        
        // Add minimal plugins
        app.add_plugins(MinimalPlugins)
           .add_plugins(RapierPhysicsPlugin::<NoUserData>::default())
           .add_plugins(StatesPlugin)
           .add_plugins(TestPlugin)
           .init_state::<SceneState>();
        
        // Transition to the Ready state
        app.update();
        let mut next_state = app.world_mut().resource_mut::<NextState<SceneState>>();
        next_state.set(SceneState::Ready);
        drop(next_state);
        
        // Run the app to process systems
        app.update();
        
        // Verify the entities were created
        let test_state = app.world().resource::<TestState>();
        assert_eq!(test_state.entities_created, 3, "Expected 3 entities to be created");
    }
} 