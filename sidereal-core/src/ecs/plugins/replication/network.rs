use bevy::prelude::*;
use bevy_replicon::prelude::*;
use bevy_replicon_renet2::{
    renet2::{self, RenetClient, RenetServer, RenetReceive, RenetSend, ConnectionConfig},
    netcode::{NetcodeClientTransport, NetcodeServerTransport, ClientAuthentication, ServerAuthentication, ServerSetupConfig},
    RepliconRenetClientPlugin, RepliconRenetServerPlugin,
};
use bevy_renet2::prelude::ChannelConfig;
use renet2_netcode::NativeSocket;
use std::net::{Ipv4Addr, UdpSocket};
use tracing::{info, error};
use std::time::SystemTime;

/// Shared configuration for connection settings
#[derive(Resource, Clone)]
pub struct NetworkConfig {
    pub server_address: String,
    pub port: u16,
    pub protocol_id: u64,
    pub max_clients: usize,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            server_address: "0.0.0.0".to_string(),
            port: 5000,
            protocol_id: 0,
            max_clients: 32,
        }
    }
}

/// Core plugin for Replicon client networking
pub struct RepliconClientPlugin {
    pub client_id: u64,
}

impl Plugin for RepliconClientPlugin {
    fn build(&self, app: &mut App) {
        info!("Building core replicon client plugin");
        
        // Extract the client_id before the closure to avoid lifetime issues
        let client_id = self.client_id;
        
        // Initialize required resources before adding the plugin
        app.init_resource::<RepliconChannels>();
        
        // Add the official client plugin with proper configuration
        app.add_plugins(RepliconRenetClientPlugin);
        
        // Configure system sets according to the expected pattern
        app.configure_sets(PreUpdate, ClientSet::ReceivePackets.after(RenetReceive))
           .configure_sets(PostUpdate, ClientSet::SendPackets.before(RenetSend));
           
        // Setup client transport using the extracted client_id
        app.add_systems(Startup, move |mut commands: Commands, config: Option<Res<NetworkConfig>>| {
            if let Some(config) = config {
                info!("Setting up client transport with client_id: {}", client_id);
                
                match setup_client_transport(client_id, &config) {
                    Ok((transport, client)) => {
                        commands.insert_resource(transport);
                        commands.insert_resource(client);
                        info!("Client transport and resources successfully initialized");
                    }
                    Err(e) => {
                        error!("Failed to set up client transport: {}", e);
                    }
                }
            } else {
                error!("No NetworkConfig resource found! Add this before using RepliconClientPlugin");
            }
        });
    }
}

/// Core plugin for Replicon server networking
pub struct RepliconServerPlugin;

impl Plugin for RepliconServerPlugin {
    fn build(&self, app: &mut App) {
        info!("Building core replicon server plugin");
        
        // Initialize required resources before adding the plugin
        app.init_resource::<RepliconChannels>()
           .init_resource::<RepliconServer>()
           .init_resource::<ConnectedClients>();
        
        // Add the official server plugin
        app.add_plugins(RepliconRenetServerPlugin);
        
        // Configure system sets according to the expected pattern
        app.configure_sets(PreUpdate, ServerSet::ReceivePackets.after(RenetReceive))
           .configure_sets(PostUpdate, ServerSet::SendPackets.before(RenetSend));
        
        // Setup server transport
        app.add_systems(Startup, |mut commands: Commands, config: Option<Res<NetworkConfig>>| {
            if let Some(config) = config {
                info!("Setting up server transport");
                
                match setup_server_transport(&config) {
                    Ok((transport, server)) => {
                        commands.insert_resource(transport);
                        commands.insert_resource(server);
                        info!("Server transport and resources successfully initialized");
                    }
                    Err(e) => {
                        error!("Failed to set up server transport: {}", e);
                    }
                }
            } else {
                error!("No NetworkConfig resource found! Add this before using RepliconServerPlugin");
            }
        });
    }
}

