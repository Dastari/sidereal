use bevy::prelude::*;
use serde_json::json;
use std::sync::{Arc, Mutex};
use uuid::Uuid;
use std::collections::HashMap;

use sidereal_replication_server::database::{EntityRecord, DatabaseResult, DatabaseError};

/// A mock database client for testing
#[derive(Resource)]
pub struct MockDatabaseClient {
    pub base_url: String,
    entities: Arc<Mutex<HashMap<String, EntityRecord>>>,
}

impl MockDatabaseClient {
    /// Create a new mock database client with test data
    pub fn new() -> Self {
        let mut entities = HashMap::new();
        
        // Generate UUID for player entity
        let player_id = Uuid::new_v4().to_string();
        entities.insert(player_id.clone(), EntityRecord {
            id: player_id,
            name: Some("Player".to_string()),
            owner_id: None,
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
                        "type": "ball",
                        "radius": 20.0
                    },
                    "mass": 10.0,
                    "friction": 0.1,
                    "restitution": 0.2,
                    "gravity_scale": 0.0
                }
            }),
            created_at: None,
            updated_at: None,
        });
        
        // Generate UUID for asteroid entity
        let asteroid_id = Uuid::new_v4().to_string();
        entities.insert(asteroid_id.clone(), EntityRecord {
            id: asteroid_id,
            name: Some("Asteroid".to_string()),
            owner_id: None,
            position_x: 100.0,
            position_y: 100.0,
            type_: "asteroid".to_string(),
            components: json!({
                "physics": {
                    "position": [100.0, 100.0],
                    "rotation": 0.3,
                    "rigid_body_type": "dynamic",
                    "velocity": [1.0, -1.0, 0.1],
                    "collider_shape": {
                        "type": "ball",
                        "radius": 30.0
                    },
                    "mass": 50.0,
                    "friction": 0.5,
                    "restitution": 0.7,
                    "gravity_scale": 0.0
                }
            }),
            created_at: None,
            updated_at: None,
        });
        
        // Generate UUID for station entity
        let station_id = Uuid::new_v4().to_string();
        entities.insert(station_id.clone(), EntityRecord {
            id: station_id,
            name: Some("Space Station".to_string()),
            owner_id: None,
            position_x: -200.0,
            position_y: 200.0,
            type_: "station".to_string(),
            components: json!({
                "physics": {
                    "position": [-200.0, 200.0],
                    "rotation": 0.0,
                    "rigid_body_type": "static",
                    "velocity": [0.0, 0.0, 0.0],
                    "collider_shape": {
                        "type": "cuboid",
                        "half_size": [50.0, 50.0]
                    },
                    "mass": 1000.0,
                    "friction": 0.8,
                    "restitution": 0.1,
                    "gravity_scale": 0.0
                }
            }),
            created_at: None,
            updated_at: None,
        });
        
        Self {
            base_url: "http://mock-db-test".to_string(),
            entities: Arc::new(Mutex::new(entities)),
        }
    }
    
    /// Add an entity to the mock database
    pub fn add_entity(&self, entity: EntityRecord) {
        let mut entities = self.entities.lock().unwrap();
        entities.insert(entity.id.clone(), entity);
    }
    
    /// Get entity IDs (for testing verification)
    pub fn get_entity_ids(&self) -> Vec<String> {
        let entities = self.entities.lock().unwrap();
        entities.keys().cloned().collect()
    }
    
    /// Fetches all entities from the database
    pub async fn fetch_all_entities(&self) -> DatabaseResult<Vec<EntityRecord>> {
        let entities = self.entities.lock().unwrap();
        
        // Since EntityRecord doesn't implement Clone, we need to manually copy each field
        let mut result = Vec::new();
        for entity in entities.values() {
            result.push(EntityRecord {
                id: entity.id.clone(),
                name: entity.name.clone(),
                owner_id: entity.owner_id.clone(),
                position_x: entity.position_x,
                position_y: entity.position_y,
                type_: entity.type_.clone(),
                components: entity.components.clone(),
                created_at: entity.created_at.clone(),
                updated_at: entity.updated_at.clone(),
            });
        }
        
        Ok(result)
    }
    
    /// Fetches entities by type from the database
    pub async fn fetch_entities_by_type(&self, entity_type: &str) -> DatabaseResult<Vec<EntityRecord>> {
        let entities = self.entities.lock().unwrap();
        let mut filtered = Vec::new();
        
        for entity in entities.values() {
            if entity.type_ == entity_type {
                filtered.push(EntityRecord {
                    id: entity.id.clone(),
                    name: entity.name.clone(),
                    owner_id: entity.owner_id.clone(),
                    position_x: entity.position_x,
                    position_y: entity.position_y,
                    type_: entity.type_.clone(),
                    components: entity.components.clone(),
                    created_at: entity.created_at.clone(),
                    updated_at: entity.updated_at.clone(),
                });
            }
        }
        
        Ok(filtered)
    }
    
    /// Fetches a single entity by ID from the database
    pub async fn fetch_entity_by_id(&self, entity_id: &str) -> DatabaseResult<EntityRecord> {
        let entities = self.entities.lock().unwrap();
        
        if let Some(entity) = entities.get(entity_id) {
            Ok(EntityRecord {
                id: entity.id.clone(),
                name: entity.name.clone(),
                owner_id: entity.owner_id.clone(),
                position_x: entity.position_x,
                position_y: entity.position_y,
                type_: entity.type_.clone(),
                components: entity.components.clone(),
                created_at: entity.created_at.clone(),
                updated_at: entity.updated_at.clone(),
            })
        } else {
            Err(DatabaseError::NotFound)
        }
    }
    
    /// Creates a new entity in the database
    pub async fn create_entity(&self, entity: &EntityRecord) -> DatabaseResult<()> {
        let mut entities = self.entities.lock().unwrap();
        
        // Make sure ID is a valid UUID
        if Uuid::parse_str(&entity.id).is_err() {
            return Err(DatabaseError::HttpError(400)); // Bad Request
        }
        
        // Manually copy fields from the entity
        entities.insert(entity.id.clone(), EntityRecord {
            id: entity.id.clone(),
            name: entity.name.clone(),
            owner_id: entity.owner_id.clone(),
            position_x: entity.position_x,
            position_y: entity.position_y,
            type_: entity.type_.clone(),
            components: entity.components.clone(),
            created_at: entity.created_at.clone(),
            updated_at: entity.updated_at.clone(),
        });
        
        Ok(())
    }
    
    /// Updates an entity in the database
    pub async fn update_entity(&self, entity_id: &str, entity: &EntityRecord) -> DatabaseResult<()> {
        let mut entities = self.entities.lock().unwrap();
        
        if !entities.contains_key(entity_id) {
            return Err(DatabaseError::NotFound);
        }
        
        // Make sure ID is a valid UUID
        if Uuid::parse_str(&entity.id).is_err() {
            return Err(DatabaseError::HttpError(400)); // Bad Request
        }
        
        // Manually copy fields from the entity
        entities.insert(entity_id.to_string(), EntityRecord {
            id: entity.id.clone(),
            name: entity.name.clone(),
            owner_id: entity.owner_id.clone(),
            position_x: entity.position_x,
            position_y: entity.position_y,
            type_: entity.type_.clone(),
            components: entity.components.clone(),
            created_at: entity.created_at.clone(),
            updated_at: entity.updated_at.clone(),
        });
        
        Ok(())
    }
    
    /// Deletes an entity from the database
    pub async fn delete_entity(&self, entity_id: &str) -> DatabaseResult<()> {
        let mut entities = self.entities.lock().unwrap();
        
        if !entities.contains_key(entity_id) {
            return Err(DatabaseError::NotFound);
        }
        
        entities.remove(entity_id);
        
        Ok(())
    }
}

