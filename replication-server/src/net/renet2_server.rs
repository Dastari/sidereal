use bevy::prelude::*;
use bevy_renet2::netcode::{NetcodeServerTransport, ServerAuthentication, ServerSetupConfig};
use bevy_renet2::prelude::RenetServer;
use bevy_renet2::prelude::ServerEvent;
use sidereal::net::config::{
    DEFAULT_PROTOCOL_ID, DEFAULT_RENET2_PORT,  create_connection_config,
};

use std::{
    error::Error,
    net::{SocketAddr, UdpSocket},
    time::{SystemTime, UNIX_EPOCH},
};
use tracing::{error, info, warn};

#[derive(Resource)]
pub struct Renet2ServerListener {
    pub server: RenetServer,
    pub transport: NetcodeServerTransport,
}

#[derive(Resource, Debug, Clone)]
pub struct Renet2ServerConfig {
    pub bind_addr: SocketAddr,
    pub max_shards: usize,
    pub protocol_id: u64,
}

impl Default for Renet2ServerConfig {
    fn default() -> Self {
        Self {
            bind_addr: format!("0.0.0.0:{}", DEFAULT_RENET2_PORT)
                .parse()
                .expect("Invalid default bind address"),
            max_shards: 32,
            protocol_id: DEFAULT_PROTOCOL_ID,
        }
    }
}

pub struct Renet2ServerPlugin {
    config: Renet2ServerConfig,
}

impl Default for Renet2ServerPlugin {
    fn default() -> Self {
        Self {
            config: Renet2ServerConfig::default(),
        }
    }
}

impl Renet2ServerPlugin {
    pub fn with_config(config: Renet2ServerConfig) -> Self {
        Self { config }
    }
}

impl Plugin for Renet2ServerPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.config.clone());
        app.add_systems(Startup, init_server_system);

        app.add_systems(
            Update,
            server_update.run_if(resource_exists::<Renet2ServerListener>),
        );

        app.add_systems(Update, handle_server_events.after(server_update));

        info!("Renet2 shard server plugin initialized");
    }
}

fn init_server_system(world: &mut World) {
    if let Err(e) = init_renet2_server(world) {
        warn!("Failed to initialize renet2 server: {}", e);
    } else {
        info!("Initialized renet2 server");
    }
}

fn init_renet2_server(world: &mut World) -> Result<(), Box<dyn Error>> {
    let config = world.resource::<Renet2ServerConfig>();
    let socket = UdpSocket::bind(config.bind_addr)?;
    let current_time = SystemTime::now().duration_since(UNIX_EPOCH)?;

    let connection_config = create_connection_config();

    let server = RenetServer::new(connection_config);

    let setup_config = ServerSetupConfig {
        current_time,
        max_clients: config.max_shards,
        protocol_id: config.protocol_id,
        socket_addresses: vec![vec![config.bind_addr]],
        authentication: ServerAuthentication::Unsecure,
    };

    let socket = bevy_renet2::netcode::NativeSocket::new(socket)?;
    let transport = NetcodeServerTransport::new(setup_config, socket)?;

    info!(
        "Shard server initialized on {} with {} max shards",
        config.bind_addr, config.max_shards
    );

    let listener = Renet2ServerListener { server, transport };
    world.insert_resource(listener);
    Ok(())
}

pub fn server_update(mut listener: ResMut<Renet2ServerListener>, time: Res<Time>) {
    let Renet2ServerListener { server, transport } = listener.as_mut();
    server.update(time.delta());

    if let Err(e) = transport.update(time.delta(), server) {
        error!("Shard transport update error: {:?}", e);
    }
}

fn handle_server_events(mut listener: ResMut<Renet2ServerListener>) {
    let server = &mut listener.server;
    while let Some(event) = server.get_event() {
        match event {
            ServerEvent::ClientConnected { client_id } => {
                info!(client_id = %client_id, "Renet2 client connected, awaiting identification");
            }
            ServerEvent::ClientDisconnected { client_id, reason } => {
                info!(
                    client_id = %client_id,
                    reason = ?reason,
                    "Renet2 client disconnected from replication server {reason}"
                );
            }
        }
    }
    let channels = [0, 1, 2]; // Try all channels
    for client_id in server.clients_id() {
        for &channel in &channels {
            while let Some(message) = server.receive_message(client_id, channel) {
                info!(
                    "Received message on channel {} from client {}: {:?}",
                    channel, client_id, message
                );
                // Now try to parse...
            }
        }
    }
}
