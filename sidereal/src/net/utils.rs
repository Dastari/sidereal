use bevy::prelude::*;
use bevy_replicon::prelude::*;
use bevy_replicon_renet2::{
    netcode::{NetcodeClientTransport, NetcodeServerTransport, NetcodeTransportError, NetcodeDisconnectReason},
    renet2::{RenetClient, RenetServer},
};
use std::net::{SocketAddr, TcpListener, UdpSocket};
use tracing::{info, warn, error, debug};
use std::time::Duration;

/// System to handle client and server connections
#[derive(Resource)]
pub struct NetworkStats {
    pub connected_clients: usize,
    pub is_connected_to_server: bool,
    pub ping_ms: f32,
    pub client_connected: bool,
    pub server_connected: bool,
    pub last_status_log: f32,
}

impl Default for NetworkStats {
    fn default() -> Self {
        Self {
            connected_clients: 0,
            is_connected_to_server: false,
            ping_ms: 0.0,
            client_connected: false,
            server_connected: false,
            last_status_log: 0.0,
        }
    }
}

/// Update network stats for the server
pub fn update_server_stats(
    server: Option<Res<RenetServer>>,
    mut stats: ResMut<NetworkStats>,
) {
    if let Some(server) = server {
        stats.connected_clients = server.connected_clients();
    }
}

/// Update network stats for clients
pub fn update_client_stats(
    client: Option<Res<RenetClient>>,
    mut stats: ResMut<NetworkStats>,
) {
    if let Some(client) = client {
        stats.is_connected_to_server = client.is_connected();
    }
}

/// Updates the client transport and handles any errors
pub fn update_client_transport(
    mut client: ResMut<RenetClient>,
    mut client_transport: ResMut<NetcodeClientTransport>,
    mut network_stats: ResMut<NetworkStats>,
    time: Res<Time>,
) {
    // Update connection status
    let was_connected = network_stats.is_connected_to_server;
    network_stats.is_connected_to_server = client.is_connected();
    
    // Track status changes
    let status_changed = was_connected != network_stats.is_connected_to_server;
    
    // Calculate times for reconnection attempts 
    let current_time = time.elapsed().as_secs_f32();
    let should_log = current_time - network_stats.last_status_log > 1.0;
    
    // Log connection status changes or periodic updates
    if status_changed || should_log {
        network_stats.last_status_log = current_time;
        if network_stats.is_connected_to_server {
            info!("Client connected to server");
        } else if client.is_connecting() {
            info!("Client connecting to server...");
        } else {
            info!("Client not connected to server");
        }
    }

    // Update the transport with a fixed delta time to ensure consistent behavior
    let delta = std::time::Duration::from_secs_f32(1.0 / 60.0); // 60 FPS fixed timestep
    
    if let Err(e) = client_transport.update(delta, &mut client) {
        if should_log {
            // Downgrade to debug level to reduce spam
            debug!("Client transport update error: {:?}", e);
        }
        
        // Don't call disconnect here - we want automatic reconnection
        network_stats.is_connected_to_server = false;
    }
}

/// Updates the server transport and handles any errors
pub fn update_server_transport(
    mut server: ResMut<RenetServer>,
    mut server_transport: ResMut<NetcodeServerTransport>,
    mut network_stats: ResMut<NetworkStats>,
    time: Res<Time>,
) {
    // Check if there are any connected clients
    network_stats.is_connected_to_server = !server.clients_id().is_empty();
    
    // Only log connection status changes once per second
    let current_time = time.elapsed().as_secs_f32();
    if current_time - network_stats.last_status_log > 1.0 {
        network_stats.last_status_log = current_time;
        if network_stats.is_connected_to_server {
            info!("Server is running with connected clients");
        } else {
            info!("Server is running but no clients connected");
        }
    }

    // Update the transport
    if let Err(e) = server_transport.update(time.delta(), &mut server) {
        error!("Server transport update error: {:?}", e);
        network_stats.is_connected_to_server = false;
    }
}

/// Finds an available port by trying to bind to ports incrementally
/// 
/// Starts from the preferred port and increments until it finds one that's available
/// Returns the socket address with the available port
pub fn find_available_port(preferred_host: &str, preferred_port: u16, max_attempts: u32) -> Option<SocketAddr> {
    let mut current_port = preferred_port;
    let max_port = u16::MAX.min(preferred_port + max_attempts as u16);
    
    info!("Looking for available port starting from {}:{}", preferred_host, preferred_port);
    
    while current_port < max_port {
        let addr = format!("{}:{}", preferred_host, current_port);
        
        // Try binding with UDP (which is what we'll use for the actual server)
        match UdpSocket::bind(&addr) {
            Ok(socket) => {
                // Found an available port
                if let Ok(local_addr) = socket.local_addr() {
                    info!("Found available port: {}", local_addr);
                    return Some(local_addr);
                }
                // If we can't get the local address, try the next port
                current_port += 1;
            }
            Err(_) => {
                // Port is in use, try next one
                warn!("Port {} is unavailable, trying next port", current_port);
                current_port += 1;
            }
        }
    }
    
    warn!("Failed to find available port after {} attempts", max_attempts);
    None
}

/// Systems set for server network updates
pub struct ServerNetworkPlugin;

impl Plugin for ServerNetworkPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<NetworkStats>()
            .add_systems(PostUpdate, update_server_stats)
            .add_systems(PostUpdate, update_server_transport);
    }
}

/// Systems set for client network updates
pub struct ClientNetworkPlugin;

impl Plugin for ClientNetworkPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<NetworkStats>()
            .add_systems(PostUpdate, update_client_stats)
            .add_systems(PostUpdate, update_client_transport);
    }
} 