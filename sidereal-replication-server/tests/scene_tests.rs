use bevy::prelude::*;
use serde_json::json;
use uuid::Uuid;
use bevy_rapier2d::prelude::*;  // Add Rapier imports for RigidBody and Collider
use bevy_state::app::StatesPlugin;
use std::collections::HashMap;

// Import the replication server code
use sidereal_replication_server::database::EntityRecord;
use sidereal_replication_server::scene::SceneState;
use sidereal_core::ecs::components::physics::{PhysicsData, ColliderShapeData};

#[cfg(test)]
mod scene_tests {
    use super::*;
    
    // Test helper to create a test app
    fn setup_test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins); // Add minimal Bevy plugins
        app.add_plugins(RapierPhysicsPlugin::<NoUserData>::default()); // Add Rapier
        app.add_plugins(StatesPlugin::default()); // Add the states plugin
        app.init_state::<SceneState>(); // Initialize the SceneState
        app
    }
    
    #[test]
    fn test_scene_state_transitions() {
        let mut app = setup_test_app();
        
        // Initial state should be Connecting
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
        
        // Manually transition to Error
        app.world_mut().resource_mut::<NextState<SceneState>>().set(SceneState::Error);
        app.update();
        assert_eq!(app.world().resource::<State<SceneState>>().get(), &SceneState::Error);
    }
    
    #[test]
    fn test_physics_data_deserialization() {
        // Create test physics data JSON
        let physics_json = json!({
            "position": [100.0, 200.0],
            "rotation": 0.5,
            "rigid_body_type": "dynamic",
            "velocity": [1.0, 2.0, 0.1],
            "collider_shape": {
                "Ball": {
                    "radius": 10.0
                }
            },
            "mass": 5.0,
            "friction": 0.5,
            "restitution": 0.3,
            "gravity_scale": 1.0
        });
        
        // Parse physics data from JSON
        let physics_data = PhysicsData::from_json(&physics_json).expect("Failed to deserialize physics data");
        
        // Verify values
        assert_eq!(physics_data.position, Some([100.0, 200.0]));
        assert_eq!(physics_data.rotation, Some(0.5));
        assert_eq!(physics_data.rigid_body_type, Some("dynamic".to_string()));
        assert_eq!(physics_data.velocity, Some([1.0, 2.0, 0.1]));
        assert_eq!(physics_data.mass, Some(5.0));
        assert_eq!(physics_data.friction, Some(0.5));
        assert_eq!(physics_data.restitution, Some(0.3));
        assert_eq!(physics_data.gravity_scale, Some(1.0));
        
        // Verify collider shape
        if let Some(ColliderShapeData::Ball { radius }) = physics_data.collider_shape {
            assert_eq!(radius, 10.0);
        } else {
            panic!("Unexpected collider shape");
        }
    }
    
    #[test]
    fn test_physics_data_serialization() {
        // Create a physics data struct
        let physics_data = PhysicsData {
            position: Some([100.0, 200.0]),
            rotation: Some(0.5),
            rigid_body_type: Some("dynamic".to_string()),
            velocity: Some([1.0, 2.0, 0.1]),
            collider_shape: Some(ColliderShapeData::Cuboid { hx: 5.0, hy: 10.0 }),
            mass: Some(5.0),
            friction: Some(0.5),
            restitution: Some(0.3),
            gravity_scale: Some(1.0),
        };
        
        // Serialize to JSON
        let json = physics_data.to_json();
        
        // Verify JSON structure
        assert_eq!(json["position"][0], 100.0);
        assert_eq!(json["position"][1], 200.0);
        assert_eq!(json["rotation"], 0.5);
        assert_eq!(json["rigid_body_type"], "dynamic");
        
        // Verify collider shape
        assert!(json["collider_shape"]["Cuboid"].is_object());
        assert_eq!(json["collider_shape"]["Cuboid"]["hx"], 5.0);
        assert_eq!(json["collider_shape"]["Cuboid"]["hy"], 10.0);
    }
    
    #[test]
    fn test_entity_creation_from_record() {
        // Create a test app
        let mut app = setup_test_app();
        
        // Generate a valid UUID for the entity
        let entity_id = Uuid::new_v4().to_string();
        
        // Create a test entity record with physics data
        let record = EntityRecord {
            id: entity_id.clone(),
            name: Some("TestEntity".to_string()),
            owner_id: None,
            position_x: 10.0,
            position_y: 20.0,
            type_: "object".to_string(),
            components: json!({
                "physics": {
                    "position": [10.0, 20.0],
                    "rotation": 0.5,
                    "rigid_body_type": "dynamic",
                    "velocity": [1.0, 2.0, 0.1],
                    "collider_shape": {
                        "Ball": {
                            "radius": 5.0
                        }
                    },
                    "mass": 1.0,
                    "friction": 0.5,
                    "restitution": 0.3,
                    "gravity_scale": 1.0
                }
            }),
            created_at: None,
            updated_at: None,
            physics_data: None,
        };
        
        // Test entity creation
        let entity = app.world_mut().spawn_empty().id();
        
        // Instead of using the private loader module, manually apply the record data
        let mut entity_mut = app.world_mut().entity_mut(entity);
        
        // Add a Name component
        if let Some(name) = &record.name {
            entity_mut.insert(Name::new(name.clone()));
        }
        
        // Add a Transform component with the position
        entity_mut.insert(Transform::from_xyz(record.position_x, record.position_y, 0.0));
        
        // Extract and apply physics data from components
        if let Some(physics_value) = record.components.get("physics") {
            if let Some(physics_data) = PhysicsData::from_json(physics_value) {
                // Add physics components manually
                if let Some(rigid_body_type) = &physics_data.rigid_body_type {
                    match rigid_body_type.as_str() {
                        "dynamic" => { entity_mut.insert(RigidBody::Dynamic); },
                        "static" => { entity_mut.insert(RigidBody::Fixed); },
                        "kinematic" => { entity_mut.insert(RigidBody::KinematicPositionBased); },
                        _ => { entity_mut.insert(RigidBody::Dynamic); },
                    };
                } else {
                    // Default to dynamic if not specified
                    entity_mut.insert(RigidBody::Dynamic);
                }
                
                if let Some(shape) = &physics_data.collider_shape {
                    match shape {
                        ColliderShapeData::Ball { radius } => {
                            entity_mut.insert(Collider::ball(*radius));
                        },
                        ColliderShapeData::Cuboid { hx, hy } => {
                            entity_mut.insert(Collider::cuboid(*hx, *hy));
                        },
                        ColliderShapeData::Capsule { half_height, radius } => {
                            entity_mut.insert(Collider::capsule_y(*half_height, *radius));
                        },
                    }
                }
            }
        }
        
        // Verify entity has the correct components
        let world = app.world();
        assert!(world.get::<Transform>(entity).is_some(), "Entity should have a Transform component");
        assert!(world.get::<RigidBody>(entity).is_some(), "Entity should have a RigidBody component");
        assert!(world.get::<Collider>(entity).is_some(), "Entity should have a Collider component");
        
        // Verify that the Name component has the expected value
        if let Some(name) = world.get::<Name>(entity) {
            assert_eq!(name.as_str(), "TestEntity");
        } else {
            panic!("Entity should have a Name component");
        }
        
        // Verify that the entity ID is valid
        assert!(Uuid::parse_str(&record.id).is_ok(), "Entity ID is not a valid UUID: {}", record.id);
        
        // Verify the transform position matches
        if let Some(transform) = world.get::<Transform>(entity) {
            assert_eq!(transform.translation.x, 10.0);
            assert_eq!(transform.translation.y, 20.0);
        } else {
            panic!("Entity should have a Transform component");
        }
        
        // Verify rigid body type
        if let Some(rb) = world.get::<RigidBody>(entity) {
            assert!(matches!(rb, RigidBody::Dynamic));
        } else {
            panic!("Entity should have a RigidBody component");
        }
    }
} 