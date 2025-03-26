use super::config::{DEFAULT_PROTOCOL_ID, create_stable_connection_config}; // Adjusted import
use bevy::prelude::*;
use bevy_replicon_renet2::{
    RepliconRenetPlugins,
    netcode::{
        ClientAuthentication, NativeSocket, NetcodeClientTransport, NetcodeServerTransport,
        ServerAuthentication, ServerSetupConfig,
    },
    renet2::{ConnectionConfig, RenetClient, RenetServer},
};
use std::{
    error::Error,
    net::{SocketAddr, UdpSocket},
    time::{SystemTime, UNIX_EPOCH},
};
use tracing::info;

pub struct NetworkingPlugin {
    pub server_addr: SocketAddr, // This might be less relevant now, consider removing if unused
}

impl Default for NetworkingPlugin {
    fn default() -> Self {
        Self {
            server_addr: "127.0.0.1:5000"
                .parse()
                .expect("Invalid default server address"),
        }
    }
}

impl Plugin for NetworkingPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(RepliconRenetPlugins);
        info!("Replicon Renet2 networking plugins added.");
    }
}

/// Creates a connection configuration using Replicon's defaults via NetworkConfig.
pub fn default_connection_config() -> ConnectionConfig {
    create_stable_connection_config() // Using the function from config.rs
}

/// Initializes Renet server resources.
pub fn init_server(
    commands: &mut Commands,
    server_port: u16,
    protocol_id: Option<u64>,
) -> Result<(), Box<dyn Error>> {
    let listen_addr = SocketAddr::new("0.0.0.0".parse().unwrap(), server_port);
    let public_addr = listen_addr; // Adjust if behind NAT
    let final_protocol_id = protocol_id.unwrap_or(DEFAULT_PROTOCOL_ID);

    info!(
        "Initializing server on {} (public {}) with protocol ID {}",
        listen_addr, public_addr, final_protocol_id
    );

    let socket = UdpSocket::bind(listen_addr)?;
    socket.set_nonblocking(true)?;
    let local_addr = socket.local_addr()?;
    info!("Server socket bound successfully to {}", local_addr);

    let native_socket = NativeSocket::new(socket)?;
    let current_time = SystemTime::now().duration_since(UNIX_EPOCH)?;

    let connection_config = default_connection_config();
    info!("Using custom stable connection configuration."); // Updated log message

    let server_config = ServerSetupConfig {
        current_time,
        max_clients: 64, // Consider making this configurable
        protocol_id: final_protocol_id,
        socket_addresses: vec![vec![public_addr]],
        authentication: ServerAuthentication::Unsecure,
    };
    info!(
        "Server configured for {} max clients.",
        server_config.max_clients
    );

    let transport = NetcodeServerTransport::new(server_config, native_socket)?;
    info!("Netcode server transport created.");

    let server = RenetServer::new(connection_config);
    info!("Renet server created.");

    commands.insert_resource(server);
    commands.insert_resource(transport);
    info!("Server resources inserted.");

    Ok(())
}

/// Initializes Renet client resources.
pub fn init_client(
    commands: &mut Commands,
    server_addr: SocketAddr,
    protocol_id: u64,
    client_id: u64,
) -> Result<(), Box<dyn Error>> {
    info!(
        "Initializing client ID {} connecting to {} with protocol ID {}",
        client_id, server_addr, protocol_id
    );

    let socket = UdpSocket::bind("0.0.0.0:0")?;
    socket.set_nonblocking(true)?;
    let local_addr = socket.local_addr()?;
    info!("Client socket bound to: {}", local_addr);

    let native_socket = NativeSocket::new(socket)?;
    let current_time = SystemTime::now().duration_since(UNIX_EPOCH)?;

    let connection_config = default_connection_config();
    info!("Using custom stable channel configuration."); // Updated log message

    let authentication = ClientAuthentication::Unsecure {
        client_id,
        protocol_id,
        server_addr,
        user_data: None,
        socket_id: 0,
    };
    info!(
        "Client authentication configured: client_id={}, protocol_id={}, server_addr={}",
        client_id, protocol_id, server_addr
    );

    let transport = NetcodeClientTransport::new(current_time, authentication, native_socket)?;
    info!("Netcode client transport created.");

    // The `drop_packets` argument is `false` by default, explicitly setting it for clarity.
    let client = RenetClient::new(connection_config, false);
    info!("Renet client created.");

    commands.insert_resource(client);
    commands.insert_resource(transport);
    info!("Client resources inserted.");

    Ok(())
}
