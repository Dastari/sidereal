use bevy::prelude::*;
use bevy::math::Vec2;
use bevy_replicon::prelude::*;
use bevy_replicon_renet2::renet2::{self, RenetClient};
use bevy_replicon_renet2::netcode::{self, NetcodeClientTransport, ClientAuthentication};
use bevy_replicon_renet2::RenetChannelsExt;
use renet2_netcode::NativeSocket;
use tracing::{info, warn, error};
use serde::Serialize;
use bevy_rapier2d::prelude::Velocity;
use std::time::{SystemTime, Duration};
use std::net::{Ipv4Addr, UdpSocket};

use crate::config::{ShardConfig, ShardState};
use super::client::ReplicationClient;
use sidereal_core::ecs::components::spatial::SpatialTracked;
use sidereal_core::ecs::plugins::replication::{
    common::{ClientStreamEvent, EntityUpdateType},
    network::{ConnectionConfig as CoreConnectionConfig, RepliconSetup},
};

/// Resource to track when to attempt next reconnection
#[derive(Resource)]
struct NextReconnectAttempt(Option<CoreConnectionConfig>);

/// Resource to store the client ID
#[derive(Resource)]
struct ReplicationClientId(u64);

/// Plugin for the replication client in the shard server
pub struct ReplicationPlugin;

impl Plugin for ReplicationPlugin {
    fn build(&self, app: &mut App) {
        info!("Building shard replication plugin");
        
        // Initialize replicon channels and add the core plugin
        app.add_plugins(sidereal_core::ReplicationClientPlugin);
        
        // Make sure RepliconChannels is initialized
        app.init_resource::<RepliconChannels>();
        
        // Add post-startup system to initialize the connection to the replication server
        app.add_systems(PostStartup, initialize_replication_connection);
        
        // Add system to monitor connection and attempt reconnection if needed
        app.add_systems(Update, check_connection_status);
        
        // Add entity update system
        app.add_systems(Update, send_entity_updates.run_if(in_state(ShardState::Ready)));
    }
}

/// Initializes the connection to the replication server using the shard configuration
fn initialize_replication_connection(
    mut commands: Commands,
    shard_config: Res<ShardConfig>,
    mut client: ResMut<ReplicationClient>,
    time: Res<Time>,
) {
    info!("Initializing connection to replication server at {}:{}", 
        shard_config.replication_server_address, 
        shard_config.replication_server_port);
    
    if client.status == sidereal_core::ecs::plugins::replication::common::ReplicationClientStatus::Disconnected {
        client.status = sidereal_core::ecs::plugins::replication::common::ReplicationClientStatus::Connecting;
        
        // Create our connection config
        let conn_config = CoreConnectionConfig {
            server_address: shard_config.replication_server_address.clone(),
            port: shard_config.replication_server_port,
            protocol_id: shard_config.network_protocol_id,
            max_clients: 1,
        };
        
        client.last_connection_attempt = time.elapsed_secs() as f64;
        
        // Use the shard ID as the client ID
        let client_id = shard_config.shard_id.as_u128() as u64;
        
        commands.insert_resource(NextReconnectAttempt(Some(conn_config)));
        commands.insert_resource(ReplicationClientId(client_id));
    }
}

