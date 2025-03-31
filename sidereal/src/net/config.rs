// sidereal/src/net/config.rs

use bevy::prelude::*;
#[cfg(feature = "replicon")]
use bevy_replicon_renet2::renet2::{ChannelConfig, ConnectionConfig, SendType};
use std::net::SocketAddr;
use std::time::Duration;
use uuid::Uuid;
use crate::net::shard_communication::REPLICATION_SERVER_SHARD_PORT;

// --- Constants ---
pub const DEFAULT_PROTOCOL_ID: u64 = 7;
pub const DEFAULT_REPLICATION_PORT: u16 = 5000;

// --- Connection Configuration Helper ---

/// Creates a Renet2 ConnectionConfig with explicitly defined channels compatible with Replicon.
#[cfg(feature = "replicon")]
pub fn create_stable_connection_config() -> ConnectionConfig {
    let server_channels_config = vec![
        ChannelConfig {
            // Channel 0: Server Messages (reliable ordered) - Replicon internal
            channel_id: 0,
            max_memory_usage_bytes: 10 * 1024 * 1024,
            send_type: SendType::ReliableOrdered {
                resend_time: Duration::from_millis(200),
            },
        },
        ChannelConfig {
            // Channel 1: Component Changes (unreliable) - Replicon internal
            channel_id: 1,
            max_memory_usage_bytes: 20 * 1024 * 1024,
            send_type: SendType::Unreliable,
        },
        ChannelConfig {
            // Channel 2: Default Channel (reliable unordered) - For user events etc.
            channel_id: 2,
            max_memory_usage_bytes: 10 * 1024 * 1024,
            send_type: SendType::ReliableUnordered {
                resend_time: Duration::from_millis(200),
            },
        },
    ];

    let client_channels_config = vec![
        ChannelConfig {
            // Channel 0: Client Messages (reliable ordered) - Replicon internal
            channel_id: 0,
            max_memory_usage_bytes: 10 * 1024 * 1024,
            send_type: SendType::ReliableOrdered {
                resend_time: Duration::from_millis(200),
            },
        },
        ChannelConfig {
            // Channel 1: Not used by Replicon client internally, but available for user events
            channel_id: 1,
            max_memory_usage_bytes: 10 * 1024 * 1024,
            send_type: SendType::Unreliable, // Keep symmetric or make ReliableOrdered if needed for client->server messages
        },
        ChannelConfig {
            // Channel 2: Default Channel (reliable unordered) - For user events etc.
            channel_id: 2,
            max_memory_usage_bytes: 10 * 1024 * 1024,
            send_type: SendType::ReliableUnordered {
                resend_time: Duration::from_millis(200),
            },
        },
    ];

    // Construct ConnectionConfig directly with the specified channels.
    // DO NOT use ..Default::default() here.
    ConnectionConfig {
        server_channels_config,
        client_channels_config,
        available_bytes_per_tick: 1024 * 1024,
        // heartbeat_time, timeout_time etc. will use their own internal defaults
    }
}

// --- Server/Shard Configurations ---

/// Configuration for a shard server instance.
#[derive(Resource, Debug, Clone)]
pub struct ShardConfig {
    pub bind_addr: SocketAddr,
    pub replication_server_addr: SocketAddr,
    pub shard_id: Uuid,
    pub protocol_id: u64,
}

impl Default for ShardConfig {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1:0".parse().expect("Invalid default bind_addr"),
            replication_server_addr: format!("127.0.0.1:{}", REPLICATION_SERVER_SHARD_PORT)
                .parse()
                .expect("Invalid default replication_server_addr"),
            shard_id: Uuid::new_v4(),
            protocol_id: DEFAULT_PROTOCOL_ID,
        }
    }
}

/// Configuration for the replication server instance.
#[derive(Resource, Debug, Clone)]
pub struct ReplicationServerConfig {
    pub bind_addr: SocketAddr,
    pub protocol_id: u64,
}

impl Default for ReplicationServerConfig {
    fn default() -> Self {
        Self {
            bind_addr: format!("0.0.0.0:{}", DEFAULT_REPLICATION_PORT)
                .parse()
                .expect("Invalid default bind_addr"),
            protocol_id: DEFAULT_PROTOCOL_ID,
        }
    }
}
