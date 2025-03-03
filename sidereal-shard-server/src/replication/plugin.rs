use bevy::prelude::*;
use bevy_rapier2d::prelude::Velocity;
use bevy_replicon_renet2::renet2::RenetClient;
use serde::Serialize;
use tracing::{debug, info};

use super::client::{EntityChangeTracker, HandshakeTracker, ShardConnectionState};
use crate::config::{ShardConfig, ShardState};
use sidereal_core::ecs::components::spatial::SpatialTracked;
use sidereal_core::ecs::plugins::replication::network::{NetworkConfig, RepliconClientPlugin};

/// Plugin for the shard server's use of the replication client
pub struct ReplicationPlugin;

impl Plugin for ReplicationPlugin {
    fn build(&self, app: &mut App) {
        info!("Building shard replication plugin");
        // Add the core client plugin with our client ID
        app.insert_resource(NetworkConfig {
            server_address: "127.0.0.1".to_string(),
            port: 5000,
            protocol_id: 1,
            max_clients: 10,
        });
        app.add_plugins(RepliconClientPlugin {
            client_id: 1, // Fixed for simplicity, could use shard_config.client_id
        });
    }
}
