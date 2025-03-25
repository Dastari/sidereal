use bevy::prelude::*;
use bevy_replicon::prelude::*;
use bevy_replicon_renet2::{
    netcode::{ClientAuthentication, NetcodeClientTransport, NativeSocket},
    renet2::{RenetClient, RenetServer, ServerEvent},
    RenetChannelsExt
};
use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, SystemTime};
use std::collections::HashMap;
use super::connection::{init_client, init_server};
use super::config::{ShardConfig, ReplicationServerConfig, ShardConnections};
use super::utils::find_available_port;

/// A resource to store connections to multiple shards
#[derive(Resource, Default)]
pub struct ShardClientConnections {
    pub clients: Vec<(u64, RenetClient)>,
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
            app.add_systems(Update, tick_shard_clients);
            // Add monitoring system
            app.add_systems(Update, monitor_shard_connections);
            // Add new system to detect shard connections
            app.add_systems(Update, handle_shard_connections);
            // Initialize resources
            app.init_resource::<ShardConnections>();
            app.init_resource::<ShardClientConnections>();
            app.init_resource::<ConnectedShards>();
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
            
            // No longer pre-registering known shard addresses - they will be discovered when shards connect
            if !self.known_shard_addresses.is_empty() {
                warn!("Pre-registering shard addresses is deprecated - shards will now report their addresses when connecting");
            }
        }
    }
}

