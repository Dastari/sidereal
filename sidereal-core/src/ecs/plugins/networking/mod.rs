// sidereal-core/src/ecs/plugins/networking/mod.rs
use std::collections::{HashMap, HashSet};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use bevy::reflect::GetTypeRegistration;

use crate::ecs::plugins::serialization::{EntitySerializer, SerializedEntity, EntitySerializationExt};

// Public modules
pub mod sectors;
pub mod change_tracking;
pub mod messages;

// Re-export types from submodules
pub use sectors::*;
pub use change_tracking::*;
pub use messages::*;

/// Unique identifier for an entity across server boundaries
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NetworkId(pub Uuid);

impl NetworkId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

/// Component to mark entities that should be synchronized between servers
#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub struct Networked {
    /// Last update tick when this entity was modified
    pub last_modified_tick: u64,
    /// Components that have changed since last sync
    pub changed_components: HashSet<String>,
}

/// Identifies a sector in the game world
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Reflect)]
pub struct SectorId {
    pub x: i32,
    pub y: i32,
}

/// Status of a sector
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SectorStatus {
    Unassigned,
    Assigned(ShardServerId),
    Transitioning { from: ShardServerId, to: ShardServerId },
}

/// Unique identifier for a shard server
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ShardServerId(pub Uuid);

impl ShardServerId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

/// Resource to track the current tick/frame number
#[derive(Resource, Default)]
pub struct NetworkTick(pub u64);

/// Component to track which sector an entity belongs to
#[derive(Component, Reflect, Clone)]
#[reflect(Component)]
pub struct EntitySector {
    pub sector: SectorId,
    pub crossing_boundary: bool,
}

/// Message types for server-to-server communication
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum NetworkMessage {
    // Connection management
    ShardRegistration {
        shard_id: ShardServerId,
        capabilities: ShardCapabilities,
    },
    SectorAssignment {
        sectors: Vec<SectorId>,
        entities: Vec<SerializedEntityBinary>,
    },
    
    // Entity updates
    EntityUpdate {
        tick: u64,
        updates: Vec<EntityDelta>,
    },
    EntityTransfer {
        entity: SerializedEntityBinary,
        from_sector: SectorId,
        to_sector: SectorId,
    },
    
    // Synchronization
    Heartbeat {
        tick: u64,
        shard_id: ShardServerId,
    },
    SyncRequest {
        sectors: Vec<SectorId>,
    },
}

/// Shard server capabilities
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ShardCapabilities {
    pub max_entities: usize,
    pub max_sectors: usize,
    pub specialized_systems: Vec<String>,
}

/// Binary serialized entity using JSON
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SerializedEntityBinary {
    pub network_id: NetworkId,
    pub sector: SectorId,
    pub components: HashMap<String, Vec<u8>>, // Binary component data (actually JSON)
}

/// Represents a change to an entity
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EntityDelta {
    pub network_id: NetworkId,
    pub sector: SectorId,
    pub changed_components: HashMap<String, Vec<u8>>, // Only changed components (actually JSON)
    pub removed_components: Vec<String>,              // Components to remove
}

/// Plugin for network entity serialization and tracking
pub struct NetworkEntityPlugin;

impl Plugin for NetworkEntityPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<Networked>()
           .register_type::<EntitySector>()
           .register_type::<SectorId>()
           .init_resource::<NetworkTick>()
           .add_systems(Update, (
               increment_network_tick,
               track_entity_changes,
           ));
    }
}

/// System to increment the network tick each frame
fn increment_network_tick(mut tick: ResMut<NetworkTick>) {
    tick.0 += 1;
}

// Helper trait for entity component manipulation by name
pub trait EntityComponentsExt {
    fn contains_by_name(&self, component_name: &str) -> bool;
}

