use crate::scene::SceneState;
use bevy::prelude::*;
use sidereal_core::ecs::components::*;
use tracing::info;
use uuid::Uuid;

use super::systems::*;

/// Plugin for managing the universe partitioning and replication
pub struct UniverseManagerPlugin;

impl Plugin for UniverseManagerPlugin {
    fn build(&self, app: &mut App) {
        info!("Building universe manager plugin");

        // Register component types
        SpatialTracked::register_required_components(app);
        ShadowEntity::register_required_components(app);

        // Register events
        app.add_event::<ClusterManagementMessage>()
            .add_event::<EntityTransitionMessage>()
            .add_event::<EntityApproachingBoundary>();

        // Initialize resources
        app.init_resource::<UniverseConfig>()
            .init_resource::<UniverseState>()
            .init_resource::<ShardServerRegistry>();

        // Add core universe management systems
        app.add_systems(
            Update,
            (
                update_global_universe_state,
                update_entity_sector_coordinates,
                handle_cluster_assignment,
                process_entity_transition_requests,
                send_entity_transition_acknowledgments,
                manage_empty_sectors,
            )
                .chain(),
        );

        // Add initialization system
        app.add_systems(OnEnter(SceneState::Ready), initialize_universe_state);
    }
}

/// Resource for tracking shard server registrations
#[derive(Resource, Default)]
pub struct ShardServerRegistry {
    pub active_shards: Vec<ShardServerInfo>,
}

/// Information about a registered shard server
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct ShardServerInfo {
    pub id: Uuid,
    pub address: String,
    pub port: u16,
    pub entity_capacity: usize,
    pub current_load: usize,
}
