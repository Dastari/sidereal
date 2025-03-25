use bevy::prelude::*;
use bevy_replicon::prelude::*;
use bevy_replicon_renet2::{
    netcode::{ClientAuthentication, NativeSocket, NetcodeClientTransport, NetcodeServerTransport},
    renet2::{ConnectionConfig, RenetClient, RenetServer, ServerEvent},
    RenetChannelsExt
};
use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, SystemTime};
use std::collections::{HashMap, HashSet};
use crate::net::{NetworkConfig, DEFAULT_PROTOCOL_ID};

use super::connection::{init_client, init_server};
use super::config::{ShardConfig, ReplicationServerConfig, ShardConnections};
use super::utils::find_available_port;

/// A resource to store shard client data
#[derive(Resource, Default)]
pub struct ShardClients {
    pub clients: Vec<(u64, RenetClient)>,
}

/// A resource to store shard transport data 
#[derive(Resource, Default)]
pub struct ShardTransports {
    pub transports: Vec<(u64, NetcodeClientTransport)>,
}

/// A resource to track established connection from shards to replication server
#[derive(Resource, Default)]
pub struct ConnectedShards {
    /// Map of client IDs to shard IDs
    pub client_to_shard: HashMap<u64, u64>,
    /// Map of shard IDs to their addresses
    pub shard_addresses: HashMap<u64, SocketAddr>,
    /// Set of shards that have established reverse connections
    pub reverse_connected: Vec<u64>,
}

/// For backwards compatibility
/// A resource to store connections to multiple shards
#[derive(Resource, Default)]
pub struct ShardClientConnections {
    pub clients: Vec<(u64, RenetClient)>,
    pub transports: Vec<(u64, NetcodeClientTransport)>,
}

/// Create a bi-directional replication plugin that configures the app
/// based on the roles (replication server, shard server, or both)
pub struct BiDirectionalReplicationSetupPlugin {
    /// The shard configuration (if this is a shard server)
    pub shard_config: Option<ShardConfig>,
    /// The replication server configuration (if this is a replication server)
    pub replication_server_config: Option<ReplicationServerConfig>,
    /// Deprecated: Known shard addresses are now discovered dynamically
    pub known_shard_addresses: Vec<SocketAddr>,
}

impl Default for BiDirectionalReplicationSetupPlugin {
    fn default() -> Self {
        Self {
            shard_config: None,
            replication_server_config: None,
            known_shard_addresses: Vec::new(),
        }
    }
}

impl Plugin for BiDirectionalReplicationSetupPlugin {
    fn build(&self, app: &mut App) {
        // Add appropriate network tick systems based on configuration
        if self.shard_config.is_some() && self.replication_server_config.is_some() {
            app.add_systems(Update, tick_bidirectional);
        } else if self.shard_config.is_some() {
            app.add_systems(Update, tick_shard_server);
        } else if self.replication_server_config.is_some() {
            // Add separate systems
            app.add_systems(Update, tick_replication_server);
            // Add backwards compatibility migration to new resource structure
            app.add_systems(Startup, migrate_to_new_resources);
            // Add client and transport update systems
            app.add_systems(Update, update_shard_clients);
            app.add_systems(Update, update_shard_transports);
            // Add cleanup system
            app.add_systems(Update, cleanup_disconnected_shards.after(update_shard_transports));
            // Add monitoring system
            app.add_systems(Update, monitor_shard_connections);
            // Add new system to detect shard connections
            app.add_systems(Update, handle_shard_connections);
            // Add system to register shards on connection
            app.add_systems(Update, register_shards_on_connection);
            // Initialize resources
            app.init_resource::<ShardConnections>();
            app.init_resource::<ShardClients>();
            app.init_resource::<ShardTransports>();
            app.init_resource::<ConnectedShards>();
            // For backwards compatibility
            app.init_resource::<ShardClientConnections>();
        }
        
        if let Some(shard_config) = &self.shard_config {
            let config = shard_config.clone();
            app.add_systems(Startup, move |mut commands: Commands, channels: Res<RepliconChannels>| {
                match init_shard_server(&mut commands, &channels, &config) {
                    Ok(_) => info!("Shard server initialized successfully"),
                    Err(e) => error!("Failed to initialize shard server: {}", e),
                }
            });
        }
        
        if let Some(replication_server_config) = &self.replication_server_config {
            let config = replication_server_config.clone();
            app.add_systems(Startup, move |mut commands: Commands, channels: Res<RepliconChannels>| {
                match init_replication_server(&mut commands, &channels, &config) {
                    Ok(_) => info!("Replication server initialized successfully"),
                    Err(e) => error!("Failed to initialize replication server: {}", e),
                }
            });
            
            // No longer pre-registering known shard addresses - they will now report their addresses when connecting
            if !self.known_shard_addresses.is_empty() {
                warn!("Pre-registering shard addresses is deprecated - shards will now report their addresses when connecting");
            }
        }
    }
}