/// Initialize a shard server that also connects to the replication server
/// Shard is a server for the replication server (sends diffs)
/// And a client of the replication server (receives commands)
pub fn init_shard_server(
    commands: &mut Commands,
    channels: &RepliconChannels,
    config: &ShardConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    // Find an available port if dynamic port is requested
    let bind_addr = if config.bind_addr.port() == 0 {
        // If port is 0, find an available port starting from 5001
        match find_available_port("127.0.0.1", 5001, 100) {
            Some(addr) => {
                info!("Using dynamically assigned port: {}", addr);
                addr
            },
            None => {
                return Err("Failed to find available port for shard server".into());
            }
        }
    } else {
        config.bind_addr
    };
    
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
    let client_id = config.shard_id;
    
    info!("Connecting to replication server at {} as client ID {}", 
        config.replication_server_addr, client_id);
    
    init_client(
        commands,
        config.replication_server_addr,
        config.protocol_id,
        client_id,
    )?;
    
    // Store the updated config with possibly modified bind_addr
    let mut updated_config = config.clone();
    updated_config.bind_addr = bind_addr;
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

/// Establish a reverse connection from the replication server to a shard
pub fn connect_to_shard(
    commands: &mut Commands,
    config: &ReplicationServerConfig,
    shard_id: u64,
    shard_addr: SocketAddr,
    shard_connections: &mut ShardClientConnections,
) -> Result<(), Box<dyn std::error::Error>> {
    // Create a unique client ID for the reverse connection
    let replication_client_id = 10000 + shard_id;
    
    info!("Creating reverse connection to shard {} at {} with client ID {}", 
        shard_id, shard_addr, replication_client_id);
    
    // Create socket and initialize connection to the shard
    let socket = UdpSocket::bind("0.0.0.0:0")?;
    let native_socket = NativeSocket::new(socket)?;
    
    // Use the proper channel configuration from Replicon
    let connection_config = config.network_config.to_connection_config();
    
    let authentication = ClientAuthentication::Unsecure {
        client_id: replication_client_id,
        protocol_id: config.protocol_id,
        server_addr: shard_addr,
        user_data: None,
        socket_id: 0,
    };
    
    let client_transport = NetcodeClientTransport::new(
        SystemTime::now().duration_since(std::time::UNIX_EPOCH)?,
        authentication,
        native_socket,
    )?;
    
    let client = RenetClient::new(connection_config, true);
    
    // Store client and transport in our ShardClientConnections resource
    shard_connections.clients.push((shard_id, client));
    shard_connections.transports.push((shard_id, client_transport));
    
    info!("Establishing reverse connection to shard {} at {}", shard_id, shard_addr);
    
    Ok(())
}

/// Handles connections from shards and establishes reverse connections
pub fn handle_shard_connections(
    mut commands: Commands,
    mut server_events: EventReader<ServerEvent>,
    server: Res<RenetServer>,
    config: Option<Res<ReplicationServerConfig>>,
    mut connected_shards: ResMut<ConnectedShards>,
    mut shard_connections: ResMut<ShardClientConnections>,
) {
    // Skip if no config is available
    let Some(config) = config else { return };
    
    // Process server events to detect connecting/disconnecting shards
    for event in server_events.read() {
        match event {
            ServerEvent::ClientConnected { client_id } => {
                // Check if this is a shard server connecting
                // Shard servers use their shard_id as client_id, which is typically < 1000
                if *client_id < 1000 {
                    let shard_id = *client_id;
                    
                    // In a real production environment, the shard would send its address to the server
                    // For local development, we'll use a convention where:
                    // - If the server can determine the port (e.g., from the packet source), use that
                    // - Otherwise use a convention based on the shard ID (5000 + shard_id)
                    
                    // For now, we're using a port mapping based on shard ID for local development
                    let shard_port = 5000 + shard_id as u16; // e.g., shard 1 = port 5001
                    let addr: SocketAddr = format!("127.0.0.1:{}", shard_port).parse().unwrap();
                    
                    info!("Shard server {} connected - using address {}", shard_id, addr);
                    warn!("Note: Using hardcoded local address - in production, the shard would send its actual address");
                    
                    // Store the shard's address
                    connected_shards.shard_addresses.insert(shard_id, addr);
                    connected_shards.client_to_shard.insert(*client_id, shard_id);
                    
                    // Check if we already have a reverse connection
                    if !connected_shards.reverse_connected.contains(&shard_id) {
                        // Establish reverse connection
                        if let Err(e) = connect_to_shard(&mut commands, &config, shard_id, addr, &mut shard_connections) {
                            error!("Failed to establish reverse connection to shard {}: {}", shard_id, e);
                        } else {
                            connected_shards.reverse_connected.push(shard_id);
                        }
                    }
                } else {
                    // Regular client connected, not a shard
                    info!("Client {} connected to replication server", client_id);
                }
            },
            ServerEvent::ClientDisconnected { client_id, .. } => {
                // Check if this was a shard
                if let Some(shard_id) = connected_shards.client_to_shard.remove(client_id) {
                    info!("Shard {} disconnected from replication server", shard_id);
                    
                    // Remove from reverse_connected list
                    if let Some(index) = connected_shards.reverse_connected.iter().position(|id| *id == shard_id) {
                        connected_shards.reverse_connected.swap_remove(index);
                    }
                    
                    // Note: We're not removing the client/transport from ShardClientConnections here
                    // because they'll be automatically cleaned up when their connection fails
                    // This makes reconnection easier
                } else {
                    // Regular client disconnected
                    info!("Client {} disconnected from replication server", client_id);
                }
            }
        }
    }
}

/// Ticks the network system for the replication server role
pub fn tick_replication_server(
    time: Res<Time>,
    mut server: ResMut<RenetServer>,
) {
    let delta = Duration::from_secs_f32(1.0 / 60.0); // 60 FPS fixed timestep
    server.update(delta);
}

/// Ticks the network system for a shard server role
pub fn tick_shard_server(
    time: Res<Time>,
    mut server: ResMut<RenetServer>,
    mut client: ResMut<RenetClient>,
) {
    let delta = Duration::from_secs_f32(1.0 / 60.0); // 60 FPS fixed timestep
    server.update(delta);
    client.update(delta);
}

/// Ticks all shard clients to receive updates from shards
pub fn tick_shard_clients(
    time: Res<Time>,
    mut shard_clients: ResMut<ShardClientConnections>,
) {
    let delta = Duration::from_secs_f32(1.0 / 60.0); // 60 FPS fixed timestep
    
    for (_, client) in shard_clients.clients.iter_mut() {
        client.update(delta);
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

/// Monitors the connection status of shard clients
pub fn monitor_shard_connections(
    shard_clients: Res<ShardClientConnections>,
    connected_shards: Res<ConnectedShards>,
) {
    for (shard_id, client) in &shard_clients.clients {
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