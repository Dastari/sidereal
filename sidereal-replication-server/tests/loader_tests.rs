use bevy::prelude::*;
use bevy_rapier2d::prelude::*;
use serde_json::json;
use bevy_state::app::StatesPlugin;
use std::collections::HashMap;

use sidereal_core::ecs::components::physics::{PhysicsData, ColliderShapeData};
use sidereal_replication_server::{
    database::EntityRecord,
    scene::SceneState,
};

// Helper function to run tests
fn run_test<F>(app: &mut App, test_fn: F)
where
    F: FnOnce(&mut App),
{
    test_fn(app);
}

// Mock database client for testing
#[derive(Resource)]
struct MockDatabaseClient {
    entities: Vec<EntityRecord>,
}

impl MockDatabaseClient {
    fn new() -> Self {
        Self {
            entities: Vec::new(),
        }
    }
    
    fn add_test_entity(&mut self, id: &str, entity_type: &str, x: f32, y: f32) {
        let physics_data = PhysicsData {
            position: Some([x, y]),
            rotation: Some(0.0),
            rigid_body_type: Some("dynamic".to_string()),
            velocity: Some([0.0, 0.0, 0.0]),
            collider_shape: Some(ColliderShapeData::Ball { radius: 10.0 }),
            mass: Some(1.0),
            friction: Some(0.5),
            restitution: Some(0.3),
            gravity_scale: Some(1.0),
        };
        
        let record = EntityRecord {
            id: id.to_string(),
            name: Some(format!("Test Entity {}", id)),
            owner_id: Some("test-user".to_string()),
            position_x: x,
            position_y: y,
            type_: entity_type.to_string(),
            components: physics_data.to_json(),
            created_at: Some("2023-01-01T00:00:00Z".to_string()),
            updated_at: Some("2023-01-01T00:00:00Z".to_string()),
            physics_data: None,
        };
        
        self.entities.push(record);
    }
}

// Test state resource
#[derive(Resource, Default)]
struct TestState {
    #[allow(dead_code)]
    entities_created: usize,
}

// Setup test app with required plugins
fn setup_test_app() -> App {
    let mut app = App::new();
    
    // Add minimal required plugins
    app.add_plugins(MinimalPlugins)
       .add_plugins(RapierPhysicsPlugin::<NoUserData>::default())
       .add_plugins(TransformPlugin::default())
       .add_plugins(HierarchyPlugin::default())
       .add_plugins(StatesPlugin::default());
    
    // Add scene state
    app.init_state::<SceneState>();
    
    // Add test state
    app.insert_resource(TestState::default());
    
    app
}

#[test]
fn test_state_transitions() {
    let mut app = setup_test_app();
    
    // Initialize to Connecting
    assert_eq!(app.world().resource::<State<SceneState>>().get(), &SceneState::Connecting);
    
    // Manually transition to Loading
    app.world_mut().resource_mut::<NextState<SceneState>>().set(SceneState::Loading);
    app.update();
    assert_eq!(app.world().resource::<State<SceneState>>().get(), &SceneState::Loading);
    
    // Manually transition to Processing
    app.world_mut().resource_mut::<NextState<SceneState>>().set(SceneState::Processing);
    app.update();
    assert_eq!(app.world().resource::<State<SceneState>>().get(), &SceneState::Processing);
    
    // Manually transition to Ready
    app.world_mut().resource_mut::<NextState<SceneState>>().set(SceneState::Ready);
    app.update();
    assert_eq!(app.world().resource::<State<SceneState>>().get(), &SceneState::Ready);
}

