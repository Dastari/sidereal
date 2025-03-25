use bevy::prelude::*;
use bevy_replicon::prelude::*;
use bevy_replicon_renet2::{
    netcode::{ClientAuthentication, NetcodeClientTransport, NetcodeServerTransport, ServerAuthentication, ServerSetupConfig,NativeSocket}, 
    renet2::{ChannelConfig, ConnectionConfig, RenetClient, RenetServer, SendType }, RepliconRenetPlugins
};
use std::net::{SocketAddr, UdpSocket};
use std::time::Duration;

pub struct NetworkingPlugin {
    pub server_addr: SocketAddr,
}

impl Default for NetworkingPlugin {
    fn default() -> Self {
        Self {
            server_addr: "127.0.0.1:5000".parse().unwrap(),
        }
    }
}

impl Plugin for NetworkingPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(RepliconRenetPlugins);
        app.add_systems(Startup, || {
            info!("Networking plugin initialized");
        });
    }
}

pub fn init_server(
    commands: &mut Commands,
    server_port: u16,
) -> Result<(), Box<dyn std::error::Error>> {
    // Bind the socket to the server address
    let server_addr = format!("0.0.0.0:{}", server_port).parse()?;
    let socket = UdpSocket::bind(server_addr)?;

    let native_socket = NativeSocket::new(socket)?;

    let server_channels = vec![ChannelConfig {
        channel_id: 0,
        max_memory_usage_bytes: 5 * 1024 * 1024,
        send_type: SendType::ReliableOrdered { resend_time: Duration::from_millis(200) },
    }];
    let client_channels = server_channels.clone();
    
    let connection_config = ConnectionConfig {
        available_bytes_per_tick: 60_000,
        server_channels_config: server_channels,
        client_channels_config: client_channels,
    };

    // Provide values for ServerSetupConfig
    let current_time = Duration::from_secs(0); // Initial time
    let socket_addresses = vec![vec![server_addr]]; // Single address for the server

    let server_config = ServerSetupConfig {
        current_time,
        max_clients: 64, // Example value
        protocol_id: 0, // Example protocol ID
        socket_addresses,
        authentication: ServerAuthentication::Unsecure, // Example authentication
    };

    // Create the transport and server
    let transport = NetcodeServerTransport::new(server_config, native_socket)?;
    let server = RenetServer::new(connection_config);

    // Insert resources into Bevy
    commands.insert_resource(server);
    commands.insert_resource(transport);

    Ok(())
}

pub fn init_client(
    commands: &mut Commands,
    channels: &RepliconChannels,
    server_addr: SocketAddr,
    protocol_id: u64,
    client_id: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    let socket = UdpSocket::bind("0.0.0.0:0")?;
    // Convert to NativeSocket for use with NetcodeClientTransport
    let native_socket = NativeSocket::new(socket)?;

    let server_channels = vec![ChannelConfig {
        channel_id: 0,
        max_memory_usage_bytes: 5 * 1024 * 1024,
        send_type: SendType::ReliableOrdered { resend_time: Duration::from_millis(200) },
    }];
    let client_channels = server_channels.clone();
    let connection_config = ConnectionConfig {
        available_bytes_per_tick: 60_000,
        server_channels_config: server_channels,
        client_channels_config: client_channels,
    };

    let authentication = ClientAuthentication::Unsecure {
        client_id,
        protocol_id,
        server_addr,
        user_data: None,
        socket_id: 0, // Add the missing socket_id field
    };
    let transport = NetcodeClientTransport::new(Duration::from_secs(0), authentication, native_socket)?;
    let client = RenetClient::new(connection_config, true);

    commands.insert_resource(client);
    commands.insert_resource(transport);
    Ok(())
}