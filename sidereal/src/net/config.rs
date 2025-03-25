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
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            protocol_id: DEFAULT_PROTOCOL_ID,
            max_clients: DEFAULT_MAX_CLIENTS,
            available_bytes_per_tick: 60_000,
            channels: vec![ChannelConfig {
                channel_id: 0,
                max_memory_usage_bytes: 5 * 1024 * 1024,
                send_type: SendType::ReliableOrdered { resend_time: Duration::from_millis(200) },
            }],
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