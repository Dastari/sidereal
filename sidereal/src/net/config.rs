use bevy::prelude::*;
use std::net::SocketAddr;
use std::time::Duration;
use bevy_replicon::prelude::*;
use bevy_replicon_renet2::{RenetChannelsExt, renet2::{ChannelConfig, ConnectionConfig, SendType}};

/// Default protocol ID used across all networking components
pub const DEFAULT_PROTOCOL_ID: u64 = 7;

/// Default port for the replication server
pub const DEFAULT_REPLICATION_PORT: u16 = 5000;

/// Default number of max clients for servers
pub const DEFAULT_MAX_CLIENTS: usize = 64;

/// Default networking configuration parameters
#[derive(Resource, Debug, Clone)]
pub struct NetworkConfig {
    /// Protocol ID used for networking (should be the same across all components)
    pub protocol_id: u64,
    /// Maximum number of clients that can connect
    pub max_clients: usize,
    /// Available bytes per tick for network transmission
    pub available_bytes_per_tick: usize,
    /// Channel configurations
    pub channels: Vec<ChannelConfig>,
    /// Keepalive flag
    pub keep_alive: bool,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            protocol_id: DEFAULT_PROTOCOL_ID,
            max_clients: DEFAULT_MAX_CLIENTS,
            available_bytes_per_tick: 60_000,
            channels: Vec::new(), // We'll use Replicon's default channels
            keep_alive: true,
        }
    }
}

impl NetworkConfig {
    /// Creates a ConnectionConfig from this NetworkConfig using Replicon's channel configurations
    pub fn to_connection_config(&self) -> ConnectionConfig {
        // Get default Replicon channels
        let channels = RepliconChannels::default();
        
        // Use the ConnectionConfig::from_channels constructor with the extension trait
        ConnectionConfig::from_channels(
            channels.server_configs(),
            channels.client_configs(),
        )
    }
    
    /// Creates a ConnectionConfig with exactly three channels configured that is fully
    /// compatible with RepliconChannels but more predictable.
    /// This should be used when absolute consistency is required.
    pub fn to_stable_connection_config(&self) -> ConnectionConfig {
        // Create explicitly defined channels that match Replicon's expectations
        // but with explicit, consistent configuration
        let mut server_channels = Vec::new();
        let mut client_channels = Vec::new();
        
        // Channel 0: Reliable for entities (both directions)
        let reliable_ordered = ChannelConfig {
            channel_id: 0,
            max_memory_usage_bytes: 5 * 1024 * 1024, // 5MB
            send_type: SendType::ReliableOrdered { 
                resend_time: Duration::from_millis(300)
            },
        };
        server_channels.push(reliable_ordered.clone());
        client_channels.push(reliable_ordered);
        
        // Channel 1: Unreliable for frequent updates (both directions)
        let unreliable = ChannelConfig {
            channel_id: 1,
            max_memory_usage_bytes: 5 * 1024 * 1024, // 5MB
            send_type: SendType::Unreliable,
        };
        server_channels.push(unreliable.clone());
        client_channels.push(unreliable);
        
        // Add Channel 2: Also needed for Replicon protocol
        let reliable_unordered = ChannelConfig {
            channel_id: 2,
            max_memory_usage_bytes: 5 * 1024 * 1024, // 5MB
            send_type: SendType::ReliableUnordered { 
               resend_time: Duration::from_millis(300)
            },
        };
        server_channels.push(reliable_unordered.clone());
        client_channels.push(reliable_unordered);
        
        // We need to ensure consistent channel IDs
        ConnectionConfig::from_channels(server_channels, client_channels)
    }
}

/// Configuration for a shard server with default values
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
    /// Network configuration
    pub network_config: NetworkConfig,
}

impl Default for ShardConfig {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1:0".parse().unwrap(), // Dynamic port
            replication_server_addr: format!("127.0.0.1:{}", DEFAULT_REPLICATION_PORT).parse().unwrap(),
            shard_id: 1,
            protocol_id: DEFAULT_PROTOCOL_ID,
            network_config: NetworkConfig::default(),
        }
    }
}

/// Configuration for the replication server with default values
#[derive(Resource, Debug, Clone)]
pub struct ReplicationServerConfig {
    /// The address to bind the replication server to
    pub bind_addr: SocketAddr,
    /// The protocol ID used for networking
    pub protocol_id: u64,
    /// Network configuration
    pub network_config: NetworkConfig,
}

impl Default for ReplicationServerConfig {
    fn default() -> Self {
        Self {
            bind_addr: format!("127.0.0.1:{}", DEFAULT_REPLICATION_PORT).parse().unwrap(),
            protocol_id: DEFAULT_PROTOCOL_ID,
            network_config: NetworkConfig::default(),
        }
    }
}

/// Resource to track connections to multiple shards
#[derive(Resource, Default)]
pub struct ShardConnections {
    /// Map of shard IDs to their connection status
    pub connected_shards: Vec<u64>,
} 