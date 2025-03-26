use bevy::prelude::*;
use bevy_replicon_renet2::renet2::{RenetClient, RenetServer};
// Added IpAddr import
use std::net::{SocketAddr, UdpSocket, IpAddr};
// Removed unused Duration import
// use std::time::Duration;
use tracing::{debug, info, warn, error};

/// Resource to store basic network statistics.
#[derive(Resource, Default)]
pub struct NetworkStats {
    /// Number of clients connected to the server. Updated by `update_server_stats`.
    pub connected_clients: usize,
    /// Whether the client is currently connected to the server. Updated by `update_client_stats`.
    pub is_connected_to_server: bool,
    /// Timestamp of the last status log to enable throttling.
    pub last_status_log: f32,
}

/// Updates the server-specific network statistics.
pub fn update_server_stats(
    server: Option<Res<RenetServer>>,
    mut stats: ResMut<NetworkStats>,
) {
    if let Some(server) = server {
        stats.connected_clients = server.connected_clients();
    } else {
        if stats.connected_clients != 0 {
             stats.connected_clients = 0;
        }
    }
}

/// Updates the client-specific network statistics.
pub fn update_client_stats(
    client: Option<Res<RenetClient>>,
    mut stats: ResMut<NetworkStats>,
) {
    if let Some(client) = client {
        stats.is_connected_to_server = client.is_connected();
    } else {
         if stats.is_connected_to_server {
             stats.is_connected_to_server = false;
         }
    }
}

/// Logs the client connection status (throttled).
pub fn log_client_status(
    // Removed unused `stats: Res<NetworkStats>` argument
    client: Option<Res<RenetClient>>,
    time: Res<Time>,
    mut last_log_time: Local<f32>,
) {
    let current_time = time.elapsed_secs();
    let throttle_interval = 1.0;

    let (connected, connecting) = client.map_or((false, false), |c| (c.is_connected(), c.is_connecting()));

    if current_time - *last_log_time > throttle_interval {
        *last_log_time = current_time;

        if connected {
            info!("Client Status: Connected");
        } else if connecting {
            debug!("Client Status: Connecting...");
        } else {
            debug!("Client Status: Disconnected");
        }
    }
}


/// Logs the server connection status (throttled).
pub fn log_server_status(
    stats: Res<NetworkStats>,
    time: Res<Time>,
    mut last_log_time: Local<f32>,
) {
    let current_time = time.elapsed_secs();
    let throttle_interval = 1.0;

    if current_time - *last_log_time > throttle_interval {
        *last_log_time = current_time;

        if stats.connected_clients > 0 {
            debug!("Server Status: Running with {} client(s)", stats.connected_clients);
        } else {
            debug!("Server Status: Running with 0 clients");
        }
    }
}

/// Finds an available UDP port by trying to bind incrementally.
/// Returns the SocketAddr with the first available port found.
pub fn find_available_port(
    preferred_host: &str,
    preferred_port: u16,
    max_attempts: u32,
) -> Option<SocketAddr> {
    // Use IpAddr type
    let host_ip: IpAddr = preferred_host.parse().unwrap_or_else(|_| {
        warn!("Failed to parse preferred_host '{}', defaulting to 127.0.0.1", preferred_host);
        IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)
    });

    let max_port = preferred_port.saturating_add(max_attempts as u16);

    info!(
        "Searching for available UDP port starting from {}:{} (max attempts: {})",
        host_ip, preferred_port, max_attempts
    );

    for current_port in preferred_port..max_port {
        let addr = SocketAddr::new(host_ip, current_port);
        match UdpSocket::bind(addr) {
            Ok(socket) => {
                match socket.local_addr() {
                    Ok(local_addr) => {
                        info!("Found available port: {}", local_addr);
                        return Some(local_addr);
                    }
                    Err(e) => {
                        error!("Could not get local address for bound socket {}: {}", addr, e);
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
                debug!("Port {} is unavailable (AddrInUse)", current_port);
            }
            Err(e) => {
                 warn!("Error binding to port {}: {} ({:?}). Trying next port.", current_port, e, e.kind());
            }
        }
    }

    error!(
        "Failed to find an available UDP port in range {}-{} after {} attempts",
        preferred_port, max_port.saturating_sub(1), max_attempts
    );
    None
}


/// Plugin for server-side network statistics and status logging.
pub struct ServerNetworkPlugin;

impl Plugin for ServerNetworkPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NetworkStats>()
            .add_systems(PostUpdate, update_server_stats)
            .add_systems(PostUpdate, log_server_status.after(update_server_stats));
    }
}

/// Plugin for client-side network statistics and status logging.
pub struct ClientNetworkPlugin;

impl Plugin for ClientNetworkPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NetworkStats>()
            .add_systems(PostUpdate, update_client_stats)
            // Corrected: log_client_status uses Local state, no need for .after() necessarily,
            // but keeping it doesn't hurt and maintains logical flow.
            .add_systems(PostUpdate, log_client_status.after(update_client_stats));
    }
}