/// Check connection status and attempt reconnection if needed
fn check_connection_status(
    mut commands: Commands,
    mut client: ResMut<ReplicationClient>,
    mut next_state: ResMut<NextState<ShardState>>,
    mut next_reconnect: Option<ResMut<NextReconnectAttempt>>,
    client_id: Option<Res<ReplicationClientId>>,
    replicon_client: Option<Res<RenetClient>>,
    channels: Option<Res<RepliconChannels>>,
    time: Res<Time>,
) {
    // If we have a pending connection attempt
    if let Some(mut next_reconnect) = next_reconnect {
        if let Some(config) = next_reconnect.0.take() {
            if let Some(client_id) = client_id {
                info!("Attempting to connect to replication server at {}:{}", 
                      config.server_address, config.port);
                
                // 1. Insert RepliconChannels if not already present
                if replicon_client.is_none() {
                    commands.insert_resource(RepliconChannels::default());
                }
                
                // 2. Get the current time
                let current_time = match SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
                    Ok(time) => time,
                    Err(err) => {
                        error!("Failed to get system time: {}", err);
                        client.status = sidereal_core::ecs::plugins::replication::common::ReplicationClientStatus::ConnectionFailed;
                        client.connection_attempts += 1;
                        return;
                    }
                };
                
                // 3. Create the server address
                let server_addr = match format!("{}:{}", config.server_address, config.port).parse() {
                    Ok(addr) => addr,
                    Err(err) => {
                        error!("Failed to parse server address: {}", err);
                        client.status = sidereal_core::ecs::plugins::replication::common::ReplicationClientStatus::ConnectionFailed;
                        client.connection_attempts += 1;
                        return;
                    }
                };
                
                // 4. Bind to any available port
                let socket = match UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)) {
                    Ok(socket) => socket,
                    Err(err) => {
                        error!("Failed to bind client socket: {}", err);
                        client.status = sidereal_core::ecs::plugins::replication::common::ReplicationClientStatus::ConnectionFailed;
                        client.connection_attempts += 1;
                        return;
                    }
                };
                
                // 5. Create the native socket
                let native_socket = match NativeSocket::new(socket) {
                    Ok(socket) => socket,
                    Err(err) => {
                        error!("Failed to create native socket: {}", err);
                        client.status = sidereal_core::ecs::plugins::replication::common::ReplicationClientStatus::ConnectionFailed;
                        client.connection_attempts += 1;
                        return;
                    }
                };
                
                // 6. Create the client authentication
                let authentication = ClientAuthentication::Unsecure {
                    client_id: client_id.0,
                    protocol_id: config.protocol_id,
                    socket_id: 0,
                    server_addr,
                    user_data: None,
                };
                
                // 7. Create the transport
                let transport = match NetcodeClientTransport::new(current_time, authentication, native_socket) {
                    Ok(transport) => transport,
                    Err(err) => {
                        error!("Failed to create netcode transport: {}", err);
                        client.status = sidereal_core::ecs::plugins::replication::common::ReplicationClientStatus::ConnectionFailed;
                        client.connection_attempts += 1;
                        return;
                    }
                };
                
                // 8. Get channel configurations from the resource
                let channel_configs = if let Some(channels) = channels {
                    (channels.get_server_configs(), channels.get_client_configs())
                } else {
                    error!("Failed to get RepliconChannels resource");
                    client.status = sidereal_core::ecs::plugins::replication::common::ReplicationClientStatus::ConnectionFailed;
                    client.connection_attempts += 1;
                    return;
                };
                
                // 9. Create the client with correct channel configurations
                let renet_client = RenetClient::new(
                    renet2::ConnectionConfig::from_channels(
                        channel_configs.0,  // Server configs
                        channel_configs.1   // Client configs
                    ),
                    false, // Don't enable encryption for now
                );
                
                // 10. Insert the resources
                commands.insert_resource(renet_client);
                commands.insert_resource(transport);
                
                // Update client status
                client.status = sidereal_core::ecs::plugins::replication::common::ReplicationClientStatus::Connected;
                        
                // Transition to the next state
                next_state.set(ShardState::Ready);
                
                info!("Connected to replication server");
            }
        }
    }
    
    // Check if we're still connected
    if client.status == sidereal_core::ecs::plugins::replication::common::ReplicationClientStatus::Connected {
        if let Some(replicon_client) = replicon_client {
            if !replicon_client.is_connected() {
                warn!("Lost connection to replication server");
                client.status = sidereal_core::ecs::plugins::replication::common::ReplicationClientStatus::Disconnected;
                client.last_connection_attempt = time.elapsed_secs() as f64;
            }
        }
    }
}

/// Send entity updates to the replication server
fn send_entity_updates(
    mut client: ResMut<ReplicationClient>,
    time: Res<Time>,
    query: Query<(Entity, &Transform, Option<&Velocity>), With<SpatialTracked>>,
    mut client_stream: EventWriter<ClientStreamEvent>,
) {
    if client.status != sidereal_core::ecs::plugins::replication::common::ReplicationClientStatus::Connected {
        return;
    }
    
    // Check if it's time to send updates
    if !client.should_send_entity_updates(&time, 0.1) {
        return;
    }
    
    // Collect entities that need updates
    let mut updates = Vec::new();
    
    for (entity, transform, velocity) in query.iter() {
        // Get velocity as Vec2
        let vel = velocity.map_or(Vec2::ZERO, |v| Vec2::new(v.linvel.x, v.linvel.y));
        
        // Skip entities that haven't changed
        if !client.entity_needs_update(entity, transform, vel) {
            continue;
        }
        
        // Create entity update data
        let position = Vec2::new(transform.translation.x, transform.translation.y);
        
        // Add to updates
        updates.push((entity, position, vel));
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
    }
}

// Entity update data structures
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