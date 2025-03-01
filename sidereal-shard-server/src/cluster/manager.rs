use bevy::prelude::*;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;
use bevy::math::IVec2;

use sidereal_core::ecs::components::spatial::Cluster;

/// Resource for managing clusters assigned to this shard
#[derive(Resource, Default)]
pub struct ClusterManager {
    /// Clusters currently assigned to this shard, keyed by coordinates
    pub assigned_clusters: HashMap<IVec2, Cluster>,
    
    /// Set of entities that are in transition to neighboring clusters
    pub transitioning_entities: HashSet<Entity>,
    
    /// Map of neighboring clusters to their shard IDs
    pub neighboring_clusters: HashMap<IVec2, Uuid>, 
}

impl ClusterManager {
    /// Add a new cluster assignment
    pub fn assign_cluster(&mut self, cluster: Cluster) {
        self.assigned_clusters.insert(cluster.base_coordinates, cluster);
    }
    
    /// Remove a cluster assignment
    pub fn unassign_cluster(&mut self, coordinates: IVec2) -> Option<Cluster> {
        self.assigned_clusters.remove(&coordinates)
    }
    
    /// Mark an entity as being in transition to another cluster
    pub fn add_transitioning_entity(&mut self, entity: Entity) {
        self.transitioning_entities.insert(entity);
    }
    
    /// Mark an entity as no longer transitioning
    pub fn remove_transitioning_entity(&mut self, entity: Entity) {
        self.transitioning_entities.remove(&entity);
    }
    
    /// Add information about a neighboring cluster
    pub fn add_neighbor(&mut self, coordinates: IVec2, shard_id: Uuid) {
        self.neighboring_clusters.insert(coordinates, shard_id);
    }
    
    /// Remove a neighboring cluster
    pub fn remove_neighbor(&mut self, coordinates: IVec2) {
        self.neighboring_clusters.remove(&coordinates);
    }
    
    /// Check if an entity is transitioning
    pub fn is_entity_transitioning(&self, entity: Entity) -> bool {
        self.transitioning_entities.contains(&entity)
    }
    
    /// Get the shard ID responsible for a neighboring cluster
    pub fn get_neighbor_shard(&self, coordinates: IVec2) -> Option<&Uuid> {
        self.neighboring_clusters.get(&coordinates)
    }
    
    /// Check if coordinates are within our assigned clusters
    pub fn is_cluster_assigned(&self, coordinates: IVec2) -> bool {
        self.assigned_clusters.contains_key(&coordinates)
    }
} 