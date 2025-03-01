use bevy::prelude::*;
use bevy_rapier2d::prelude::*;
use uuid::Uuid;
use std::collections::HashMap;

use sidereal_core::ecs::components::physics::{PhysicsData, ColliderShapeData};
use sidereal_replication_server::database::EntityRecord;

#[cfg(test)]
mod physics_tests {
    use super::*;

    fn setup_test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
           .add_plugins(RapierPhysicsPlugin::<NoUserData>::default());
        app
    }

    #[test]
    fn test_physics_data_creation() {
        // Create physics data manually
        let physics_data = PhysicsData {
            position: Some([100.0, 200.0]),
            rotation: Some(0.5),
            rigid_body_type: Some("dynamic".to_string()),
            velocity: Some([1.0, 2.0, 0.1]),
            collider_shape: Some(ColliderShapeData::Ball { radius: 10.0 }),
            mass: Some(5.0),
            friction: Some(0.5),
            restitution: Some(0.3),
            gravity_scale: Some(1.0),
        };

        // Verify fields
        assert_eq!(physics_data.position, Some([100.0, 200.0]));
        assert_eq!(physics_data.rotation, Some(0.5));
        assert_eq!(physics_data.rigid_body_type, Some("dynamic".to_string()));
        assert_eq!(physics_data.mass, Some(5.0));
    }

    #[test]
    fn test_physics_data_from_components() {
        // Create components
        let transform = Transform::from_xyz(100.0, 200.0, 0.0);
        let rigid_body = RigidBody::Dynamic;
        let velocity = Velocity {
            linvel: Vec2::new(1.0, 2.0),
            angvel: 0.1,
        };
        let mass_props = AdditionalMassProperties::Mass(5.0);
        let friction = Friction::coefficient(0.5);
        let restitution = Restitution::coefficient(0.3);
        let gravity_scale = GravityScale(1.0);

        // Create physics data from components
        let physics_data = PhysicsData::from_components(
            Some(&transform),
            Some(&rigid_body),
            Some(&velocity),
            None, // No collider for now
            Some(&mass_props),
            Some(&friction),
            Some(&restitution),
            Some(&gravity_scale),
        );

        // Verify fields
        assert_eq!(physics_data.position, Some([100.0, 200.0]));
        assert!(physics_data.rotation.is_some()); // Rotation should exist but value may vary
        assert_eq!(physics_data.rigid_body_type, Some("dynamic".to_string()));
        assert_eq!(physics_data.velocity, Some([1.0, 2.0, 0.1]));
        assert_eq!(physics_data.mass, Some(5.0));
        assert_eq!(physics_data.friction, Some(0.5));
        assert_eq!(physics_data.restitution, Some(0.3));
        assert_eq!(physics_data.gravity_scale, Some(1.0));
    }

    #[test]
    fn test_physics_data_to_entity() {
        let mut app = setup_test_app();

        // Create physics data
        let physics_data = PhysicsData {
            position: Some([100.0, 200.0]),
            rotation: Some(0.5),
            rigid_body_type: Some("dynamic".to_string()),
            velocity: Some([1.0, 2.0, 0.1]),
            collider_shape: Some(ColliderShapeData::Ball { radius: 10.0 }),
            mass: Some(5.0),
            friction: Some(0.5),
            restitution: Some(0.3),
            gravity_scale: Some(1.0),
        };

        // Apply physics data to a new entity
        let entity_id = app.world_mut().spawn_empty().id();
        
        // Apply physics data directly using entity_mut
        let mut entity_mut = app.world_mut().entity_mut(entity_id);
        
        // Apply position
        if let Some(position) = physics_data.position {
            entity_mut.insert(Transform::from_xyz(position[0], position[1], 0.0));
        }
        
        // Apply rotation
        if let Some(rotation) = physics_data.rotation {
            if let Some(mut transform) = entity_mut.get_mut::<Transform>() {
                transform.rotation = Quat::from_rotation_z(rotation);
            }
        }
        
        // Apply rigid body type
        if let Some(rigid_body_type) = &physics_data.rigid_body_type {
            match rigid_body_type.as_str() {
                "dynamic" => entity_mut.insert(RigidBody::Dynamic),
                "static" => entity_mut.insert(RigidBody::Fixed),
                "kinematic" => entity_mut.insert(RigidBody::KinematicPositionBased),
                _ => entity_mut.insert(RigidBody::Dynamic),
            };
        }
        
        // Apply velocity
        if let Some(velocity) = physics_data.velocity {
            entity_mut.insert(Velocity {
                linvel: Vec2::new(velocity[0], velocity[1]),
                angvel: velocity[2],
            });
        }
        
        // Apply collider shape
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
        
        // Apply mass
        if let Some(mass) = physics_data.mass {
            entity_mut.insert(AdditionalMassProperties::Mass(mass));
        }
        
        // Apply friction
        if let Some(friction) = physics_data.friction {
            entity_mut.insert(Friction {
                coefficient: friction,
                combine_rule: CoefficientCombineRule::Average,
            });
        }
        
        // Apply restitution
        if let Some(restitution) = physics_data.restitution {
            entity_mut.insert(Restitution {
                coefficient: restitution,
                combine_rule: CoefficientCombineRule::Average,
            });
        }
        
        // Apply gravity scale
        if let Some(gravity_scale) = physics_data.gravity_scale {
            entity_mut.insert(GravityScale(gravity_scale));
        }
        
        // Update app to process commands
        app.update();

        // Verify components
        let transform = app.world().entity(entity_id).get::<Transform>().unwrap();
        assert_eq!(transform.translation.x, 100.0);
        assert_eq!(transform.translation.y, 200.0);

        let rigid_body = app.world().entity(entity_id).get::<RigidBody>().unwrap();
        assert_eq!(*rigid_body, RigidBody::Dynamic);

        let velocity = app.world().entity(entity_id).get::<Velocity>().unwrap();
        assert_eq!(velocity.linvel.x, 1.0);
        assert_eq!(velocity.linvel.y, 2.0);
        assert_eq!(velocity.angvel, 0.1);

        let _collider = app.world().entity(entity_id).get::<Collider>().unwrap();
        // We can't directly verify the collider shape, but we can check it exists

        let mass = app.world().entity(entity_id).get::<AdditionalMassProperties>().unwrap();
        match mass {
            AdditionalMassProperties::Mass(m) => assert_eq!(*m, 5.0),
            _ => panic!("Expected Mass but got a different AdditionalMassProperties"),
        }

        let friction = app.world().entity(entity_id).get::<Friction>().unwrap();
        assert_eq!(friction.coefficient, 0.5);

        let restitution = app.world().entity(entity_id).get::<Restitution>().unwrap();
        assert_eq!(restitution.coefficient, 0.3);

        let gravity_scale = app.world().entity(entity_id).get::<GravityScale>().unwrap();
        assert_eq!(gravity_scale.0, 1.0);
    }

    #[test]
    fn test_physics_data_json_roundtrip() {
        // Create a simple physics data object
        let physics_data = PhysicsData {
            position: Some([100.0, 200.0]),
            rotation: Some(0.5),
            rigid_body_type: Some("dynamic".to_string()),
            velocity: Some([1.0, 2.0, 0.1]),
            collider_shape: Some(ColliderShapeData::Ball { radius: 10.0 }),
            mass: Some(5.0),
            friction: Some(0.5),
            restitution: Some(0.3),
            gravity_scale: Some(1.0),
        };
        
        // Serialize to JSON
        let json = physics_data.to_json();
        
        // Deserialize back
        let deserialized = PhysicsData::from_json(&json).expect("Failed to deserialize");
        
        // Verify fields match
        assert_eq!(deserialized.position, Some([100.0, 200.0]));
        assert_eq!(deserialized.rotation, Some(0.5));
        assert_eq!(deserialized.rigid_body_type, Some("dynamic".to_string()));
        assert_eq!(deserialized.velocity, Some([1.0, 2.0, 0.1]));
        assert_eq!(deserialized.mass, Some(5.0));
        assert_eq!(deserialized.friction, Some(0.5));
        assert_eq!(deserialized.restitution, Some(0.3));
        assert_eq!(deserialized.gravity_scale, Some(1.0));
        
        // Verify collider shape
        match deserialized.collider_shape {
            Some(ColliderShapeData::Ball { radius }) => {
                assert_eq!(radius, 10.0);
            },
            _ => panic!("Expected Ball shape, got something else"),
        }
        
        // Generate a UUID for testing
        let entity_id = Uuid::new_v4().to_string();
        
        // Create an EntityRecord using the physics data
        let record = EntityRecord {
            id: entity_id.clone(),
            name: Some("Test Physics Entity".to_string()),
            owner_id: None,
            position_x: 100.0,
            position_y: 200.0,
            type_: "object".to_string(),
            components: json,
            created_at: None,
            updated_at: None,
            physics_data: None,
        };
        
        // Verify entity ID is a valid UUID
        assert!(Uuid::parse_str(&record.id).is_ok(), "Entity ID is not a valid UUID: {}", record.id);
        
        // Extract physics data from the record
        let extracted = PhysicsData::from_json(&record.components).expect("Failed to extract physics data");
        
        // Verify data matches original
        assert_eq!(extracted.position, physics_data.position);
        assert_eq!(extracted.rotation, physics_data.rotation);
        assert_eq!(extracted.rigid_body_type, physics_data.rigid_body_type);
    }

    #[test]
    fn test_different_collider_shapes() {
        // Test ball shape
        let ball_data = PhysicsData {
            position: Some([0.0, 0.0]),
            rotation: None,
            rigid_body_type: Some("dynamic".to_string()),
            velocity: None,
            collider_shape: Some(ColliderShapeData::Ball { radius: 10.0 }),
            mass: None,
            friction: None,
            restitution: None,
            gravity_scale: None,
        };

        // Test cuboid shape
        let cuboid_data = PhysicsData {
            position: Some([0.0, 0.0]),
            rotation: None,
            rigid_body_type: Some("dynamic".to_string()),
            velocity: None,
            collider_shape: Some(ColliderShapeData::Cuboid { hx: 5.0, hy: 7.5 }),
            mass: None,
            friction: None,
            restitution: None,
            gravity_scale: None,
        };

        // Test capsule shape
        let capsule_data = PhysicsData {
            position: Some([0.0, 0.0]),
            rotation: None,
            rigid_body_type: Some("dynamic".to_string()),
            velocity: None,
            collider_shape: Some(ColliderShapeData::Capsule { half_height: 5.0, radius: 2.0 }),
            mass: None,
            friction: None,
            restitution: None,
            gravity_scale: None,
        };

        // Convert to JSON and verify shape properties
        let ball_json = ball_data.to_json();
        assert_eq!(ball_json["collider_shape"]["Ball"]["radius"], 10.0);

        let cuboid_json = cuboid_data.to_json();
        assert_eq!(cuboid_json["collider_shape"]["Cuboid"]["hx"], 5.0);
        assert_eq!(cuboid_json["collider_shape"]["Cuboid"]["hy"], 7.5);

        let capsule_json = capsule_data.to_json();
        assert_eq!(capsule_json["collider_shape"]["Capsule"]["half_height"], 5.0);
        assert_eq!(capsule_json["collider_shape"]["Capsule"]["radius"], 2.0);
    }
} 