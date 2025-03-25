use super::config::{DEFAULT_PROTOCOL_ID, NetworkConfig};
use bevy::prelude::*;
use bevy_replicon::prelude::*;
use bevy_replicon_renet2::{
    RenetChannelsExt, RepliconRenetPlugins,
    netcode::{
        ClientAuthentication, NativeSocket, NetcodeClientTransport, NetcodeServerTransport,
        ServerAuthentication, ServerSetupConfig,
    },
    renet2::{ChannelConfig, ConnectionConfig, RenetClient, RenetServer, SendType},
};
use std::net::{SocketAddr, UdpSocket};
use std::time::Duration;

pub struct NetworkingPlugin {
    pub server_addr: SocketAddr,
    pub network_config: NetworkConfig,
}

impl Default for NetworkingPlugin {
    fn default() -> Self {
        Self {
            server_addr: "127.0.0.1:5000".parse().unwrap(),
            network_config: NetworkConfig::default(),
        }
    }
}

impl Plugin for NetworkingPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(RepliconRenetPlugins);
        app.insert_resource(self.network_config.clone());
        app.add_systems(Startup, || {
            info!("Networking plugin initialized");
        });
    }
}

/// Creates a connection configuration with Replicon's default channels
fn default_connection_config() -> ConnectionConfig {
    // Create a default network config
    let config = super::config::NetworkConfig::default();

    // Use the stable connection config for guaranteed compatibility
    config.to_stable_connection_config()
}

pub fn init_server(
    commands: &mut Commands,
    server_port: u16,
    protocol_id: Option<u64>,
) -> Result<(), Box<dyn std::error::Error>> {
    let server_addr = format!("0.0.0.0:{}", server_port).parse()?;
    let final_protocol_id = protocol_id.unwrap_or(DEFAULT_PROTOCOL_ID);

    info!(
        "Initializing server at {} with protocol ID {}",
        server_addr, final_protocol_id
    );

    // Bind the socket with specific options
    let socket = UdpSocket::bind(server_addr)?;
    socket.set_nonblocking(true)?;

    info!(
        "Server socket bound successfully to {}",
        socket.local_addr()?
    );

    let native_socket = NativeSocket::new(socket)?;
    info!("Native socket created");

    // Use Replicon's default connection config
    let connection_config = default_connection_config();
    info!("Using Replicon default connection config");

    // Create server config with minimal authentication for maximum compatibility
    let server_config = ServerSetupConfig {
        current_time: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?,
        max_clients: 64,
        protocol_id: final_protocol_id,
        socket_addresses: vec![vec![server_addr]],
        authentication: ServerAuthentication::Unsecure,
    };

    info!(
        "Creating server transport with protocol ID {} and minimal authentication",
        server_config.protocol_id
    );

    let transport = NetcodeServerTransport::new(server_config, native_socket)?;
    info!("Server transport created");

    let server = RenetServer::new(connection_config);
    info!("Server created");

    commands.insert_resource(server);
    commands.insert_resource(transport);
    info!("Server resources inserted");

    Ok(())
}

pub fn init_client(
    commands: &mut Commands,
    server_addr: SocketAddr,
    protocol_id: u64,
    client_id: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    info!(
        "Initializing client with ID {} connecting to {} with protocol ID {}",
        client_id, server_addr, protocol_id
    );

    // Bind to 0.0.0.0 to avoid any specific interface binding issues
    let socket = UdpSocket::bind("0.0.0.0:0")?;

    // Set socket options for better reliability
    socket.set_nonblocking(true)?;

    // Log the local socket address
    info!("Client socket bound to: {}", socket.local_addr()?);

    let native_socket = NativeSocket::new(socket)?;

    // Use default connection config from Replicon
    let connection_config = default_connection_config();

    // Log connection config details
    info!("Using Replicon default connection config");

    // Simplest possible authentication - no user_data
    let authentication = ClientAuthentication::Unsecure {
        client_id,
        protocol_id,
        server_addr,
        user_data: None, // No user data for simplicity
        socket_id: 0,
    };

    // Log authentication details
    info!(
        "Client authentication: client_id={}, protocol_id={}, server_addr={}, no user_data",
        client_id, protocol_id, server_addr
    );

    let transport = NetcodeClientTransport::new(
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?,
        authentication,
        native_socket,
    )?;

    // Create client with standard timeout values
    let client = RenetClient::new(connection_config, false);

    info!("Client transport and client created successfully");

    commands.insert_resource(client);
    commands.insert_resource(transport);

    info!("Client resources inserted");
    Ok(())
}
