use bevy::prelude::*;
use bevy_replicon_renet2::{
    netcode::{
        ClientAuthentication, NativeSocket, NetcodeClientTransport, NetcodeServerTransport,
        ServerAuthentication, ServerConfig, ServerSetupConfig, ServerSocketConfig,
    },
    renet2::{ChannelConfig, ConnectionConfig, DefaultChannel, RenetClient, RenetServer, SendType},
};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::time::{Duration, SystemTime};

/// Default configuration for the replication system
#[derive(Resource)]
pub struct ReplicationConfig {
    /// Connection configuration
    pub connection_config: ConnectionConfig,
    /// Maximum number of clients that can connect
    pub max_clients: usize,
    /// Protocol ID used to identify the game's protocol
    pub protocol_id: u64,
    /// Server address
    pub server_addr: SocketAddr,
}

impl Default for ReplicationConfig {
    fn default() -> Self {
        Self {
            connection_config: ConnectionConfig {
                // Default channel configuration
                server_channels_config: vec![
                    // Using numeric channel IDs (0, 1, 2) for core channels
                    ChannelConfig {
                        channel_id: 0,                            // Channel for entity updates
                        max_memory_usage_bytes: 10 * 1024 * 1024, // 10 MB
                        send_type: SendType::ReliableOrdered {
                            resend_time: Duration::from_millis(200),
                        },
                    },
                    ChannelConfig {
                        channel_id: 1,                            // Channel for component updates
                        max_memory_usage_bytes: 10 * 1024 * 1024, // 10 MB
                        send_type: SendType::ReliableOrdered {
                            resend_time: Duration::from_millis(200),
                        },
                    },
                    ChannelConfig {
                        channel_id: 2,                           // Channel for events
                        max_memory_usage_bytes: 5 * 1024 * 1024, // 5 MB
                        send_type: SendType::ReliableOrdered {
                            resend_time: Duration::from_millis(200),
                        },
                    },
                    // Custom game channel for unreliable but fast updates
                    ChannelConfig {
                        channel_id: DefaultChannel::Unreliable as u8,
                        max_memory_usage_bytes: 5 * 1024 * 1024, // 5 MB
                        send_type: SendType::Unreliable,
                    },
                ],
                client_channels_config: vec![
                    // Channel for client inputs/commands
                    ChannelConfig {
                        channel_id: DefaultChannel::ReliableOrdered as u8,
                        max_memory_usage_bytes: 5 * 1024 * 1024, // 5 MB
                        send_type: SendType::ReliableOrdered {
                            resend_time: Duration::from_millis(200),
                        },
                    },
                    // Channel for less important client updates
                    ChannelConfig {
                        channel_id: DefaultChannel::Unreliable as u8,
                        max_memory_usage_bytes: 5 * 1024 * 1024, // 5 MB
                        send_type: SendType::Unreliable,
                    },
                ],
                available_bytes_per_tick: 1024 * 64, // 64 KB per tick
            },
            max_clients: 64,
            protocol_id: 0,
            server_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 5678),
        }
    }
}

impl ReplicationConfig {
    /// Create a server configuration from this replication config
    pub fn create_server_config(&self) -> ServerConfig {
        let socket_config = ServerSocketConfig {
            needs_encryption: false,
            public_addresses: vec![self.server_addr],
        };

        ServerConfig {
            current_time: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap(),
            max_clients: self.max_clients as usize,
            protocol_id: self.protocol_id,
            sockets: vec![socket_config],
            authentication: ServerAuthentication::Unsecure,
        }
    }

    /// Create client authentication from this replication config
    pub fn create_client_authentication(&self, client_id: u64) -> ClientAuthentication {
        ClientAuthentication::Unsecure {
            server_addr: self.server_addr,
            client_id,
            user_data: None,
            protocol_id: self.protocol_id,
            socket_id: 0,
        }
    }

