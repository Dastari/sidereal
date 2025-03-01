use bevy::prelude::*;
use bevy_rapier2d::prelude::Velocity;
use tracing::info;
use uuid::Uuid;
use crate::cluster::manager::ClusterManager;
use crate::cluster::boundary::BoundaryEntityRegistry;
use sidereal_core::ecs::components::spatial::{SpatialPosition, UniverseConfig};

// Distance from boundary to consider an entity as "near boundary"
const BOUNDARY_THRESHOLD: f32 = 50.0;

// For compatibility with existing code that might rely on Position
// This will be removed once all code is migrated to use SpatialPosition
pub type Position = SpatialPosition;

// Define CLUSTER_SIZE constant since we can't import it
pub const CLUSTER_SIZE: f32 = 1000.0;

/// System to detect entities near cluster boundaries
pub fn detect_boundary_entities(
    _boundary_registry: ResMut<BoundaryEntityRegistry>,
    cluster_manager: Res<ClusterManager>,
    _query: Query<(Entity, &SpatialPosition, Option<&Velocity>)>,
    _universe_config: Res<UniverseConfig>,
) {
    // Skip if no clusters are assigned
    if cluster_manager.assigned_clusters.is_empty() {
        return;
    }

    // This system is temporarily disabled until we have proper integration with universe_config
    info!("Boundary detection system is disabled temporarily");
}

/// System to handle entities that have crossed cluster boundaries
pub fn handle_cluster_transitions(
    _commands: Commands,
    cluster_manager: ResMut<ClusterManager>,
    _query: Query<(Entity, &SpatialPosition, Option<&Velocity>)>,
) {
    // Skip if no clusters are assigned
    if cluster_manager.assigned_clusters.is_empty() {
        return;
    }
    
    // This system is temporarily disabled until we have proper integration with universe_config
    info!("Cluster transition system is disabled temporarily");
}

/// System to update neighboring clusters information
pub fn update_neighboring_clusters(
    mut cluster_manager: ResMut<ClusterManager>,
) {
    // For each assigned cluster, determine its neighbors
    let assigned_clusters: Vec<IVec2> = cluster_manager.assigned_clusters.keys().cloned().collect();
    
    for cluster_pos in &assigned_clusters {
        // Check all 8 neighboring positions
        for dx in -1..=1 {
            for dy in -1..=1 {
                // Skip the center (current cluster)
                if dx == 0 && dy == 0 {
                    continue;
                }
                
                let neighbor_pos = IVec2::new(cluster_pos.x + dx, cluster_pos.y + dy);
                
                // If this neighbor is not assigned to us, it's a neighboring cluster
                if !cluster_manager.assigned_clusters.contains_key(&neighbor_pos) {
                    // Generate a random UUID for now until we can properly query the cluster service
                    let dummy_uuid = Uuid::nil();
                    cluster_manager.add_neighbor(neighbor_pos, dummy_uuid);
                }
            }
        }
    }
} 