/// Initialize a shard server
pub fn init_shard_server(
    commands: &mut Commands,
    channels: &RepliconChannels,
    config: &ShardConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    // Use a predictable port pattern: 5000 + shard_id
    let bind_port = 5000 + config.shard_id as u16;
    
    // Explicitly use 127.0.0.1 to ensure IPv4 connections
    // Avoid 0.0.0.0 as it can cause binding issues on some platforms
    let bind_addr = SocketAddr::new(
        "127.0.0.1".parse().unwrap(),
        bind_port
    );
    
    info!("Initializing shard server {} at {} with protocol ID {}", 
        config.shard_id, bind_addr, config.protocol_id);
    
    // Step 1: Initialize the shard as a server
    // This allows it to replicate entities to clients (in this case, the replication server)
    init_server(
        commands,
        bind_addr.port(),
        Some(config.protocol_id),
    )?;
    
    // Step 2: Also initialize the shard as a client to the replication server
    // This allows it to receive commands or data from the replication server
    // CRUCIAL: Use a different ID range for the client role to avoid ID conflicts
    // Use 20000 + shard_id instead of just shard_id
    let client_id = 20000 + config.shard_id;
    
    // Make sure we're connecting to IPv4 for replication server using exact localhost address
    let replication_server_addr = SocketAddr::new(
        "127.0.0.1".parse().unwrap(), // Ensure IPv4 localhost
        5000 // Always connect to replication server on port 5000
    );
    
    // CRITICAL: Make sure we're using the exact same protocol ID
    let protocol_id = config.protocol_id;
    
    info!("Connecting to replication server at {} as client ID {} with protocol ID {}", 
        replication_server_addr, client_id, protocol_id);
    
    // IMPORTANT: Introduce a small delay between server and client initialization
    // This helps avoid race conditions in socket binding that may cause immediate disconnections
    std::thread::sleep(std::time::Duration::from_millis(100));
    
    init_client(
        commands,
        replication_server_addr,
        protocol_id, // Use the explicit protocol ID
        client_id,
    )?;
    
    // Store the updated config with possibly modified bind_addr
    let mut updated_config = config.clone();
    updated_config.bind_addr = bind_addr;
    updated_config.replication_server_addr = replication_server_addr;  // Use the IPv4 address
    commands.insert_resource(updated_config);
    
    Ok(())
}

/// Initialize a replication server
/// Connections to shards are established dynamically when they connect
pub fn init_replication_server(
    commands: &mut Commands,
    channels: &RepliconChannels,
    config: &ReplicationServerConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    // Initialize as a server for game clients and shard servers
    info!("Initializing replication server at {} with protocol ID {}", 
        config.bind_addr, config.protocol_id);
    
    init_server(
        commands,
        config.bind_addr.port(),
        Some(config.protocol_id),
    )?;
    
    // Store the config for reference
    commands.insert_resource(config.clone());
    
    Ok(())
}

/// Handles connections from shards and establishes reverse connections
pub fn handle_shard_connections(
    time: Res<Time>,
    mut commands: Commands,
    mut server_events: EventReader<ServerEvent>,
    server: Res<RenetServer>,
    config: Option<Res<ReplicationServerConfig>>,
    mut connected_shards: ResMut<ConnectedShards>,
    mut shard_clients: ResMut<ShardClients>,
    mut shard_transports: ResMut<ShardTransports>,
    transport: Res<NetcodeServerTransport>,
) {
    // Use elapsed time to throttle log messages (log only once per second)
    let should_log = time.elapsed().as_secs() % 2 == 0;
    
    for event in server_events.read() {
        match event {
            ServerEvent::ClientConnected { client_id } => {
                // Check if this is a shard connecting
                if let Some(&shard_id) = connected_shards.client_to_shard.get(client_id) {
                    if should_log {
                        info!("Shard {} connected to replication server", shard_id);
                    }
                    
                    // Get the shard's address
                    let shard_addr = connected_shards.shard_addresses.get(&shard_id).cloned();
                    let shard_protocol_id = config.as_ref().map(|c| c.protocol_id).unwrap_or(DEFAULT_PROTOCOL_ID);
                    
                    if let Some(shard_addr) = shard_addr {
                        if should_log {
                            info!("Found shard {} at address {}", shard_id, shard_addr);
                        }
                        
                        // Check if we already have a reverse connection
                        if !connected_shards.reverse_connected.contains(&shard_id) {
                            if should_log {
                                info!("Shard {} doesn't have a reverse connection yet - establishing now", shard_id);
                            }
                            
                            // Establish reverse connection
                            if let Err(e) = connect_to_shard(
                                &mut commands,
                                &config, 
                                shard_id, 
                                shard_addr,
                                shard_protocol_id,
                                &mut shard_clients,
                                &mut shard_transports,
                                should_log
                            ) {
                                error!("Reverse connection failed: {}", e);
                            } else if should_log {
                                info!("Reverse connection to shard {} successfully established", shard_id);
                                connected_shards.reverse_connected.push(shard_id);
                            }
                        } else if should_log {
                            info!("Shard {} already has a reverse connection", shard_id);
                        }
                    } else if should_log {
                        info!("No address found for shard {}", shard_id);
                    }
                } else if should_log {
                    // Regular client connected, not a shard
                    info!("Client {} connected to replication server", client_id);
                }
            },
            ServerEvent::ClientDisconnected { client_id, .. } => {
                // Check if this was a shard
                if let Some(shard_id) = connected_shards.client_to_shard.remove(client_id) {
                    if should_log {
                        info!("Shard {} disconnected from replication server", shard_id);
                    }
                    
                    // Remove from reverse_connected list
                    if let Some(index) = connected_shards.reverse_connected.iter().position(|id| *id == shard_id) {
                        connected_shards.reverse_connected.swap_remove(index);
                        if should_log {
                            info!("Removed shard {} from reverse_connected list", shard_id);
                        }
                    }
                    
                    // Note: We're not removing the client/transport from ShardClients here
                    // because they'll be automatically cleaned up when their connection fails
                    // This makes reconnection easier
                } else if should_log {
                    // Regular client disconnected
                    info!("Client {} disconnected from replication server", client_id);
                }
            }
        }
    }
}

