use bevy::prelude::*;
use bevy_replicon::prelude::*;
use bevy_replicon_renet2::{
    netcode::{ClientAuthentication, NetcodeClientTransport, NetcodeServerTransport, ServerAuthentication, ServerSetupConfig,NativeSocket}, 
    renet2::{ChannelConfig, ConnectionConfig, RenetClient, RenetServer, SendType }, RepliconRenetPlugins
};
use std::net::{SocketAddr, UdpSocket};
use std::time::Duration;
use super::config::{NetworkConfig, DEFAULT_PROTOCOL_ID};

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

fn default_connection_config() -> ConnectionConfig {
    let channels = vec![ChannelConfig {
        channel_id: 0,
        max_memory_usage_bytes: 5 * 1024 * 1024,
        send_type: SendType::ReliableOrdered { resend_time: Duration::from_millis(200) },
    }];
    ConnectionConfig {
        available_bytes_per_tick: 60_000,
        server_channels_config: channels.clone(),
        client_channels_config: channels,
    }
}

pub fn init_server(
    commands: &mut Commands,
    server_port: u16,
    protocol_id: Option<u64>,
) -> Result<(), Box<dyn std::error::Error>> {
    let server_addr = format!("0.0.0.0:{}", server_port).parse()?;
    let socket = UdpSocket::bind(server_addr)?;
    let native_socket = NativeSocket::new(socket)?;

    let connection_config = default_connection_config();
    let server_config = ServerSetupConfig {
        current_time: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?,
        max_clients: 64,
        protocol_id: protocol_id.unwrap_or(DEFAULT_PROTOCOL_ID),
        socket_addresses: vec![vec![server_addr]],
        authentication: ServerAuthentication::Unsecure,
    };

    let transport = NetcodeServerTransport::new(server_config, native_socket)?;
    let server = RenetServer::new(connection_config);

    commands.insert_resource(server);
    commands.insert_resource(transport);
    Ok(())
}

pub fn init_client(
    commands: &mut Commands,
    server_addr: SocketAddr,
    protocol_id: u64,
    client_id: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    let socket = UdpSocket::bind("0.0.0.0:0")?;
    let native_socket = NativeSocket::new(socket)?;

    let connection_config = default_connection_config();
    let authentication = ClientAuthentication::Unsecure {
        client_id,
        protocol_id,
        server_addr,
        user_data: None,
        socket_id: 0,
    };
    let transport = NetcodeClientTransport::new(
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?,
        authentication,
        native_socket,
    )?;
    let client = RenetClient::new(connection_config, true);

    commands.insert_resource(client);
    commands.insert_resource(transport);
    Ok(())
}