/// Setup client transport with proper configuration
pub fn setup_client_transport(
    client_id: u64, 
    config: &NetworkConfig
) -> Result<(NetcodeClientTransport, RenetClient), String> {
    info!("Creating client transport for client ID: {}", client_id);
    
    // Get current time for the connection
    let current_time = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_err(|e| format!("Failed to get system time: {}", e))?;
    
    // Parse server address
    let server_addr = format!("{}:{}", config.server_address, config.port)
        .parse()
        .map_err(|e| format!("Failed to parse server address: {}", e))?;
    
    // Bind to any available port
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))
        .map_err(|e| format!("Failed to bind client socket: {}", e))?;
    
    // Create authentication
    let authentication = ClientAuthentication::Unsecure {
        client_id,
        protocol_id: config.protocol_id,
        socket_id: 0,
        server_addr,
        user_data: None,
    };
    
    // Create native socket
    let native_socket = NativeSocket::new(socket)
        .map_err(|e| format!("Failed to create native socket: {}", e))?;
    
    // Create transport
    let transport = NetcodeClientTransport::new(current_time, authentication, native_socket)
        .map_err(|e| format!("Failed to create netcode transport: {}", e))?;
    
    // Create channels using RepliconChannels resource
    let replicon_channels = RepliconChannels::default();
    
    // Create server and client channel configs
    let server_channels: Vec<ChannelConfig> = replicon_channels.server_channels().iter().enumerate()
        .map(|(index, channel)| {
            let send_type = match channel.kind {
                ChannelKind::Unreliable => renet2::SendType::Unreliable,
                ChannelKind::Unordered => renet2::SendType::ReliableUnordered {
                    resend_time: channel.resend_time,
                },
                ChannelKind::Ordered => renet2::SendType::ReliableOrdered {
                    resend_time: channel.resend_time,
                },
            };
            
            ChannelConfig {
                channel_id: index as u8,
                max_memory_usage_bytes: channel.max_bytes.unwrap_or(replicon_channels.default_max_bytes),
                send_type,
            }
        })
        .collect();
        
    let client_channels: Vec<ChannelConfig> = replicon_channels.client_channels().iter().enumerate()
        .map(|(index, channel)| {
            let send_type = match channel.kind {
                ChannelKind::Unreliable => renet2::SendType::Unreliable,
                ChannelKind::Unordered => renet2::SendType::ReliableUnordered {
                    resend_time: channel.resend_time,
                },
                ChannelKind::Ordered => renet2::SendType::ReliableOrdered {
                    resend_time: channel.resend_time,
                },
            };
            
            ChannelConfig {
                channel_id: index as u8,
                max_memory_usage_bytes: channel.max_bytes.unwrap_or(replicon_channels.default_max_bytes),
                send_type,
            }
        })
        .collect();
    
    // Create connection config
    let channels_config = ConnectionConfig::from_channels(server_channels, client_channels);
    
    // Create client
    let client = renet2::RenetClient::new(channels_config, false);
    
    Ok((transport, client))
}

/// Setup server transport with proper configuration
pub fn setup_server_transport(
    config: &NetworkConfig
) -> Result<(NetcodeServerTransport, RenetServer), String> {
    info!("Creating server transport");
    
    // Parse server address
    let server_addr = format!("{}:{}", config.server_address, config.port)
        .parse()
        .map_err(|e| format!("Failed to parse server address: {}", e))?;
    
    // Create channels using RepliconChannels resource
    let replicon_channels = RepliconChannels::default();
    
    // Create server and client channel configs
    let server_channels: Vec<ChannelConfig> = replicon_channels.server_channels().iter().enumerate()
        .map(|(index, channel)| {
            let send_type = match channel.kind {
                ChannelKind::Unreliable => renet2::SendType::Unreliable,
                ChannelKind::Unordered => renet2::SendType::ReliableUnordered {
                    resend_time: channel.resend_time,
                },
                ChannelKind::Ordered => renet2::SendType::ReliableOrdered {
                    resend_time: channel.resend_time,
                },
            };
            
            ChannelConfig {
                channel_id: index as u8,
                max_memory_usage_bytes: channel.max_bytes.unwrap_or(replicon_channels.default_max_bytes),
                send_type,
            }
        })
        .collect();
        
    let client_channels: Vec<ChannelConfig> = replicon_channels.client_channels().iter().enumerate()
        .map(|(index, channel)| {
            let send_type = match channel.kind {
                ChannelKind::Unreliable => renet2::SendType::Unreliable,
                ChannelKind::Unordered => renet2::SendType::ReliableUnordered {
                    resend_time: channel.resend_time,
                },
                ChannelKind::Ordered => renet2::SendType::ReliableOrdered {
                    resend_time: channel.resend_time,
                },
            };
            
            ChannelConfig {
                channel_id: index as u8,
                max_memory_usage_bytes: channel.max_bytes.unwrap_or(replicon_channels.default_max_bytes),
                send_type,
            }
        })
        .collect();
    
    // Create connection config
    let channels_config = ConnectionConfig::from_channels(server_channels, client_channels);
    
    // Create server
    let server = renet2::RenetServer::new(channels_config);
    
    // Bind server socket
    let socket = UdpSocket::bind(server_addr)
        .map_err(|e| format!("Failed to bind server socket: {}", e))?;
    
    // Create native socket
    let native_socket = NativeSocket::new(socket)
        .map_err(|e| format!("Failed to create native socket: {}", e))?;
    
    // Get current time
    let current_time = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_err(|e| format!("Failed to get system time: {}", e))?;
    
    // Create server setup config
    let server_config = ServerSetupConfig {
        protocol_id: config.protocol_id,
        current_time,
        max_clients: config.max_clients,
        authentication: ServerAuthentication::Unsecure,
        socket_addresses: vec![vec![server_addr]],
    };
    
    // Create transport
    let transport = NetcodeServerTransport::new(server_config, native_socket)
        .map_err(|e| format!("Failed to create server transport: {}", e))?;
    
    Ok((transport, server))
} 