use bevy::prelude::*;
use bevy_replicon::prelude::*;
use bevy_replicon_renet2::renet2::{RenetClient, RenetServer};
use std::net::SocketAddr;
use std::time::{Duration, SystemTime};
use super::connection::{init_client, init_server};

/// Configuration for a shard server
#[derive(Resource, Debug, Clone)]
pub struct ShardConfig {
    /// The address to bind the shard server to
    pub bind_addr: SocketAddr,
    /// The address of the replication server
    pub replication_server_addr: SocketAddr,
    /// A unique ID for this shard
    pub shard_id: u64,
    /// The protocol ID used for networking
    pub protocol_id: u64,
}

impl Default for ShardConfig {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1:0".parse().unwrap(), // Dynamic port
            replication_server_addr: "127.0.0.1:5000".parse().unwrap(),
            shard_id: 1,
            protocol_id: 7,
        }
    }
}

/// Configuration for the replication server's connections to shards
#[derive(Resource, Debug, Clone)]
pub struct ReplicationServerConfig {
    /// The address to bind the replication server to
    pub bind_addr: SocketAddr,
    /// The protocol ID used for networking
    pub protocol_id: u64,
}

impl Default for ReplicationServerConfig {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1:5000".parse().unwrap(),
            protocol_id: 7,
        }
    }
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
        app.add_systems(Update, tick_bidirectional_network);
        
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
    )?;
    
    // Step 2: Also initialize the shard as a client to the replication server
    // This allows it to receive commands or data from the replication server
    let client_id = config.shard_id;
    init_client(
        commands,
        channels,
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
    )?;
    
    // Step 2: Connect to each shard as a client
    // This allows receiving component changes from each shard
    for (idx, shard_addr) in shard_addresses.iter().enumerate() {
        let replication_client_id = 10000 + (idx as u64); // Use a different range of IDs for clarity
        
        // In a real implementation, we would create proper resources for each client connection
        // For now, this is a placeholder - we'd need a better way to manage multiple connections
        init_client(
            commands,
            channels,
            *shard_addr,
            config.protocol_id,
            replication_client_id,
        )?;
    }
    
    // Store the config for reference
    commands.insert_resource(config.clone());
    
    Ok(())
}

/// Ticks the network systems for both server and client roles
/// This is needed for bi-directional replication to work
pub fn tick_bidirectional_network(
    time: Res<Time>,
    mut server: Option<ResMut<RenetServer>>,
    mut client: Option<ResMut<RenetClient>>,
) {
    // Use fixed delta time for consistent updates
    let delta = Duration::from_secs_f32(1.0 / 60.0); // 60 FPS fixed timestep
    
    // Tick both server and client if present
    if let Some(ref mut server) = server {
        server.update(delta);
    }
    
    if let Some(ref mut client) = client {
        client.update(delta);
    }
} 