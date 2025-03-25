use bevy::prelude::*;
use bevy_replicon::prelude::*;
use bevy_replicon_renet2::{
    netcode::{ClientAuthentication, NetcodeClientTransport, NativeSocket},
    renet2::{RenetClient, RenetServer},
    RenetChannelsExt
};
use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, SystemTime};
use super::connection::{init_client, init_server};
use super::config::{ShardConfig, ReplicationServerConfig, ShardConnections};

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
    /// Known shard addresses (if this is a replication server)
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
            app.init_resource::<ShardConnections>();
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
            let shard_addresses = self.known_shard_addresses.clone();
            app.add_systems(Startup, move |mut commands: Commands, channels: Res<RepliconChannels>| {
                match init_replication_server(&mut commands, &channels, &config, &shard_addresses) {
                    Ok(_) => info!("Replication server initialized successfully"),
                    Err(e) => error!("Failed to initialize replication server: {}", e),
                }
            });
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
    // Step 1: Initialize the shard as a server
    // This allows it to replicate entities to clients (in this case, the replication server)
    init_server(
        commands,
        config.bind_addr.port(),
        Some(config.protocol_id),
    )?;
    
    // Step 2: Also initialize the shard as a client to the replication server
    // This allows it to receive commands or data from the replication server
    let client_id = config.shard_id;
    init_client(
        commands,
        config.replication_server_addr,
        config.protocol_id,
        client_id,
    )?;
    
    // Store the config for reference
    commands.insert_resource(config.clone());
    
    Ok(())
}

/// Initialize a replication server that connects to each shard
/// This is used to collect component changes from shards
pub fn init_replication_server(
    commands: &mut Commands,
    channels: &RepliconChannels,
    config: &ReplicationServerConfig,
    shard_addresses: &[SocketAddr],
) -> Result<(), Box<dyn std::error::Error>> {
    // Step 1: Initialize as a server for game clients
    init_server(
        commands,
        config.bind_addr.port(),
        Some(config.protocol_id),
    )?;
    
    // Step 2: Create resources to track connections to shards
    let mut shard_connections = ShardConnections::default();
    let mut shard_client_connections = ShardClientConnections::default();
    
    // Step 3: Connect to each shard as a client to receive component changes
    for (idx, shard_addr) in shard_addresses.iter().enumerate() {
        let shard_id = idx as u64 + 1; // Assume sequential shard IDs starting from 1
        let replication_client_id = 10000 + shard_id; // Use a different range of IDs for clarity
        
        // Create socket and initialize connection to the shard
        let socket = UdpSocket::bind("0.0.0.0:0")?;
        let native_socket = NativeSocket::new(socket)?;
        
        // Use the proper channel configuration from Replicon
        let connection_config = config.network_config.to_connection_config();
        
        let authentication = ClientAuthentication::Unsecure {
            client_id: replication_client_id,
            protocol_id: config.protocol_id,
            server_addr: *shard_addr,
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
        shard_client_connections.clients.push((shard_id, client));
        shard_client_connections.transports.push((shard_id, client_transport));
        
        // Track the shard connection
        shard_connections.connected_shards.push(shard_id);
        
        info!("Attempting to connect to shard {} at {}", shard_id, shard_addr);
    }
    
    // Store resources
    commands.insert_resource(shard_connections);
    commands.insert_resource(shard_client_connections);
    commands.insert_resource(config.clone());
    
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
    time: Res<Time>,
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
) {
    for (shard_id, client) in &shard_clients.clients {
        if client.is_connected() {
            info!("Connected to shard {}", shard_id);
        } else if client.is_connecting() {
            debug!("Still connecting to shard {}", shard_id);
        }
    }
} 