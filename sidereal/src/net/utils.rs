use bevy::prelude::*;
use bevy_replicon::prelude::*;
use bevy_replicon_renet2::{
    netcode::{NetcodeClientTransport, NetcodeServerTransport},
    renet2::{RenetClient, RenetServer},
};

/// System to handle client and server connections
#[derive(Resource)]
pub struct NetworkStats {
    pub connected_clients: usize,
    pub is_connected_to_server: bool,
    pub ping_ms: f32,
}

impl Default for NetworkStats {
    fn default() -> Self {
        Self {
            connected_clients: 0,
            is_connected_to_server: false,
            ping_ms: 0.0,
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

/// System to update the server transport
pub fn update_server_transport(
    time: Res<Time>,
    server: Option<ResMut<RenetServer>>,
    mut transport: Option<ResMut<NetcodeServerTransport>>,
) {
    if let (Some(mut server), Some(ref mut transport)) = (server, transport.as_mut()) {
        if let Err(e) = transport.update(time.delta(), &mut server) {
            error!("Server transport update error: {:?}", e);
        }
    }
}

/// System to update the client transport
pub fn update_client_transport(
    time: Res<Time>,
    client: Option<ResMut<RenetClient>>,
    mut transport: Option<ResMut<NetcodeClientTransport>>,
) {
    if let (Some(mut client), Some(ref mut transport)) = (client, transport.as_mut()) {
        if let Err(e) = transport.update(time.delta(), &mut client) {
            error!("Client transport update error: {:?}", e);
        }
    }
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