/// Establish a reverse connection from the replication server to a shard
pub fn connect_to_shard(
    commands: &mut Commands,
    config: &Option<Res<ReplicationServerConfig>>,
    shard_id: u64,
    shard_addr: SocketAddr,
    shard_protocol_id: u64,
    shard_clients: &mut ShardClients,
    shard_transports: &mut ShardTransports,
    should_log: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    // Use a different ID range for replication -> shard connections
    let replication_client_id = 10000 + shard_id;
    
    // Log detailed connection information
    if should_log {
        info!("Attempting to establish reverse connection to shard {} at {}", shard_id, shard_addr);
        info!("Using client ID {} and protocol ID {}", replication_client_id, shard_protocol_id);
    }
    
    // Use minimal authentication with no user data for simplicity
    let authentication = ClientAuthentication::Unsecure {
        client_id: replication_client_id,
        protocol_id: shard_protocol_id,
        server_addr: shard_addr,
        user_data: None, // No user data for simplicity
        socket_id: 0,
    };
    
    // Bind to a specific address for the client
    let client_socket_addr = SocketAddr::new("0.0.0.0".parse().unwrap(), 0);
    if should_log {
        info!("Binding reverse connection socket to {}", client_socket_addr);
    }
    
    // Create socket and initialize connection to the shard
    let socket = UdpSocket::bind(client_socket_addr)?;
    socket.set_nonblocking(true)?;
    
    if should_log {
        info!("Reverse connection socket bound to {}", socket.local_addr()?);
    }
    
    let native_socket = NativeSocket::new(socket)?;
    
    // Use our stable connection config for guaranteed compatibility
    let config = crate::net::config::NetworkConfig::default();
    let connection_config = config.to_stable_connection_config();
    
    if should_log {
        info!("Creating transport for reverse connection to shard {}", shard_id);
    }
    
    let current_time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?;
    let transport = NetcodeClientTransport::new(current_time, authentication, native_socket)?;
    
    if should_log {
        info!("Creating client for reverse connection to shard {}", shard_id);
    }
    
    let client = RenetClient::new(connection_config, false);
    
    // Store the client and transport in our collections
    if should_log {
        info!("Storing reverse connection to shard {} in ShardClients", shard_id);
    }
    
    shard_clients.clients.push((shard_id, client));
    shard_transports.transports.push((shard_id, transport));
    
    Ok(())
}

/// Ticks the network system for the replication server role
pub fn tick_replication_server(
    time: Res<Time>,
    mut server: ResMut<RenetServer>,
) {
    let delta = Duration::from_secs_f32(1.0 / 60.0); // 60 FPS fixed timestep
    server.update(delta);
}

/// Ticks the network system for the shard server role
pub fn tick_shard_server(
    time: Res<Time>,
    mut server: ResMut<RenetServer>,
) {
    // Only update the server, DON'T update the transport - that's handled by ClientNetworkPlugin
    let delta = Duration::from_secs_f32(1.0 / 60.0); // 60 FPS fixed timestep
    server.update(delta);
    
    // DO NOT update transport here - let the ClientNetworkPlugin handle that
    // This was causing a conflict with update_client_transport
}

