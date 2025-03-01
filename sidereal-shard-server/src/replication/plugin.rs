use bevy::prelude::*;
use bevy::core::Name;
use bevy::math::{Vec2, IVec2};
use bevy_replicon::prelude::*;
use bevy_replicon_renet2::renet2::{ConnectionConfig, RenetClient};
use bevy_replicon_renet2::netcode::{NetcodeClientTransport, ClientAuthentication, NativeSocket};
use bevy_replicon_renet2::RenetChannelsExt;
use tracing::{info, warn, error};
use uuid::Uuid;
use rand::random;
use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket},
    time::{Duration, SystemTime},
    collections::HashSet,
};
use serde::Serialize;

use crate::config::{ShardConfig, ShardState};
use sidereal_core::ecs::plugins::replication::client::ReplicationClientPlugin as CoreReplicationClientPlugin;
use super::client::{ReplicationClient, ReplicationClientStatus};
use sidereal_core::ecs::components::spatial::{SpatialTracked, SpatialPosition};
// Import the core replication plugin
use sidereal_core::ReplicationClientPlugin;
use super::p2p::ShardP2PPlugin;

// Max attempts for connecting to replication server
const MAX_CONNECTION_ATTEMPTS: u32 = 5;
// Base delay between connection attempts (in seconds)
const CONNECTION_RETRY_BASE_DELAY: f64 = 1.0;

/// Event for sending client data to server
#[derive(Event)]
pub struct ClientStreamEvent {
    pub event_type: String,
    pub data: String,
}

/// Plugin for the replication client in the shard server
pub struct ReplicationPlugin;

impl Plugin for ReplicationPlugin {
    fn build(&self, app: &mut App) {
        info!("Building shard replication plugin");
        
        // Add the core replication client plugin
        app.add_plugins(CoreReplicationClientPlugin);
        
        // Add post-startup system to connect to the replication server after all plugins are initialized
        app.add_systems(PostStartup, connect_to_replication_server);
    }
}

/// Connects to the replication server using the shard configuration
fn connect_to_replication_server(
    mut commands: Commands,
    channels: Res<RepliconChannels>,
    shard_config: Res<ShardConfig>,
    mut next_state: ResMut<NextState<ShardState>>,
) {
    info!("Connecting to replication server at {}:{}", 
        shard_config.replication_server_address, 
        shard_config.replication_server_port);
    
    // Create the client with the replicon channels
    let client = RenetClient::new(
        ConnectionConfig::from_channels(
            channels.get_server_configs(), 
            channels.get_client_configs()
        ),
        false, // Don't enable encryption for now
    );

    // Get the current time for the client setup
    let current_time = match SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
        Ok(time) => time,
        Err(err) => {
            error!("Failed to get system time: {}", err);
            return;
        }
    };
    
    // Use the shard ID as the client ID
    let client_id = shard_config.shard_id.as_u128() as u64;
    
    // Create the server address
    let server_addr = match format!("{}:{}", 
        shard_config.replication_server_address, 
        shard_config.replication_server_port
    ).parse() {
        Ok(addr) => addr,
        Err(err) => {
            error!("Failed to parse server address: {}", err);
            return;
        }
    };
    
    // Bind to any available port
    let socket = match UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)) {
        Ok(socket) => socket,
        Err(err) => {
            error!("Failed to bind client socket: {}", err);
            return;
        }
    };
    
    // Create the authentication
    let authentication = ClientAuthentication::Unsecure {
        client_id,
        protocol_id: shard_config.network_protocol_id,
        socket_id: 0,
        server_addr,
        user_data: None,
    };
    
    // Create the native socket
    let native_socket = match NativeSocket::new(socket) {
        Ok(socket) => socket,
        Err(err) => {
            error!("Failed to create native socket: {}", err);
            return;
        }
    };
    
    // Create the transport
    let transport = match NetcodeClientTransport::new(current_time, authentication, native_socket) {
        Ok(transport) => transport,
        Err(err) => {
            error!("Failed to create netcode transport: {}", err);
            return;
        }
    };

    // Insert the client and transport as resources
    commands.insert_resource(client);
    commands.insert_resource(transport);
    
    info!("Connected to replication server as shard {}", shard_config.shard_id);
    
    // Transition to the next state
    next_state.set(ShardState::Ready);
}

