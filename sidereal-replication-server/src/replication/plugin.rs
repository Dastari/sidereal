use bevy::prelude::*;
use tracing::{info, error, warn, debug};
use bevy::math::{Vec2, IVec2};
use bevy_replicon::prelude::*;
use bevy_replicon_renet2::renet2::{self, RenetServer};
use bevy_replicon_renet2::netcode::{NetcodeServerTransport, ServerAuthentication, ServerSetupConfig};
use bevy_replicon_renet2::renet2::ServerEvent;
use bevy_replicon_renet2::RenetChannelsExt;
use uuid::Uuid;
use std::time::{SystemTime, Duration};
use std::collections::HashMap;

use crate::scene::SceneState;
use sidereal_core::ecs::plugins::replication::network::{RepliconSetup, ConnectionConfig as CoreConnectionConfig};

// Custom ClientId type to wrap u64 for type safety
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClientId(u64);

impl ClientId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    pub fn get_raw_id(&self) -> u64 {
        self.0
    }
}

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
            server_ip: "0.0.0.0".to_string(), // Default to all interfaces
            server_port: 5000,                  // Default port for replication server
            max_clients: 32,                    // Maximum number of shard servers
            protocol_id: 0,                     // Use protocol ID 0 to match clients
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
        
        // Initialize replicon channels
        app.init_resource::<RepliconChannels>();

        // Convert our NetworkConfig to core ConnectionConfig
        let core_config = CoreConnectionConfig {
            server_address: network_config.server_ip.clone(),
            port: network_config.server_port,
            protocol_id: network_config.protocol_id,
            max_clients: network_config.max_clients,
        };
        
        // Use the core library's RepliconSetup to handle server setup
        info!("Setting up replication server using core library utilities");
        info!("Server configuration: {}:{}, protocol_id={}, max_clients={}", 
            core_config.server_address, core_config.port, core_config.protocol_id, core_config.max_clients);
            
        match RepliconSetup::setup_server_resources(app, &core_config) {
            Ok(_) => {
                info!("Successfully initialized replication server with core library utilities");
                // Socket timeout configuration - if needed
                if let Some(mut transport) = app.world_mut().get_resource_mut::<NetcodeServerTransport>() {
                    // Any additional transport configuration could go here
                    info!("Server transport configured successfully");
                }
            },
            Err(e) => {
                error!("Failed to initialize replication server: {}", e);
                return;
            }
        }
        
        info!("Replication server started successfully at {}:{}", 
            network_config.server_ip, network_config.server_port);
        
        // Register replication events
        app.add_event::<ReplicationEvent>()
           .add_event::<ShardServerConnectionEvent>()
           .add_event::<ClusterAssignmentEvent>()
           .add_event::<EntityTransferEvent>();
        
        // Add the network update system to PreUpdate to ensure connections are processed
        app.add_systems(PreUpdate, update_server_network);
        
        // Add systems to handle connections and server events
        app.add_systems(Update, (
            handle_server_events,
            handle_server_connection_events,
        ));
        
        // Add system to process handshake messages
        app.add_systems(Update, process_handshake_messages);
        
        // Add system to process client messages
        app.add_systems(Update, process_client_messages);
        
        // Add systems for the Ready state
        app.add_systems(Update, (
            monitor_connections,
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

/// Update the network state - this is crucial to process incoming connections
fn update_server_network(
    mut server: ResMut<RenetServer>,
    mut transport: ResMut<NetcodeServerTransport>,
    _time: Res<Time>,
    _clients: Option<Res<ConnectedClients>>,
    _client_connect_times: Local<HashMap<u64, f64>>,
    _last_log_time: Local<f64>,
) {
    // Get current system time
    let current_time = match SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
        Ok(duration) => duration,
        Err(err) => {
            error!("Failed to get system time for network update: {}", err);
            return;
        }
    };
    
    // Use a static counter to throttle logging (resets every 600 frames ~ 10 seconds at 60fps)
    static mut LAST_CLIENT_COUNT: usize = 0;
    

    // Existing update call
    if let Err(err) = transport.update(current_time, &mut server) {
        let err_str = format!("{:?}", err);
        // Log additional context for connection expired errors
        if err_str.contains("connection expired") {
            warn!("Connection expired error during transport update. This may indicate time synchronization issues between client and server.");
            // Log current system time for debugging
            info!("Server current time: {:?}", current_time);
        } else {
            error!("Transport update error: {:?}", err);
        }
    }
    
    // ALWAYS check for new connections after update
    let new_client_count = server.connected_clients();
    let client_ids = server.clients_id();
    
    // Check if client count changed
    unsafe {
        if new_client_count != LAST_CLIENT_COUNT {
            info!("Client count changed: {} -> {}", LAST_CLIENT_COUNT, new_client_count);
            
            if new_client_count > LAST_CLIENT_COUNT {
                info!("New client(s) connected! Client IDs: {:?}", client_ids);
            } else {
                info!("Client(s) disconnected");
            }
            
            LAST_CLIENT_COUNT = new_client_count;
        }
    }
    
    // Check for messages from connected clients (only if they exist)
    if !client_ids.is_empty() {
        for client_id in client_ids {
            for channel_id in 0..=2 {
                while let Some(message) = server.receive_message(client_id, channel_id) {
                    if let Ok(message_str) = std::str::from_utf8(&message) {
                        info!("Received message from client {} on channel {}: {}", 
                              client_id, channel_id, message_str);
                    } else {
                        info!("Received binary message from client {} on channel {}: {} bytes", 
                              client_id, channel_id, message.len());
                    }
                }
            }
        }
    }
}

/// Monitor active connections for logging purposes
fn monitor_connections(
    clients: Option<Res<ConnectedClients>>,
    time: Res<Time>,
) {
    // Log active connections periodically for monitoring
    if let Some(clients) = clients {
        let elapsed = time.elapsed_secs_f64() as u64;
        if elapsed % 5 == 0 && elapsed > 0 {
            info!("Replication server is running with {} connected clients", clients.len());
            
            // Optionally log each connected client
            for client in clients.iter() {
                info!("  - Connected client: {:?}", client.id());
            }
        }
    }
}

/// Handle server events to track client connections
fn handle_server_events(
    mut connection_events: EventWriter<ShardServerConnectionEvent>,
    server: Option<Res<RenetServer>>,
    connected_clients: Option<Res<ConnectedClients>>,
    mut previous_clients: Local<Vec<bevy_replicon::core::ClientId>>,
) {
    if let (Some(_server), Some(connected_clients)) = (server, connected_clients) {
        // Create a set of current client IDs
        let current_clients: Vec<bevy_replicon::core::ClientId> = connected_clients.iter().map(|c| c.id()).collect();
        
        // Find clients that were present before but aren't now (disconnected)
        for old_client in previous_clients.iter() {
            if !current_clients.contains(old_client) {
                info!("Client {:?} disconnected", old_client);
                
                // In a real implementation, you would look up the shard_id associated with this client
                // For now, generate a random one
                let shard_id = Uuid::new_v4();
                
                // Access raw client ID - using to_bits() which is commonly used in bevy_replicon
                // This will need to be adjusted based on the actual API of bevy_replicon::core::ClientId
                let raw_id = format!("{:?}", old_client)
                    .trim_start_matches("ClientId(")
                    .trim_end_matches(")")
                    .parse::<u64>()
                    .unwrap_or(0);
                
                connection_events.send(ShardServerConnectionEvent::Disconnected {
                    client_id: ClientId::new(raw_id),
                    shard_id,
                });
            }
        }
        
        // Process new connections
        for client in connected_clients.iter() {
            let client_id = client.id();
            
            // Check if this is a new client
            if !previous_clients.contains(&client_id) {
                info!("New connection detected from client ID: {:?}", client_id);
                
                // In a real implementation, the shard_id would be obtained from the client's auth data
                // For now, we're using a random UUID, but this should be the ID sent by the client
                let shard_id = Uuid::new_v4();
                
                info!("Associating client {:?} with shard ID: {}", client_id, shard_id);
                
                // Access raw client ID - using same approach as above
                let raw_id = format!("{:?}", client_id)
                    .trim_start_matches("ClientId(")
                    .trim_end_matches(")")
                    .parse::<u64>()
                    .unwrap_or(0);
                
                connection_events.send(ShardServerConnectionEvent::Connected {
                    client_id: ClientId::new(raw_id),
                    shard_id,
                });
                
                // Log success
                info!("Successfully registered client {:?} with shard ID {}", client_id, shard_id);
            }
        }
        
        // Update previous clients list
        *previous_clients = current_clients;
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

/// Process handshake messages from connected clients
fn process_handshake_messages(
    mut server: ResMut<RenetServer>,
    mut events: EventWriter<ShardServerConnectionEvent>,
) {
    // Get all clients - no need for into_iter().collect()
    // ClientId appears to be u64 in the actual implementation
    let client_ids = server.clients_id();
    
    // Auto-authenticate all connected clients
    for &client_id in &client_ids {
        // There's no is_client_authenticated() method, we'll check if we've already 
        // processed this client by seeing if we've already sent an event
        // Using Debug format for ClientId since it doesn't implement Display
        info!("Auto-authenticating client {:?} (auth disabled)", client_id);
        
        // There's no accept_connection method in RenetServer
        // Instead, we'll just mark the client as authenticated in our own logic
        
        // Generate a random UUID for shard ID for now
        let shard_id = Uuid::new_v4();
        
        // Send welcome message
        let welcome_msg = format!("{{\"type\":\"welcome\",\"server_time\":{},\"message\":\"Authentication disabled, welcome!\"}}", 
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs_f64());
        
        // Send message on channel 0
        if server.can_send_message(client_id, 0, welcome_msg.len()) {
            server.send_message(client_id, 0, welcome_msg.as_bytes().to_vec());
            info!("Sent welcome message to client {:?}", client_id);
        }
        
        // Dispatch connection event
        events.send(ShardServerConnectionEvent::Connected { 
            client_id: ClientId::new(client_id),
            shard_id 
        });
        
        // Immediately dispatch authenticated event
        events.send(ShardServerConnectionEvent::Authenticated { 
            client_id: ClientId::new(client_id),
            shard_id 
        });
    }
}

/// Process client messages from connected clients
fn process_client_messages(
    mut server: Option<ResMut<RenetServer>>,
    clients: Option<Res<ConnectedClients>>,
) {
    if let (Some(mut server), Some(clients)) = (server, clients) {
        for client in clients.iter() {
            let replicon_client_id = client.id();
            
            // Access raw client ID using Debug formatting and parsing
            let raw_client_id = format!("{:?}", replicon_client_id)
                .trim_start_matches("ClientId(")
                .trim_end_matches(")")
                .parse::<u64>()
                .unwrap_or(0);
            
            let client_id = ClientId::new(raw_client_id);
            
            // Check reliable channel (0) for client messages
            let channel_id: u8 = 0;
            
            // Process all messages from this client
            while let Some(message) = server.receive_message(raw_client_id, channel_id) {
                if let Ok(msg_str) = std::str::from_utf8(&message) {
                    info!("Message from client {:?}: {}", client_id, msg_str);
                    
                    // Handle client message 
                    // For example, to respond with an acknowledgment:
                    // let response = "ack".to_string();
                    // let _ = server.send_message(raw_client_id, channel_id, response.into_bytes());
                }
            }
        }
    }
}

/// Handler for server connection events from bevy_replicon_renet2
fn handle_server_connection_events(
    mut server: Option<ResMut<RenetServer>>,
    mut connection_events: EventWriter<ShardServerConnectionEvent>,
    clients: Option<Res<ConnectedClients>>,
    time: Res<Time>,
) {
    if let Some(mut server) = server {
        // Process all connection events in the server
        while let Some(event) = server.get_event() {
            match event {
                ServerEvent::ClientConnected { client_id } => {
                    info!("New client connection: {} at t={:.2}s", client_id, time.elapsed_secs_f64());
                    
                    // For now, we'll generate a random shard_id
                    // In a production system, this would come from auth data
                    let shard_id = Uuid::new_v4();
                    
                    connection_events.send(ShardServerConnectionEvent::Connected {
                        client_id: ClientId::new(client_id),
                        shard_id,
                    });
                },
                ServerEvent::ClientDisconnected { client_id, reason } => {
                    info!("Client disconnected: {}, reason: {:?}", client_id, reason);
                    
                    // We would look up the shard_id from a mapping in a real system
                    let shard_id = Uuid::new_v4();
                    
                    connection_events.send(ShardServerConnectionEvent::Disconnected {
                        client_id: ClientId::new(client_id),
                        shard_id,
                    });
                }
            }
        }
    }
    
    // Log active connections periodically
    let seconds = time.elapsed_secs_f64() as i64;
    if seconds % 5 == 0 && seconds > 0 {
        if let Some(clients) = clients {
            info!("Replication server has {} active connections", clients.len());
        }
    }
} 