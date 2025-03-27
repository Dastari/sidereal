// sidereal/src/utils.rs
// No changes needed based on the request. This file seems independent
// of the bi-directional setup.

use bevy::prelude::*;
use bevy_replicon_renet2::renet2::{RenetClient, RenetServer};
use std::net::{IpAddr, SocketAddr, UdpSocket};
use tracing::{debug, error, info, warn};

/// Resource to store basic network statistics.
#[derive(Resource, Default, Debug)] // Added Debug
pub struct NetworkStats {
    /// Number of clients connected to the server. Updated by `update_server_stats`.
    pub connected_clients: usize,
    /// Whether the client is currently connected to the server. Updated by `update_client_stats`.
    pub is_connected_to_server: bool,
    // Removed last_status_log - prefer using Local state in logging systems
}

/// Updates the server-specific network statistics.
pub fn update_server_stats(server: Option<Res<RenetServer>>, mut stats: ResMut<NetworkStats>) {
    let new_count = server.map_or(0, |s| s.connected_clients());
    if stats.connected_clients != new_count {
        stats.connected_clients = new_count;
        // Optionally log change here if desired, instead of periodic logging
        // info!("Server connected clients: {}", new_count);
    }
}

/// Updates the client-specific network statistics.
pub fn update_client_stats(client: Option<Res<RenetClient>>, mut stats: ResMut<NetworkStats>) {
    let new_connected = client.map_or(false, |c| c.is_connected());
    if stats.is_connected_to_server != new_connected {
        stats.is_connected_to_server = new_connected;
        // Optionally log change here
        // info!("Client connection status: {}", new_connected);
    }
}

pub fn log_client_status(
    client: Option<Res<RenetClient>>,
    time: Res<Time>,
    mut last_log_time: Local<f32>,
) {
    let current_time = time.elapsed_secs();
    let throttle_interval = 5.0; // Log less frequently

    if current_time - *last_log_time > throttle_interval {
        *last_log_time = current_time;

        match client {
            Some(client) => {
                if client.is_connected() {
                    debug!("Client Status: Connected");
                } else if client.is_connecting() {
                    info!("Client Status: Connecting..."); // Changed to info
                } else {
                    warn!("Client Status: Disconnected"); // Changed to info
                }
            }
            None => {
                // Only log if this system is expected to run when no client exists
                // debug!("Client Status: No client resource found.");
            }
        }
    }
}

/// Logs the server connection status (throttled using Local state).
pub fn log_server_status(
    server: Option<Res<RenetServer>>, // Use server directly
    time: Res<Time>,
    mut last_log_time: Local<f32>,
    stats: Res<NetworkStats>, // Keep stats for count
) {
    let current_time = time.elapsed_secs();
    let throttle_interval = 5.0; // Log less frequently

    if current_time - *last_log_time > throttle_interval {
        *last_log_time = current_time;

        if server.is_some() {
            // Use stats resource which is already updated
            debug!(
                "Server Status: Running with {} client(s)",
                stats.connected_clients
            );
        } else {
            // Only log if this system is expected to run when no server exists
            // debug!("Server Status: No server resource found.");
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
    let host_ip: IpAddr = preferred_host.parse().unwrap_or_else(|_| {
        warn!(
            "Failed to parse preferred_host '{}', defaulting to 127.0.0.1",
            preferred_host
        );
        IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)
    });

    let max_port = preferred_port.saturating_add(max_attempts as u16);

    debug!(
        // Changed to debug level for less noise
        "Searching for available UDP port starting from {}:{} (max attempts: {})",
        host_ip, preferred_port, max_attempts
    );

    for current_port in preferred_port..max_port {
        let addr = SocketAddr::new(host_ip, current_port);
        match UdpSocket::bind(addr) {
            Ok(socket) => {
                // We need to close the socket handle immediately after binding successfully
                // to free up the port for the actual application use.
                // The `socket` variable goes out of scope here, closing the socket.
                match socket.local_addr() {
                    Ok(local_addr) => {
                        info!("Found available port: {}", local_addr);
                        return Some(local_addr);
                    }
                    Err(e) => {
                        error!(
                            "Could not get local address for temporarily bound socket {}: {}",
                            addr, e
                        );
                    }
                }
                // Explicitly drop socket here just in case (though scope drop should handle it)
                drop(socket);
                // Return the address we successfully bound to.
                return Some(addr);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
                // Port is busy, try the next one. This is expected.
                // debug!("Port {} is unavailable (AddrInUse)", current_port); // Maybe too verbose for debug
            }
            Err(e) => {
                // Log other errors that might indicate a bigger problem.
                warn!(
                    "Error binding to port {}: {} ({:?}). Trying next port.",
                    current_port,
                    e,
                    e.kind()
                );
            }
        }
    }

    error!(
        "Failed to find an available UDP port in range {}-{} after {} attempts",
        preferred_port,
        max_port.saturating_sub(1),
        max_attempts
    );
    None
}

/// Plugin for server-side network statistics and status logging.
pub struct ServerNetworkPlugin;

impl Plugin for ServerNetworkPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NetworkStats>()
            // Run stats update in PostUpdate to capture state after network ticks
            .add_systems(PostUpdate, update_server_stats)
            // Run logging after stats update
            .add_systems(PostUpdate, log_server_status.after(update_server_stats));
    }
}

/// Plugin for client-side network statistics and status logging.
pub struct ClientNetworkPlugin;

impl Plugin for ClientNetworkPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NetworkStats>()
            .add_systems(PostUpdate, update_client_stats)
            .add_systems(PostUpdate, log_client_status.after(update_client_stats));
    }
}