/// Monitor connection status and handle reconnection
fn handle_connection_status(
    mut client: ResMut<ReplicationClient>,
    renet_client: Option<Res<RenetClient>>,
    transport: Option<Res<NetcodeClientTransport>>,
    time: Res<Time>,
    mut next_state: ResMut<NextState<ShardState>>,
    shard_config: Res<ShardConfig>,
) {
    let current_time = time.elapsed_secs_f64();
    
    // Handle different connection states
    match client.status {
        ReplicationClientStatus::Disconnected => {
            // Check if we should attempt to reconnect
            let backoff_time = get_backoff_time(client.connection_attempts);
            if current_time - client.last_connection_attempt >= backoff_time {
                info!("Initiating reconnection to replication server (attempt {})", 
                      client.connection_attempts + 1);
                
                // Set status to pending so we don't try again before the system runs
                client.status = ReplicationClientStatus::ConnectionPending;
                next_state.set(ShardState::Connecting);
            }
        },
        ReplicationClientStatus::Connecting => {
            // Check if client and transport exist
            if let (Some(renet_client), Some(transport)) = (renet_client, transport) {
                if renet_client.is_connected() {
                    info!("Successfully connected to replication server");
                    client.status = ReplicationClientStatus::Connected;
                    client.connection_attempts = 0;
                    next_state.set(ShardState::Ready);
                } else if transport.is_connected() {
                    // Client authenticated but not fully connected
                    info!("Authenticated with replication server, completing connection");
                } else if current_time - client.last_connection_attempt > 5.0 {
                    // Connection timeout
                    warn!("Connection attempt timed out");
                    client.status = ReplicationClientStatus::ConnectionFailed;
                    client.connection_attempts += 1;
                }
            }
        },
        ReplicationClientStatus::Connected => {
            // Check if we're still connected
            if let (Some(renet_client), _) = (renet_client, transport) {
                if !renet_client.is_connected() {
                    warn!("Lost connection to replication server");
                    client.status = ReplicationClientStatus::Disconnected;
                    client.last_connection_attempt = current_time;
                    next_state.set(ShardState::Connecting);
                } else if client.should_send_heartbeat(&time, 5.0) {
                    // Send heartbeat
                    info!("Connected to replication server - heartbeat");
                    
                    // Update the last heartbeat time to prevent spamming
                    client.update_heartbeat(&time);
                }
            }
        },
        ReplicationClientStatus::ConnectionFailed => {
            // Check if we should retry
            if client.connection_attempts >= MAX_CONNECTION_ATTEMPTS {
                error!("Failed to connect to replication server after {} attempts", 
                       MAX_CONNECTION_ATTEMPTS);
                next_state.set(ShardState::Error);
            } else {
                let backoff_time = get_backoff_time(client.connection_attempts);
                if current_time - client.last_connection_attempt >= backoff_time {
                    info!("Retrying connection to replication server (attempt {})", 
                          client.connection_attempts + 1);
                    client.status = ReplicationClientStatus::ConnectionPending;
                }
            }
        },
        _ => {}
    }
}

/// Send entity updates to the replication server
fn send_entity_updates(
    mut client: ResMut<ReplicationClient>,
    time: Res<Time>,
    query: Query<(Entity, &Transform, Option<&Velocity>), With<SpatialTracked>>,
    mut client_stream: EventWriter<ClientStreamEvent>,
) {
    if client.status != ReplicationClientStatus::Connected {
        return;
    }
    
    // Check if it's time to send updates
    if !client.should_send_entity_updates(&time, 0.1) {
        return;
    }
    
    // Collect entities that need updates
    let mut updates = Vec::new();
    
    for (entity, transform, velocity) in query.iter() {
        // Skip entities that haven't changed
        if !client.entity_needs_update(entity, transform, velocity) {
            continue;
        }
        
        // Create entity update data
        let position = Vec2::new(transform.translation.x, transform.translation.y);
        let vel = velocity.map_or(Vec2::ZERO, |v| Vec2::new(v.linvel.x, v.linvel.y));
        
        // Add to updates
        updates.push((entity, position, vel));
        
        // Mark as pending in the client
        client.pending_entity_updates.insert(entity);
    }
    
    // Send updates if we have any
    if !updates.is_empty() {
        info!("Sending {} entity updates to replication server", updates.len());
        
        // Create a batch update event
        let update_event = EntityUpdateBatch {
            timestamp: time.elapsed_secs_f64(),
            entities: updates.iter().map(|(entity, pos, vel)| {
                EntityUpdate {
                    entity: *entity,
                    position: *pos,
                    velocity: *vel,
                }
            }).collect(),
        };
        
        // Send the update through the client stream
        client_stream.send(ClientStreamEvent {
            event_type: "entity_updates".to_string(),
            data: serde_json::to_string(&update_event).unwrap_or_default(),
        });
        
        client.update_entity_update_time(&time);
    }
}

// Helper struct for entity updates
#[derive(Serialize)]
struct EntityUpdateBatch {
    timestamp: f64,
    entities: Vec<EntityUpdate>,
}

#[derive(Serialize)]
struct EntityUpdate {
    entity: Entity,
    position: Vec2,
    velocity: Vec2,
}

// Helper to calculate exponential backoff time
fn get_backoff_time(retry_count: u32) -> f64 {
    let base_time = CONNECTION_RETRY_BASE_DELAY;
    let max_time = 60.0;
    let backoff = base_time * (2.0_f64).powi(retry_count as i32);
    backoff.min(max_time)
} 