use bevy::prelude::*;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;
use sidereal_core::ecs::components::spatial::BoundaryDirection;

/// Resource to track entities near cluster boundaries that may need to be replicated to neighboring shards
#[derive(Resource, Default)]
pub struct BoundaryEntityRegistry {
    /// Entities near a boundary, with their directions
    pub boundary_entities: HashMap<Entity, HashSet<BoundaryDirection>>,
    
    /// Entities replicated from neighboring shards, grouped by source shard
    pub foreign_entities: HashMap<Uuid, HashSet<Entity>>,
}

impl BoundaryEntityRegistry {
    /// Add an entity to the boundary registry
    pub fn add_boundary_entity(&mut self, entity: Entity, direction: BoundaryDirection) {
        self.boundary_entities
            .entry(entity)
            .or_insert_with(HashSet::new)
            .insert(direction);
    }
    
    /// Remove a direction from an entity's boundary awareness
    pub fn remove_boundary_direction(&mut self, entity: Entity, direction: BoundaryDirection) {
        if let Some(directions) = self.boundary_entities.get_mut(&entity) {
            directions.remove(&direction);
            if directions.is_empty() {
                self.boundary_entities.remove(&entity);
            }
        }
    }
    
    /// Remove an entity from the boundary registry
    pub fn remove_boundary_entity(&mut self, entity: Entity) {
        self.boundary_entities.remove(&entity);
    }
    
    /// Get all directions an entity is close to
    pub fn get_boundary_directions(&self, entity: Entity) -> Option<&HashSet<BoundaryDirection>> {
        self.boundary_entities.get(&entity)
    }
    
    /// Add a foreign entity from a neighboring shard
    pub fn add_foreign_entity(&mut self, entity: Entity, source_shard: Uuid) {
        self.foreign_entities
            .entry(source_shard)
            .or_insert_with(HashSet::new)
            .insert(entity);
    }
    
    /// Remove a foreign entity
    pub fn remove_foreign_entity(&mut self, entity: Entity, source_shard: Uuid) {
        if let Some(entities) = self.foreign_entities.get_mut(&source_shard) {
            entities.remove(&entity);
            if entities.is_empty() {
                self.foreign_entities.remove(&source_shard);
            }
        }
    }
    
    /// Get all foreign entities from a specific shard
    pub fn get_foreign_entities(&self, source_shard: Uuid) -> Option<&HashSet<Entity>> {
        self.foreign_entities.get(&source_shard)
    }
    
    /// Check if an entity is near a specific boundary
    pub fn is_near_boundary(&self, entity: Entity, direction: BoundaryDirection) -> bool {
        self.boundary_entities
            .get(&entity)
            .map(|directions| directions.contains(&direction))
            .unwrap_or(false)
    }
    
    /// Check if an entity is near any boundary
    pub fn is_near_any_boundary(&self, entity: Entity) -> bool {
        self.boundary_entities.contains_key(&entity)
    }
} 