#[cfg(test)]
mod mock_database_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_fetch_all_entities() {
        let mock_client = MockDatabaseClient::new();
        let entities = mock_client.fetch_all_entities().await.unwrap();
        
        assert_eq!(entities.len(), 3, "Expected 3 entities");
        
        // Validate the IDs are in UUID format
        for entity in &entities {
            assert!(Uuid::parse_str(&entity.id).is_ok(), "Entity ID is not a valid UUID: {}", entity.id);
        }
    }
    
    #[tokio::test]
    async fn test_fetch_entities_by_type() {
        let mock_client = MockDatabaseClient::new();
        
        let players = mock_client.fetch_entities_by_type("player").await.unwrap();
        assert_eq!(players.len(), 1, "Expected 1 player entity");
        assert!(players[0].id.contains('-'), "Player ID should be in UUID format");
        
        let asteroids = mock_client.fetch_entities_by_type("asteroid").await.unwrap();
        assert_eq!(asteroids.len(), 1, "Expected 1 asteroid entity");
        assert!(asteroids[0].id.contains('-'), "Asteroid ID should be in UUID format");
        
        let stations = mock_client.fetch_entities_by_type("station").await.unwrap();
        assert_eq!(stations.len(), 1, "Expected 1 station entity");
        assert!(stations[0].id.contains('-'), "Station ID should be in UUID format");
        
        let unknown = mock_client.fetch_entities_by_type("unknown").await.unwrap();
        assert_eq!(unknown.len(), 0, "Expected 0 unknown entities");
    }
    
    #[tokio::test]
    async fn test_fetch_entity_by_id() {
        let mock_client = MockDatabaseClient::new();
        let all_entities = mock_client.fetch_all_entities().await.unwrap();
        let player_id = &all_entities[0].id;
        
        let entity = mock_client.fetch_entity_by_id(player_id).await.unwrap();
        assert_eq!(entity.id, *player_id);
        
        // The first entity could be any of the three types (player, asteroid, station)
        // since HashMap doesn't guarantee order, so we don't assert on the type
        
        // Validate UUID format
        assert!(Uuid::parse_str(player_id).is_ok(), "Entity ID is not a valid UUID");
        
        let result = mock_client.fetch_entity_by_id("00000000-0000-0000-0000-000000000000").await;
        assert!(result.is_err(), "Expected error for unknown entity");
        
        match result {
            Err(DatabaseError::NotFound) => {}, // Expected
            _ => panic!("Expected NotFound error"),
        }
    }
    
    #[tokio::test]
    async fn test_create_entity() {
        let mock_client = MockDatabaseClient::new();
        
        // Generate a valid UUID for the new entity
        let new_entity_id = Uuid::new_v4().to_string();
        
        let new_entity = EntityRecord {
            id: new_entity_id.clone(),
            name: Some("New Entity".to_string()),
            owner_id: None,
            position_x: 50.0,
            position_y: 50.0,
            type_: "test".to_string(),
            components: json!({}),
            created_at: None,
            updated_at: None,
        };
        
        let result = mock_client.create_entity(&new_entity).await;
        assert!(result.is_ok(), "Failed to create entity");
        
        let entities = mock_client.fetch_all_entities().await.unwrap();
        assert_eq!(entities.len(), 4, "Expected 4 entities after creation");
        
        let created = mock_client.fetch_entity_by_id(&new_entity_id).await.unwrap();
        assert_eq!(created.id, new_entity_id);
        assert_eq!(created.type_, "test");
    }
    
    #[tokio::test]
    async fn test_update_entity() {
        let mock_client = MockDatabaseClient::new();
        let all_entities = mock_client.fetch_all_entities().await.unwrap();
        let player_id = all_entities[0].id.clone();
        
        // Fetch and modify an entity
        let mut player = mock_client.fetch_entity_by_id(&player_id).await.unwrap();
        player.position_x = 50.0;
        player.position_y = 50.0;
        
        let result = mock_client.update_entity(&player_id, &player).await;
        assert!(result.is_ok(), "Failed to update entity");
        
        let updated = mock_client.fetch_entity_by_id(&player_id).await.unwrap();
        assert_eq!(updated.position_x, 50.0);
        assert_eq!(updated.position_y, 50.0);
        
        // Try to update non-existent entity
        let result = mock_client.update_entity("00000000-0000-0000-0000-000000000000", &player).await;
        assert!(result.is_err(), "Expected error when updating unknown entity");
        
        match result {
            Err(DatabaseError::NotFound) => {}
            _ => panic!("Expected NotFound error"),
        }
    }
    
    #[tokio::test]
    async fn test_delete_entity() {
        let mock_client = MockDatabaseClient::new();
        let all_entities = mock_client.fetch_all_entities().await.unwrap();
        let player_id = all_entities[0].id.clone();
        
        let initial_count = mock_client.fetch_all_entities().await.unwrap().len();
        assert_eq!(initial_count, 3, "Expected 3 initial entities");
        
        let result = mock_client.delete_entity(&player_id).await;
        assert!(result.is_ok(), "Failed to delete entity");
        
        let after_delete = mock_client.fetch_all_entities().await.unwrap();
        assert_eq!(after_delete.len(), 2, "Expected 2 entities after deletion");
        
        let result = mock_client.fetch_entity_by_id(&player_id).await;
        assert!(result.is_err(), "Entity should be deleted");
        
        // Try to delete non-existent entity
        let result = mock_client.delete_entity("00000000-0000-0000-0000-000000000000").await;
        assert!(result.is_err(), "Expected error when deleting unknown entity");
        
        match result {
            Err(DatabaseError::NotFound) => {}
            _ => panic!("Expected NotFound error"),
        }
    }
    
    /// Tests generating valid UUIDs
    #[test]
    fn test_uuid_generation() {
        // Generate a UUID using the uuid crate directly
        let uuid = Uuid::new_v4().to_string();
        
        // Verify it's a valid UUID
        assert!(Uuid::parse_str(&uuid).is_ok(), "Generated UUID is not valid: {}", uuid);
        
        // Check that it has the correct format (36 characters with hyphens)
        assert_eq!(uuid.len(), 36);
        assert_eq!(uuid.chars().filter(|&c| c == '-').count(), 4);
    }
} 