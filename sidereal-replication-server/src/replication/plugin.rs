use bevy::prelude::*;
use tracing::{info, warn, error};
use std::net::{UdpSocket, SocketAddr};
use std::time::Duration;
use bevy::math::{Vec2, IVec2};
use bevy_replicon::prelude::*;
use bevy_replicon_renet2::renet2::{ConnectionConfig, RenetServer};
use bevy_replicon_renet2::netcode::{NetcodeServerTransport, ServerAuthentication, ServerSetupConfig, NativeSocket};
use bevy_replicon_renet2::RenetChannelsExt;
use uuid::Uuid;

use crate::scene::SceneState;
use sidereal_core::ecs::components::*;
use bevy_replicon_renet2::RepliconRenetServerPlugin;
use sidereal_core::ecs::components::spatial::SpatialPosition;
use bevy_rapier2d::prelude::{RigidBody, Velocity, Collider};
use bevy::core::Name;

// Network configuration for the replication server
#[derive(Resource, Clone)]
pub struct NetworkConfig {
    pub server_ip: String,
    pub server_port: u16,
    pub max_clients: usize,
    pub protocol_id: u64,
    pub server_id: Uuid,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            server_ip: "127.0.0.1".to_string(), // Default to localhost
            server_port: 5000,                  // Default port
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
        
        // Configure the server settings
        let server_addr: SocketAddr = format!("{}:{}", network_config.server_ip, network_config.server_port)
            .parse()
            .expect("Failed to parse server address");
        
        // Add the core replication server plugin
        app.add_plugins(ReplicationServerPlugin)
           .insert_resource(network_config);

        // Start the transport
        app.add_systems(Startup, setup_replication_server);
        
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
            coordinate_neighbor_discovery,
            monitor_shard_servers,
        ).run_if(in_state(SceneState::Ready)));
    }
}

// Events for shard server connections
#[derive(Event)]
pub enum ShardServerConnectionEvent {
    Connected { client_id: ClientId, shard_id: Uuid },
    Disconnected { client_id: ClientId, shard_id: Uuid },
    Authenticated { client_id: ClientId, shard_id: Uuid },
}

// Events for cluster assignment
#[derive(Event)]
pub struct ClusterAssignmentEvent {
    pub shard_id: Uuid,
    pub client_id: ClientId,
    pub clusters: Vec<ClusterAssignment>,
}

#[derive(Clone)]
pub struct ClusterAssignment {
    pub id: Uuid,
    pub base_coordinates: IVec2,
    pub size: IVec2,
}

// Events for entity transfer between shards
#[derive(Event)]
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

/// Setup the replication server initially
fn setup_replication_server(
    mut commands: Commands,
    network_config: Res<NetworkConfig>,
    channels: Res<RepliconChannels>,
) {
    info!("Setting up replication server with ID: {}", network_config.server_id);
    
    // Create a UdpSocket and bind it
    let server_addr = format!("{}:{}", network_config.server_ip, network_config.server_port);
    
    // Create socket
    let socket = match UdpSocket::bind(&server_addr) {
        Ok(socket) => socket,
        Err(err) => {
            error!("Failed to bind to {}: {}", server_addr, err);
            return;
        }
    };
    
    // Create and initialize server
    let server = RenetServer::new(
        ConnectionConfig::from_channels(
            channels.get_server_configs(),
            channels.get_client_configs(),
        )
    );
    
    // Create server configuration
    let public_addr = server_addr.parse().expect("Failed to parse server address");
    let server_config = ServerSetupConfig {
        current_time: std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .expect("Time went backwards"),
        max_clients: network_config.max_clients,
        protocol_id: network_config.protocol_id,
        authentication: ServerAuthentication::Unsecure,
        socket_addresses: vec![vec![public_addr]],
    };
    
    // Create transport
    match NativeSocket::new(socket) {
        Ok(native_socket) => {
            match NetcodeServerTransport::new(server_config, native_socket) {
                Ok(transport) => {
                    commands.insert_resource(server);
                    commands.insert_resource(transport);
                    info!("Replication server started successfully on {}", server_addr);
                },
                Err(err) => {
                    error!("Failed to create NetcodeServerTransport: {:?}", err);
                }
            }
        },
        Err(err) => {
            error!("Failed to create NativeSocket: {:?}", err);
        }
    }
}

/// Monitor server events (connect, disconnect, etc.)
fn handle_server_events(
    mut connection_events: EventWriter<ShardServerConnectionEvent>,
    server: Option<Res<RepliconServer>>,
) {
    // Implementation would need to be updated to use the modern renet2 API
    // Here we'll use a placeholder that just indicates what we would do
    if let Some(_server) = server {
        // Process new connections using the renet2 5.0 API
        // This is a simplified placeholder
        
        // Process disconnections using the renet2 5.0 API
        // This is a simplified placeholder
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
    // We would need access to shard assignment information
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

/// Coordinate neighbor discovery for adjacent shards
fn coordinate_neighbor_discovery(
    // We would need access to shard assignments and cluster information
) {
    // In a real implementation, we would:
    // 1. Check for clusters that are adjacent but managed by different shards
    // 2. Send neighbor discovery events to those shards
    // 3. Facilitate direct connection establishment
}

/// Monitor shard server health and status
fn monitor_shard_servers(
    // We would need access to connected clients and their status
) {
    // In a real implementation, we would:
    // 1. Check for heartbeats from connected shard servers
    // 2. Detect and handle shard server failures
    // 3. Reassign clusters if needed
} 