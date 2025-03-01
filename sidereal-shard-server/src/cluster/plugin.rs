use bevy::prelude::*;
use tracing::info;
use crate::cluster::manager::ClusterManager;
use crate::cluster::boundary::BoundaryEntityRegistry;
use sidereal_core::ecs::components::spatial::{SpatialTracked, UniverseConfig};

/// Plugin for managing clusters assigned to a shard
pub struct ClusterManagerPlugin;

impl Plugin for ClusterManagerPlugin {
    fn build(&self, app: &mut App) {
        info!("Building cluster manager plugin");
        
        // Initialize resources
        app.init_resource::<ClusterManager>()
           .init_resource::<BoundaryEntityRegistry>()
           .init_resource::<UniverseConfig>(); // Use the universe config from sidereal_core
           
        // Register sidereal_core components
        SpatialTracked::register_required_components(app);
        
        // Add systems
        // Note: Systems are currently commented out until fully implemented with core integration
        // app.add_systems(
        //    Update, 
        //    (
        //        detect_boundary_entities,
        //        handle_cluster_transitions,
        //        update_neighboring_clusters,
        //    ).chain().run_if(in_state(ShardState::Ready))
        // );
        
        info!("Cluster manager plugin ready");
    }
} 