    /// Create a test configuration that uses in-memory transport
    pub fn test_config() -> Self {
        Self {
            connection_config: ConnectionConfig {
                server_channels_config: vec![
                    ChannelConfig {
                        channel_id: 0,                       // EntityUpdates
                        max_memory_usage_bytes: 1024 * 1024, // 1 MB for tests
                        send_type: SendType::ReliableOrdered {
                            resend_time: Duration::from_millis(100),
                        },
                    },
                    ChannelConfig {
                        channel_id: 1, // ComponentUpdates
                        max_memory_usage_bytes: 1024 * 1024,
                        send_type: SendType::ReliableOrdered {
                            resend_time: Duration::from_millis(100),
                        },
                    },
                    ChannelConfig {
                        channel_id: 2, // Events
                        max_memory_usage_bytes: 512 * 1024,
                        send_type: SendType::ReliableOrdered {
                            resend_time: Duration::from_millis(100),
                        },
                    },
                    ChannelConfig {
                        channel_id: DefaultChannel::Unreliable as u8,
                        max_memory_usage_bytes: 512 * 1024,
                        send_type: SendType::Unreliable,
                    },
                ],
                client_channels_config: vec![
                    ChannelConfig {
                        channel_id: DefaultChannel::ReliableOrdered as u8,
                        max_memory_usage_bytes: 512 * 1024,
                        send_type: SendType::ReliableOrdered {
                            resend_time: Duration::from_millis(100),
                        },
                    },
                    ChannelConfig {
                        channel_id: DefaultChannel::Unreliable as u8,
                        max_memory_usage_bytes: 512 * 1024,
                        send_type: SendType::Unreliable,
                    },
                ],
                available_bytes_per_tick: 16 * 1024, // 16 KB per tick for tests
            },
            max_clients: 10,
            protocol_id: 1000,
            server_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 0),
        }
    }
}

/// Helper function to create a server with default configuration
pub fn create_default_server(
) -> Result<(RenetServer, NetcodeServerTransport), Box<dyn std::error::Error>> {
    let config = ReplicationConfig::default();
    let server = RenetServer::new(config.connection_config.clone());

    // Create a UDP socket for the server
    let udp_socket = UdpSocket::bind(config.server_addr)?;
    let socket = NativeSocket::new(udp_socket)?;

    // Create server setup config
    let addresses: Vec<Vec<SocketAddr>> = vec![vec![config.server_addr]];
    let server_config = ServerSetupConfig {
        current_time: SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap(),
        max_clients: config.max_clients,
        protocol_id: config.protocol_id,
        socket_addresses: addresses,
        authentication: ServerAuthentication::Unsecure,
    };

    let transport = NetcodeServerTransport::new(server_config, socket)?;

    Ok((server, transport))
}

/// Helper function to create a client with default configuration
pub fn create_default_client(
    client_id: u64,
) -> Result<(RenetClient, NetcodeClientTransport), Box<dyn std::error::Error>> {
    let config = ReplicationConfig::default();
    let client = RenetClient::new(config.connection_config.clone(), false);

    // Create a UDP socket for the client with a random port
    let udp_socket = UdpSocket::bind("0.0.0.0:0")?;
    let socket = NativeSocket::new(udp_socket)?;

    let current_time = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();
    let client_auth = ClientAuthentication::Unsecure {
        server_addr: config.server_addr,
        client_id,
        user_data: None,
        protocol_id: config.protocol_id,
        socket_id: 0,
    };

    let transport = NetcodeClientTransport::new(current_time, client_auth, socket)?;

    Ok((client, transport))
}

/// Helper function to create a server and client pair for testing
/// Note: This function should only be used in test code as it makes
/// networking assumptions that wouldn't be valid in production.
#[cfg(test)]
pub fn create_test_server_client() -> Result<
    (
        (RenetServer, NetcodeServerTransport),
        (RenetClient, NetcodeClientTransport),
    ),
    Box<dyn std::error::Error>,
> {
    // Use loopback address with port 0 to let the OS assign a port
    let test_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 0);
    let mut config = ReplicationConfig::test_config();
    config.server_addr = test_addr;

    // Bind the server socket first to get the assigned port
    let server_udp_socket = UdpSocket::bind(test_addr)?;
    let actual_server_addr = server_udp_socket.local_addr()?;
    let server_socket = NativeSocket::new(server_udp_socket)?;

    // Update the config with the actual server address
    config.server_addr = actual_server_addr;

    // Create server
    let server = RenetServer::new(config.connection_config.clone());

    // Create server setup config
    let addresses: Vec<Vec<SocketAddr>> = vec![vec![config.server_addr]];
    let server_config = ServerSetupConfig {
        current_time: SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap(),
        max_clients: config.max_clients,
        protocol_id: config.protocol_id,
        socket_addresses: addresses,
        authentication: ServerAuthentication::Unsecure,
    };

    let server_transport = NetcodeServerTransport::new(server_config, server_socket)?;

    // Create client
    let client = RenetClient::new(config.connection_config.clone(), false);
    let client_udp_socket = UdpSocket::bind("127.0.0.1:0")?;
    let client_socket = NativeSocket::new(client_udp_socket)?;

    let current_time = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();
    let client_auth = ClientAuthentication::Unsecure {
        server_addr: config.server_addr,
        client_id: 0,
        user_data: None,
        protocol_id: config.protocol_id,
        socket_id: 0,
    };

    let client_transport = NetcodeClientTransport::new(current_time, client_auth, client_socket)?;

    Ok(((server, server_transport), (client, client_transport)))
}
