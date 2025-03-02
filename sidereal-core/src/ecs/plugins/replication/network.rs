use bevy::prelude::*;
use bevy_replicon::prelude::*;
use bevy_replicon_renet2::renet2;
use bevy_replicon_renet2::netcode::{self};
use bevy_replicon_renet2::RenetChannelsExt;
use renet2_netcode::NativeSocket;
use std::net::{Ipv4Addr, UdpSocket};
use tracing::info;
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
            server_address: "0.0.0.0".to_string(),
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
        // Get RepliconChannels
        let channels = app.world_mut().resource::<RepliconChannels>();
        
        // Create the client
        let client = renet2::RenetClient::new(
            renet2::ConnectionConfig::from_channels(
                channels.get_server_configs(), 
                channels.get_client_configs()
            ),
            false, // Don't enable encryption for now
        );

        // Get the current time
        let current_time = match SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
            Ok(duration) => duration,
            Err(err) => return Err(format!("Failed to get system time: {}", err)),
        };
        info!("Using system time for client transport: {:?}", current_time);
        
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
    
    /// Setup server resources for a replication server
    pub fn setup_server_resources(app: &mut App, config: &ConnectionConfig) -> Result<(), String> {
        // Initialize replicon channels
        app.init_resource::<RepliconChannels>();
        let channels = app.world().resource::<RepliconChannels>();

        // Get server and client channel configs
        let (server_channels, client_channels) = (channels.get_server_configs(), channels.get_client_configs());

        // Setup the server address
        let server_addr = format!("{}:{}", config.server_address, config.port)
            .parse()
            .map_err(|e| format!("Failed to parse server address: {}", e))?;

        // Create connection config
        let connection_config = renet2::ConnectionConfig::from_channels(server_channels, client_channels);

        // Create a server with the connection config - note: no 'false' param like we tried before
        let server = renet2::RenetServer::new(connection_config);

        // Setup the UDP socket
        let socket = UdpSocket::bind(server_addr).map_err(|e| format!("Failed to bind socket: {}", e))?;

        // Create the native socket
        let native_socket = NativeSocket::new(socket)
            .map_err(|e| format!("Failed to create native socket: {}", e))?;

        // Get the current time
        let current_time = match SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
            Ok(duration) => duration,
            Err(err) => return Err(format!("Failed to get system time: {}", err)),
        };
        info!("Using system time for server transport: {:?}", current_time);

        // Create server setup config - this is the correct type for NetcodeServerTransport::new
        let server_config = netcode::ServerSetupConfig {
            protocol_id: config.protocol_id,
            current_time,
            max_clients: config.max_clients,
            authentication: netcode::ServerAuthentication::Unsecure,
            socket_addresses: vec![vec![server_addr]],
        };

        // Create the transport with the correct parameters
        let transport = match netcode::NetcodeServerTransport::new(server_config, native_socket) {
            Ok(transport) => transport,
            Err(err) => return Err(format!("Failed to create server transport: {}", err)),
        };

        // Add the resources to the app
        app.insert_resource(server);
        app.insert_resource(transport);

        // Return success
        Ok(())
    }
} 