/// Updates all shard clients
pub fn update_shard_clients(
    mut clients: ResMut<ShardClients>,
) {
    let delta = Duration::from_secs_f32(1.0 / 60.0); // 60 FPS fixed timestep
    
    // Update each client
    for (_, client) in &mut clients.clients {
        client.update(delta);
    }
}

/// Updates shard transports one at a time
pub fn update_shard_transports(
    time: Res<Time>,
    mut transports: ResMut<ShardTransports>,
    mut clients: ResMut<ShardClients>,
    mut disconnected: Local<HashSet<u64>>,
) {
    // Clear disconnected list from previous frame
    disconnected.clear();
    
    // Create a temporary map of shard_id -> client_idx
    let client_indices: HashMap<u64, usize> = clients.clients.iter()
        .enumerate()
        .map(|(idx, (shard_id, _))| (*shard_id, idx))
        .collect();
    
    // Process each transport with its matching client
    for (i, (shard_id, transport)) in transports.transports.iter_mut().enumerate() {
        // Find the matching client index
        if let Some(&client_idx) = client_indices.get(shard_id) {
            // Access the client by index
            if client_idx < clients.clients.len() {
                let client = &mut clients.clients[client_idx].1;
                
                // Only update if client is active
                if client.is_connected() || client.is_connecting() {
                    // Update the transport
                    if let Err(e) = transport.update(time.delta(), client) {
                        error!("Transport update error for shard {}: {}", shard_id, e);
                        disconnected.insert(*shard_id);
                    }
                } else {
                    // Client is disconnected
                    disconnected.insert(*shard_id);
                }
            }
        } else {
            // No matching client
            disconnected.insert(*shard_id);
        }
    }
}

/// Resource for tracking disconnected shards
#[derive(Resource, Default)]
pub struct DisconnectedShards(pub HashSet<u64>);

/// Cleans up disconnected shards after transport updates
pub fn cleanup_disconnected_shards(
    mut clients: ResMut<ShardClients>,
    mut transports: ResMut<ShardTransports>,
    disconnected: Local<HashSet<u64>>,
) {
    // Clean up disconnected clients and transports
    for &shard_id in disconnected.iter() {
        // Remove client
        if let Some(client_idx) = clients.clients.iter().position(|(id, _)| *id == shard_id) {
            info!("Cleaning up disconnected client for shard {}", shard_id);
            clients.clients.swap_remove(client_idx);
        }
        
        // Remove transport
        if let Some(transport_idx) = transports.transports.iter().position(|(id, _)| *id == shard_id) {
            transports.transports.swap_remove(transport_idx);
        }
    }
}

/// Ticks the network systems for both server and client roles when running as both
/// replication server and shard server at the same time
pub fn tick_bidirectional(
    _time: Res<Time>,
    mut server: ResMut<RenetServer>,
    mut client: ResMut<RenetClient>,
) {
    let delta = Duration::from_secs_f32(1.0 / 60.0); // 60 FPS fixed timestep
    server.update(delta);
    client.update(delta);
}

/// Migration system to move from old ShardClientConnections to new resources
pub fn migrate_to_new_resources(
    mut old_connections: ResMut<ShardClientConnections>,
    mut clients: ResMut<ShardClients>,
    mut transports: ResMut<ShardTransports>,
) {
    // Move all clients to the new resource
    for (shard_id, client) in std::mem::take(&mut old_connections.clients) {
        clients.clients.push((shard_id, client));
    }
    
    // Move all transports to the new resource
    for (shard_id, transport) in std::mem::take(&mut old_connections.transports) {
        transports.transports.push((shard_id, transport));
    }
    
    info!("Migrated old ShardClientConnections to new split resources");
}

/// Monitors the connection status of shard clients
pub fn monitor_shard_connections(
    clients: Res<ShardClients>,
    connected_shards: Res<ConnectedShards>,
) {
    for (shard_id, client) in &clients.clients {
        if client.is_connected() {
            // Get the shard's address for better logging
            let addr = connected_shards.shard_addresses.get(shard_id)
                .map(|a| a.to_string())
                .unwrap_or_else(|| "unknown".to_string());
                
            debug!("Connected to shard {} at {}", shard_id, addr);
        } else if client.is_connecting() {
            debug!("Still connecting to shard {}", shard_id);
        } else {
            debug!("Not connected to shard {}", shard_id);
        }
    }
}

/// Register shards for replication when they connect
pub fn register_shards_on_connection(
    mut commands: Commands,
    query: Query<Entity, (With<ConnectedClient>, Without<ReplicatedClient>)>,
) {
    for entity in query.iter() {
        info!("Marking shard client {:?} for replication", entity);
        commands.entity(entity).insert(ReplicatedClient);
    }
} 