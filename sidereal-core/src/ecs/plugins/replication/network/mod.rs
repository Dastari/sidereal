use bevy::prelude::*;
use bevy_replicon::prelude::*;
use bevy_replicon_renet2::renet2;
use bevy_replicon_renet2::netcode::{self, ServerSetupConfig, ServerAuthentication, ClientAuthentication, NetcodeClientTransport, NetcodeServerTransport};
use bevy_replicon_renet2::RenetChannelsExt;
use renet2_netcode::NativeSocket;
use tracing::{info, warn, error};
use std::net::{UdpSocket, SocketAddr, Ipv4Addr};
use std::time::SystemTime;

/// Shared configuration for connection settings
#[derive(Resource, Clone)]
pub struct ConnectionConfig {
    pub server_address: String,
    pub port: u16,
    pub protocol_id: u64,
    pub max_clients: usize,
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            server_address: "127.0.0.1".to_string(),
            port: 5000,
            protocol_id: 0,
            max_clients: 32,
        }
    }
}

/// Helper resource for setting up Replicon
#[derive(Resource)]
pub struct RepliconSetup;

impl RepliconSetup {
    pub fn setup_client_resources(
        app: &mut App, 
        config: &ConnectionConfig, 
        client_id: u64
    ) -> Result<(), String> {
        // Insert the replicon channels
        app.insert_resource(RepliconChannels::default());
        let channels = app.world().resource::<RepliconChannels>();
        
        // Create the client
        let client = renet2::RenetClient::new(
            renet2::ConnectionConfig::from_channels(
                channels.get_server_configs(), 
                channels.get_client_configs()
            ),
            false, // Don't enable encryption for now
        );

        // Get the current time for the client setup
        let current_time = match SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
            Ok(time) => time,
            Err(err) => return Err(format!("Failed to get system time: {}", err)),
        };
        
        // Create the server address
        let server_addr = match format!("{}:{}", config.server_address, config.port).parse() {
            Ok(addr) => addr,
            Err(err) => return Err(format!("Failed to parse server address: {}", err)),
        };
        
        // Bind to any available port
        let socket = match UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)) {
            Ok(socket) => socket,
            Err(err) => return Err(format!("Failed to bind client socket: {}", err)),
        };
        
        // Create the authentication
        let authentication = netcode::ClientAuthentication::Unsecure {
            client_id,
            protocol_id: config.protocol_id,
            socket_id: 0,
            server_addr,
            user_data: None,
        };
        
        // Create the native socket
        let native_socket = match NativeSocket::new(socket) {
            Ok(socket) => socket,
            Err(err) => return Err(format!("Failed to create native socket: {}", err)),
        };
        
        // Create the transport
        let transport = match netcode::NetcodeClientTransport::new(current_time, authentication, native_socket) {
            Ok(transport) => transport,
            Err(err) => return Err(format!("Failed to create netcode transport: {}", err)),
        };

        // Insert the client and transport as resources
        app.insert_resource(client);
        app.insert_resource(transport);
        
        Ok(())
    }
    
    pub fn setup_server_resources(
        app: &mut App, 
        config: &ConnectionConfig
    ) -> Result<(), String> {
        // Insert the replicon channels
        app.insert_resource(RepliconChannels::default());
        let channels = app.world().resource::<RepliconChannels>();
        
        // Create socket
        let server_addr = format!("{}:{}", config.server_address, config.port);
        let socket = match UdpSocket::bind(&server_addr) {
            Ok(socket) => socket,
            Err(err) => return Err(format!("Failed to bind to {}: {}", server_addr, err)),
        };
        
        // Create and initialize server
        let server = renet2::RenetServer::new(
            renet2::ConnectionConfig::from_channels(
                channels.get_server_configs(),
                channels.get_client_configs(),
            )
        );
        
        // Create server configuration
        let public_addr = match server_addr.parse() {
            Ok(addr) => addr,
            Err(err) => return Err(format!("Failed to parse server address: {}", err)),
        };
        
        let current_time = match SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
            Ok(time) => time,
            Err(err) => return Err(format!("Failed to get system time: {}", err)),
        };
        
        let server_config = netcode::ServerSetupConfig {
            current_time,
            max_clients: config.max_clients,
            protocol_id: config.protocol_id,
            authentication: netcode::ServerAuthentication::Unsecure,
            socket_addresses: vec![vec![public_addr]],
        };
        
        // Create transport
        match NativeSocket::new(socket) {
            Ok(native_socket) => {
                match netcode::NetcodeServerTransport::new(server_config, native_socket) {
                    Ok(transport) => {
                        app.insert_resource(server);
                        app.insert_resource(transport);
                    },
                    Err(err) => return Err(format!("Failed to create NetcodeServerTransport: {:?}", err)),
                }
            },
            Err(err) => return Err(format!("Failed to create NativeSocket: {:?}", err)),
        }
        
        Ok(())
    }
} 