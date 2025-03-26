use bevy::prelude::*;
// Removed RepliconChannelsExt as it's not directly used here anymore
// Keep ChannelConfig, ConnectionConfig, SendType for the helper function
use bevy_replicon_renet2::renet2::{ChannelConfig, ConnectionConfig, SendType};
use std::net::SocketAddr;
use std::time::Duration;
// Removed RepliconChannels import

// --- Constants ---
/// Default protocol ID used across all networking components.
pub const DEFAULT_PROTOCOL_ID: u64 = 7;
/// Default port for the replication server.
pub const DEFAULT_REPLICATION_PORT: u16 = 5000;
/// Default number of max clients for servers (Note: This constant might be used elsewhere, e.g., in ServerSetupConfig).
pub const DEFAULT_MAX_CLIENTS: usize = 64;

// --- Removed NetworkConfig Struct ---

// --- Connection Configuration Helper ---

/// Creates a Renet2 ConnectionConfig with explicitly defined channels compatible with Replicon.
///
/// This provides a stable channel setup regardless of potential changes
/// in Replicon's internal defaults.
pub fn create_stable_connection_config() -> ConnectionConfig {
    // Explicitly define channels matching Replicon's expectations.
    let mut server_channels = Vec::with_capacity(3);
    let mut client_channels = Vec::with_capacity(3);

    // Channel 0: Reliable ordered for entity spawning, despawning, component insertions/removals.
    let reliable_ordered = ChannelConfig {
        channel_id: 0,
        max_memory_usage_bytes: 10 * 1024 * 1024, // 10MB buffer
        send_type: SendType::ReliableOrdered {
            resend_time: Duration::from_millis(200), // Slightly lower resend time
        },
    };
    server_channels.push(reliable_ordered.clone());
    client_channels.push(reliable_ordered);

    // Channel 1: Unreliable unordered for frequent component updates.
    let unreliable_unordered = ChannelConfig {
        channel_id: 1,
        max_memory_usage_bytes: 10 * 1024 * 1024, // 10MB buffer
        send_type: SendType::Unreliable, // Changed from Unreliable for component updates
    };
    server_channels.push(unreliable_unordered.clone());
    client_channels.push(unreliable_unordered);

    // Channel 2: Reliable unordered for server events and client events.
    let reliable_unordered = ChannelConfig {
        channel_id: 2,
        max_memory_usage_bytes: 10 * 1024 * 1024, // 10MB buffer
        send_type: SendType::ReliableUnordered {
            resend_time: Duration::from_millis(200), // Slightly lower resend time
        },
    };
    server_channels.push(reliable_unordered.clone());
    client_channels.push(reliable_unordered);

    ConnectionConfig::from_channels(server_channels, client_channels)
}

// --- Server/Shard Configurations ---

/// Configuration for a shard server.
#[derive(Resource, Debug, Clone)]
pub struct ShardConfig {
    /// The desired **local** address to bind the shard server to (port might be adjusted).
    /// If port is 0, it's assigned dynamically (usually SHARD_PORT_OFFSET + shard_id).
    pub bind_addr: SocketAddr,
    /// The address of the replication server to connect to.
    pub replication_server_addr: SocketAddr,
    /// A unique ID for this shard.
    pub shard_id: u64,
    /// The protocol ID used for networking (must match replication server).
    pub protocol_id: u64,
    // Removed network_config field
}

impl Default for ShardConfig {
    fn default() -> Self {
        Self {
            // Default to loopback, port 0 indicates dynamic assignment later
            bind_addr: "127.0.0.1:0".parse().expect("Invalid default bind_addr"),
            // Default replication server address
            replication_server_addr: format!("127.0.0.1:{}", DEFAULT_REPLICATION_PORT)
                .parse()
                .expect("Invalid default replication_server_addr"),
            shard_id: 1, // Default to shard 1, should be overridden
            protocol_id: DEFAULT_PROTOCOL_ID,
            // Removed network_config assignment
        }
    }
}

/// Configuration for the replication server.
#[derive(Resource, Debug, Clone)]
pub struct ReplicationServerConfig {
    /// The address the replication server should listen on.
    /// Port 0 usually defaults to DEFAULT_REPLICATION_PORT.
    pub bind_addr: SocketAddr,
    /// The protocol ID used for networking (must match shards).
    pub protocol_id: u64,
    // Removed network_config field
}

impl Default for ReplicationServerConfig {
    fn default() -> Self {
        Self {
            // Default to listen on all interfaces (0.0.0.0) on the default port
            bind_addr: format!("0.0.0.0:{}", DEFAULT_REPLICATION_PORT)
                .parse()
                .expect("Invalid default bind_addr"),
            protocol_id: DEFAULT_PROTOCOL_ID,
            // Removed network_config assignment
        }
    }
}

// --- Removed ShardConnections Resource ---