impl EntityComponentsExt for EntityRef<'_> {
    fn contains_by_name(&self, _component_name: &str) -> bool {
        // This is a simplified implementation - in a real scenario, 
        // you would use the reflection system to check if the component exists
        // For now, just return false to avoid compiler errors
        false
    }
}

// Helper trait for entity component removal by name
pub trait EntityRemoveExt {
    fn remove_by_name(&mut self, component_name: &str);
}

impl EntityRemoveExt for EntityWorldMut<'_> {
    fn remove_by_name(&mut self, _component_name: &str) {
        // This is a simplified implementation - in a real scenario,
        // you would use the reflection system to remove the component
        // For now, just do nothing to avoid compiler errors
    }
}

/// System to track entity component changes
fn track_entity_changes(
    mut query: Query<(Entity, &mut Networked)>,
    tick: Res<NetworkTick>,
    world: &World,
) {
    for (entity, mut networked) in query.iter_mut() {
        if networked.last_modified_tick < tick.0 {
            // Check for changed components
            let type_registry = world.resource::<AppTypeRegistry>().read();
            
            // This would need to be expanded to check all registered components
            // For now, this is a placeholder
            for registration in type_registry.iter() {
                let type_name = registration.type_info().type_path();
                
                // Check if component exists and has changed
                // This is a simplified version - you'd need to implement actual change detection
                if world.entity(entity).contains_by_name(type_name) {
                    networked.changed_components.insert(type_name.to_string());
                }
            }
            
            networked.last_modified_tick = tick.0;
        }
    }
}

/// Trait for serializing entities to binary format
pub trait BinaryEntitySerializer {
    fn serialize_entity_binary(&self, entity: Entity, network_id: NetworkId) -> Result<SerializedEntityBinary, String>;
    fn serialize_entity_delta(&self, entity: Entity, network_id: NetworkId, changed_components: &HashSet<String>) -> Result<EntityDelta, String>;
    fn deserialize_entity_binary(&mut self, serialized: &SerializedEntityBinary) -> Result<(Entity, NetworkId), String>;
    fn apply_entity_delta(&mut self, delta: &EntityDelta) -> Result<Entity, String>;
}

impl BinaryEntitySerializer for World {
    fn serialize_entity_binary(&self, entity: Entity, network_id: NetworkId) -> Result<SerializedEntityBinary, String> {
        let serialized = self.serialize_entity(entity).map_err(|e| e.to_string())?;
        let sector = self.entity(entity)
            .get::<EntitySector>()
            .map(|s| s.sector.clone())
            .unwrap_or(SectorId { x: 0, y: 0 }); // Default sector if not specified

        let mut binary_components = HashMap::new();
        
        // Store JSON components as binary data
        for (component_name, component_json) in serialized.components {
            let component_bytes = serde_json::to_vec(&component_json)
                .map_err(|e| format!("Failed to encode component {}: {}", component_name, e))?;
            
            binary_components.insert(component_name, component_bytes);
        }
        
        Ok(SerializedEntityBinary {
            network_id,
            sector,
            components: binary_components,
        })
    }
    
    fn serialize_entity_delta(&self, entity: Entity, network_id: NetworkId, changed_components: &HashSet<String>) -> Result<EntityDelta, String> {
        let serialized = self.serialize_entity(entity).map_err(|e| e.to_string())?;
        let sector = self.entity(entity)
            .get::<EntitySector>()
            .map(|s| s.sector.clone())
            .unwrap_or(SectorId { x: 0, y: 0 });
            
        let mut binary_changed_components = HashMap::new();
        
        // Only include components that have changed
        for component_name in changed_components {
            if let Some(component_json) = serialized.components.get(component_name) {
                let component_bytes = serde_json::to_vec(component_json)
                    .map_err(|e| format!("Failed to encode component {}: {}", component_name, e))?;
                
                binary_changed_components.insert(component_name.clone(), component_bytes);
            }
        }
        
        Ok(EntityDelta {
            network_id,
            sector,
            changed_components: binary_changed_components,
            removed_components: Vec::new(), // Would need a system to track removed components
        })
    }
    
