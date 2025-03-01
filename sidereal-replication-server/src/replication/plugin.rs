use bevy::prelude::*;
use tracing::{info, error};
use bevy::math::{Vec2, IVec2};
use bevy_replicon::prelude::*;
use uuid::Uuid;

use crate::scene::SceneState;
use sidereal_core::ecs::plugins::replication::network::{ConnectionConfig, RepliconSetup};

// Network configuration for the replication server
#[derive(Resource, Clone)]
#[allow(dead_code)]
pub struct NetworkConfig {
    pub server_ip: String,
    pub server_port: u16,           // Port 5000 by default, see impl Default below
    pub max_clients: usize,
    pub protocol_id: u64,
    pub server_id: Uuid,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            server_ip: "127.0.0.1".to_string(), // Default to localhost
            server_port: 5000,                  // Default port for replication server
            max_clients: 32,                    // Maximum number of shard servers
            protocol_id: 0,                     // Protocol ID for renet2
            server_id: Uuid::new_v4(),          // Unique ID for this server
        }
    }
}

/// Plugin for handling replication tasks
pub struct ReplicationPlugin;

impl Plugin for ReplicationPlugin {
    fn build(&self, app: &mut App) {
        info!("Building replication plugin");
        
        // Initialize network config (would be loaded from env in production)
        let network_config = app
            .world_mut()
            .get_resource_or_insert_with(NetworkConfig::default)
            .clone();
        
        // Add resources but don't add the client plugin since this is the server
        app.insert_resource(network_config.clone());
        
        // Setup the replication server directly in the plugin build method
        let config = ConnectionConfig {
            server_address: network_config.server_ip.clone(),
            port: network_config.server_port,
            protocol_id: network_config.protocol_id,
            max_clients: network_config.max_clients,
        };
        
        // Setup the server resources using the shared helper from sidereal_core
        match RepliconSetup::setup_server_resources(app, &config) {
            Ok(_) => {
                info!("Replication server started successfully at {}:{}", 
                    network_config.server_ip, network_config.server_port);
            },
            Err(err) => {
                error!("Failed to start replication server: {}", err);
            }
        }
        
        // Register replication events
        app.add_event::<ReplicationEvent>()
           .add_event::<ShardServerConnectionEvent>()
           .add_event::<ClusterAssignmentEvent>()
           .add_event::<EntityTransferEvent>();
        
        // Add systems
        app.add_systems(Update, (
            handle_server_events,
            handle_replication_events,
            process_entity_transfer_requests,
        ).run_if(in_state(SceneState::Ready)));
    }
}

// Events for shard server connections
#[derive(Event)]
#[allow(dead_code)]
pub enum ShardServerConnectionEvent {
    Connected { client_id: ClientId, shard_id: Uuid },
    Disconnected { client_id: ClientId, shard_id: Uuid },
    Authenticated { client_id: ClientId, shard_id: Uuid },
}

// Events for cluster assignment
#[derive(Event)]
#[allow(dead_code)]
pub struct ClusterAssignmentEvent {
    pub shard_id: Uuid,
    pub client_id: ClientId,
    pub clusters: Vec<ClusterAssignment>,
}

#[derive(Clone)]
#[allow(dead_code)]
pub struct ClusterAssignment {
    pub id: Uuid,
    pub base_coordinates: IVec2,
    pub size: IVec2,
}

// Events for entity transfer between shards
#[derive(Event)]
#[allow(dead_code)]
pub enum EntityTransferEvent {
    Request {
        entity_id: Entity,
        source_shard_id: Uuid,
        destination_shard_id: Uuid,
        position: Vec2,
        velocity: Vec2,
    },
    Acknowledge {
        entity_id: Entity,
        destination_shard_id: Uuid,
        transfer_time: f64,
    },
}

/// Events for entity replication
#[derive(Event)]
#[allow(dead_code)]
pub enum ReplicationEvent {
    EntityUpdated {
        entity: Entity,
        cluster_id: Uuid,
    },
    EntityCreated {
        entity: Entity,
        cluster_id: Uuid,
    },
    EntityDeleted {
        entity: Entity,
        cluster_id: Uuid,
    },
}

/// Monitor server events (connect, disconnect, etc.)
fn handle_server_events(
    mut connection_events: EventWriter<ShardServerConnectionEvent>,
    server: Option<Res<RepliconServer>>,
    connected_clients: Option<Res<ConnectedClients>>,
) {
    if let (Some(_server), Some(connected_clients)) = (server, connected_clients) {
        // Process new connections
        for client in connected_clients.iter() {
            connection_events.send(ShardServerConnectionEvent::Connected {
                client_id: client.id(),
                shard_id: Uuid::new_v4(), // In a real implementation, this would be obtained from the client's auth data
            });
        }
    }
}

/// Handle replication events from components
fn handle_replication_events(
    mut events: EventReader<ReplicationEvent>,
    server: Option<Res<RepliconServer>>,
) {
    if let Some(_server) = server {
        for event in events.read() {
            match event {
                ReplicationEvent::EntityUpdated { entity, cluster_id } => {
                    info!("Entity {:?} updated in cluster {}", entity, cluster_id);
                    // In a real implementation, this would send the update to all clients
                    // or target specific clients based on the cluster
                },
                ReplicationEvent::EntityCreated { entity, cluster_id } => {
                    info!("Entity {:?} created in cluster {}", entity, cluster_id);
                    // In a real implementation, we would handle entity creation
                },
                ReplicationEvent::EntityDeleted { entity, cluster_id } => {
                    info!("Entity {:?} deleted in cluster {}", entity, cluster_id);
                    // In a real implementation, we would handle entity deletion
                },
            }
        }
    }
}

/// Process entity transfer requests between shards
fn process_entity_transfer_requests(
    mut transfer_events: EventReader<EntityTransferEvent>,
) {
    for event in transfer_events.read() {
        match event {
            EntityTransferEvent::Request { 
                entity_id, 
                source_shard_id, 
                destination_shard_id, 
                position: _, 
                velocity: _ 
            } => {
                info!(
                    "Entity transfer request: Entity {:?} from shard {} to shard {}", 
                    entity_id, source_shard_id, destination_shard_id
                );
                
                // In a real implementation, we would:
                // 1. Verify the transfer request is valid
                // 2. Coordinate the handover between shards
                // 3. Update entity ownership in the replication system
            },
            EntityTransferEvent::Acknowledge { 
                entity_id, 
                destination_shard_id, 
                transfer_time 
            } => {
                info!(
                    "Entity transfer acknowledged: Entity {:?} to shard {} at time {}", 
                    entity_id, destination_shard_id, transfer_time
                );
                
                // In a real implementation, we would:
                // 1. Complete the transfer process
                // 2. Update entity tracking in the universe state
                // 3. Notify other interested shards
            },
        }
    }
} 