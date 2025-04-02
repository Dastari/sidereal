use super::config::{DEFAULT_PROTOCOL_ID, create_connection_config}; // Adjusted import
use bevy::prelude::*;
#[cfg(feature = "replicon")]
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

/// Plugin for Replicon-based client-server networking
pub struct NetworkingPlugin {
    pub server_addr: SocketAddr,
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
    create_connection_config() // Using the function from config.rs
}



pub fn init_client(
    commands: &mut Commands,
    server_addr: SocketAddr,
    protocol_id: u64,
    client_id: u64,
) -> Result<(), Box<dyn Error>> {
    info!(
        "Initializing game client ID {} connecting to {} with protocol ID {}",
        client_id, server_addr, protocol_id
    );

    let socket = UdpSocket::bind("0.0.0.0:0")?;
    socket.set_nonblocking(true)?;
    let local_addr = socket.local_addr()?;
    info!("Client socket bound to: {}", local_addr);

    let native_socket = NativeSocket::new(socket)?;
    let current_time = SystemTime::now().duration_since(UNIX_EPOCH)?;

    let connection_config = default_connection_config();
    info!("Using Replicon connection configuration for client connection.");

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