    fn deserialize_entity_binary(&mut self, serialized: &SerializedEntityBinary) -> Result<(Entity, NetworkId), String> {
        // Convert binary components back to JSON for compatibility with existing deserializer
        let mut json_components = HashMap::new();
        
        for (component_name, component_bytes) in &serialized.components {
            let component_json: serde_json::Value = serde_json::from_slice(component_bytes)
                .map_err(|e| format!("Failed to decode component {}: {}", component_name, e))?;
            
            json_components.insert(component_name.clone(), component_json);
        }
        
        let json_entity = SerializedEntity {
            components: json_components,
        };
        
        let entity = self.deserialize_entity(&json_entity)?;
        
        // Add the NetworkId component
        self.entity_mut(entity).insert(serialized.network_id);
        
        // Add the sector component
        self.entity_mut(entity).insert(EntitySector {
            sector: serialized.sector,
            crossing_boundary: false,
        });
        
        Ok((entity, serialized.network_id))
    }
    
    fn apply_entity_delta(&mut self, delta: &EntityDelta) -> Result<Entity, String> {
        // Find entity by NetworkId
        let entity = self.query_filtered::<Entity, With<NetworkId>>()
            .iter(self)
            .find(|&e| self.entity(e).get::<NetworkId>().unwrap() == &delta.network_id)
            .ok_or_else(|| format!("Entity with NetworkId {:?} not found", delta.network_id))?;
            
        // Update changed components
        for (component_name, component_bytes) in &delta.changed_components {
            let component_json: serde_json::Value = serde_json::from_slice(component_bytes)
                .map_err(|e| format!("Failed to decode component {}: {}", component_name, e))?;
            
            // Use existing deserialization logic to update the component
            let mut temp_entity = SerializedEntity {
                components: HashMap::new(),
            };
            temp_entity.components.insert(component_name.clone(), component_json);
            
            // Apply just this component using custom trait
            self.entity_mut(entity).apply_from_serialized(&temp_entity, component_name)?;
        }
        
        // Remove components if needed - using custom extension trait
        let mut entity_ref = self.entity_mut(entity);
        for component_name in &delta.removed_components {
            entity_ref.remove_by_name(component_name);
        }
        
        // Update sector if needed
        self.entity_mut(entity).insert(EntitySector {
            sector: delta.sector,
            crossing_boundary: false,
        });
        
        Ok(entity)
    }
}

/// Helper trait to apply specific components from a serialized entity
pub trait EntityApplyExt {
    fn apply_from_serialized(&mut self, serialized: &SerializedEntity, component_name: &str) -> Result<(), String>;
}

impl EntityApplyExt for EntityWorldMut<'_> {
    fn apply_from_serialized(&mut self, _serialized: &SerializedEntity, _component_name: &str) -> Result<(), String> {
        // This is a simplified version - the real implementation would use reflection
        // to deserialize and apply the specific component
        
        // Would need to be implemented based on your reflection system
        Ok(())
    }
}

// Extension trait for App to register network-related components
pub trait NetworkingAppExt {
    fn register_networked_component<T>(&mut self) -> &mut Self
    where
        T: Component + Reflect + GetTypeRegistration;
}

impl NetworkingAppExt for App {
    fn register_networked_component<T>(&mut self) -> &mut Self
    where
        T: Component + Reflect + GetTypeRegistration,
    {
        self.register_type::<T>()
            .register_serializable_component::<T>()
    }
}

/// Main plugin for server-to-server networking
pub struct ServerNetworkingPlugin;

impl Plugin for ServerNetworkingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NetworkTick>()
           .register_type::<Networked>()
           .register_type::<EntitySector>()
           .register_type::<SectorId>()
           .add_systems(Update, (
               increment_network_tick,
               track_entity_changes,
           ));
    }
}