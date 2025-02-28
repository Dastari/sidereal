use bevy::prelude::*;
use serde_json::json;
use std::env;
use uuid::Uuid;

// Import the replication server code
use sidereal_replication_server::database::{DatabaseClient, EntityRecord, DatabaseError};

#[cfg(test)]
mod database_tests {
    use super::*;

    /// Helper function to set up test environment for database tests
    fn setup_test_env() {
        env::set_var("SUPABASE_URL", "http://test-server.example.com");
        env::set_var("ANON_KEY", "test-key-123");
    }

    #[test]
    fn test_database_client_creation() {
        setup_test_env();
        
        // This test should succeed if the DatabaseClient is created without errors
        let client = DatabaseClient::new().expect("Failed to create client");
        
        // Verify the base URL was set correctly
        assert_eq!(client.base_url, "http://test-server.example.com");
    }
    
    #[test]
    fn test_entity_record_serialization() {
        // Generate a valid UUID for the entity
        let entity_id = Uuid::new_v4().to_string();
        
        // Create a test entity record
        let record = EntityRecord {
            id: entity_id.clone(),
            name: Some("Test Entity".to_string()),
            owner_id: Some("test-user".to_string()),
            position_x: 10.0,
            position_y: 20.0,
            type_: "test-type".to_string(),
            components: json!({
                "physics": {
                    "position": [10.0, 20.0],
                    "rotation": 0.5,
                    "rigid_body_type": "dynamic",
                    "velocity": [1.0, 2.0, 0.0],
                    "collider_shape": {
                        "type": "ball",
                        "radius": 5.0
                    },
                    "mass": 10.0,
                    "friction": 0.5,
                    "restitution": 0.2,
                    "gravity_scale": 1.0
                }
            }),
            created_at: Some("2023-01-01T00:00:00Z".to_string()),
            updated_at: Some("2023-01-01T00:00:00Z".to_string()),
        };
        
        // Serialize to JSON
        let serialized = serde_json::to_string(&record).expect("Failed to serialize");
        
        // Deserialize back into an EntityRecord
        let deserialized: EntityRecord = serde_json::from_str(&serialized).expect("Failed to deserialize");
        
        // Verify fields match
        assert_eq!(deserialized.id, entity_id);
        assert_eq!(deserialized.name, Some("Test Entity".to_string()));
        assert_eq!(deserialized.owner_id, Some("test-user".to_string()));
        assert_eq!(deserialized.position_x, 10.0);
        assert_eq!(deserialized.position_y, 20.0);
        assert_eq!(deserialized.type_, "test-type");

        // Verify the ID is a valid UUID
        assert!(Uuid::parse_str(&deserialized.id).is_ok(), "Entity ID is not a valid UUID");
    }
    
    #[test]
    fn test_entity_record_physics_json() {
        // Generate a valid UUID for the entity
        let entity_id = Uuid::new_v4().to_string();
        
        // Create a test entity record with physics data
        let record = EntityRecord {
            id: entity_id,
            name: Some("Physics Test".to_string()),
            owner_id: None,
            position_x: 50.0,
            position_y: 75.0,
            type_: "physics-test".to_string(),
            components: json!({
                "physics": {
                    "position": [50.0, 75.0],
                    "rotation": 1.5,
                    "rigid_body_type": "dynamic",
                    "velocity": [5.0, -2.0, 0.1],
                    "collider_shape": {
                        "type": "ball",
                        "radius": 10.0
                    },
                    "mass": 20.0,
                    "friction": 0.7,
                    "restitution": 0.5,
                    "gravity_scale": 1.0
                }
            }),
            created_at: None,
            updated_at: None,
        };
        
        // Access physics data
        let physics = record.components.get("physics").expect("Physics data not found");
        
        // Verify physics data fields
        assert_eq!(physics["position"][0], 50.0);
        assert_eq!(physics["position"][1], 75.0);
        assert_eq!(physics["rotation"], 1.5);
        assert_eq!(physics["rigid_body_type"], "dynamic");
        assert_eq!(physics["mass"], 20.0);
        assert_eq!(physics["friction"], 0.7);
        assert_eq!(physics["restitution"], 0.5);
        
        // Verify collider shape
        let collider = &physics["collider_shape"];
        assert_eq!(collider["type"], "ball");
        assert_eq!(collider["radius"], 10.0);
    }

    #[test]
    fn test_error_handling() {
        // Test NotFound error
        let not_found = DatabaseError::NotFound;
        match not_found {
            DatabaseError::NotFound => (),
            _ => panic!("Expected NotFound error"),
        }
        
        // Test HttpError
        let http_error = DatabaseError::HttpError(404);
        match http_error {
            DatabaseError::HttpError(code) => assert_eq!(code, 404),
            _ => panic!("Expected HttpError with code 404"),
        }
    }
} 