#[test]
fn test_physics_data_extraction() {
    let mut app = setup_test_app();
    
    run_test(&mut app, |app| {
        // Create test physics data
        let component_data = vec![
            json!({
                "position": [10.0, 20.0],
                "rotation": 1.5,
                "rigid_body_type": "dynamic",
                "velocity": [1.0, 2.0, 0.1],
                "collider_shape": {
                    "Ball": {
                        "radius": 15.0
                    }
                },
                "mass": 2.5,
                "friction": 0.4,
                "restitution": 0.6,
                "gravity_scale": 0.8
            }),
            json!({
                "position": [30.0, 40.0],
                "rotation": 0.5,
                "rigid_body_type": "fixed",
                "velocity": [0.0, 0.0, 0.0],
                "collider_shape": {
                    "Cuboid": {
                        "hx": 10.0,
                        "hy": 5.0
                    }
                }
            })
        ];
        
        // Create entities directly
        for component_json in &component_data {
            if let Some(physics_data) = PhysicsData::from_json(component_json) {
                // Create an entity with the physics data
                let entity_id = app.world_mut().spawn_empty().id();
                let mut entity_commands = app.world_mut().entity_mut(entity_id);
                
                // Manually apply physics components based on the data
                if let Some(position) = physics_data.position {
                    entity_commands.insert(Transform::from_xyz(position[0], position[1], 0.0));
                }
                
                if let Some(rotation) = physics_data.rotation {
                    entity_commands.insert(Transform::from_rotation(Quat::from_rotation_z(rotation)));
                }
                
                if let Some(rb_type) = &physics_data.rigid_body_type {
                    match rb_type.as_str() {
                        "dynamic" => { entity_commands.insert(RigidBody::Dynamic); },
                        "fixed" => { entity_commands.insert(RigidBody::Fixed); },
                        "kinematic_position_based" => { entity_commands.insert(RigidBody::KinematicPositionBased); },
                        "kinematic_velocity_based" => { entity_commands.insert(RigidBody::KinematicVelocityBased); },
                        _ => {}
                    }
                }
                
                // Add collider based on shape
                if let Some(shape) = &physics_data.collider_shape {
                    match shape {
                        ColliderShapeData::Ball { radius } => {
                            entity_commands.insert(Collider::ball(*radius));
                        },
                        ColliderShapeData::Cuboid { hx, hy } => {
                            entity_commands.insert(Collider::cuboid(*hx, *hy));
                        },
                        ColliderShapeData::Capsule { half_height, radius } => {
                            entity_commands.insert(Collider::capsule_y(*half_height, *radius));
                        }
                    }
                }
            }
        }
        
        // Update the world
        app.update();
        
        // Verify the physics components were inserted correctly
        // TODO: Implement verification
    });
}

#[test]
fn test_load_entities_system() {
    let mut app = setup_test_app();
    
    run_test(&mut app, |app| {
        // Set up a mock database with entities
        let mut mock_db = MockDatabaseClient::new();
        mock_db.add_test_entity("entity-1", "player", 10.0, 20.0);
        mock_db.add_test_entity("entity-2", "asteroid", 30.0, 40.0);
        app.insert_resource(mock_db);
        
        // Manually transition to Loading
        app.world_mut().resource_mut::<NextState<SceneState>>().set(SceneState::Loading);
        app.update();
        assert_eq!(app.world().resource::<State<SceneState>>().get(), &SceneState::Loading);
        
        // Manually transition to Processing
        app.world_mut().resource_mut::<NextState<SceneState>>().set(SceneState::Processing);
        app.update();
        assert_eq!(app.world().resource::<State<SceneState>>().get(), &SceneState::Processing);
        
        // Manually transition to Ready
        app.world_mut().resource_mut::<NextState<SceneState>>().set(SceneState::Ready);
        app.update();
        assert_eq!(app.world().resource::<State<SceneState>>().get(), &SceneState::Ready);
        
        // Verify entities were created
        // TODO: Implement verification
    });
}

#[test]
fn test_invalid_physics_data() {
    let mut app = setup_test_app();
    
    run_test(&mut app, |app| {
        // Test with invalid JSON
        let invalid_json = json!({
            "position": "not_an_array",
            "rotation": true,
            "rigid_body_type": 123,
        });
        
        let physics_data = PhysicsData::from_json(&invalid_json);
        assert!(physics_data.is_none(), "Expected None for invalid physics data");
        
        // Test with incomplete JSON that might be valid
        let incomplete_json = json!({
            "position": [10.0, 20.0],
            // Missing rotation and other fields
        });
        
        // The incomplete JSON might be valid since all fields are optional
        let physics_data = PhysicsData::from_json(&incomplete_json);
        assert!(physics_data.is_some(), "PhysicsData should accept partial data with just position");
        
        // Manually transition to Error
        app.world_mut().resource_mut::<NextState<SceneState>>().set(SceneState::Error);
        app.update();
        
        // Verify state change
        assert_eq!(app.world().resource::<State<SceneState>>().get(), &SceneState::Error);
    });
} 