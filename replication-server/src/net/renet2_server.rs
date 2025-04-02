use bevy::prelude::*;
use bevy_renet2::netcode::{NetcodeServerTransport, ServerAuthentication, ServerSetupConfig};
use bevy_renet2::prelude::RenetServer;
use bevy_renet2::prelude::ServerEvent;
use sidereal::create_connection_config;
use std::{
    collections::{HashMap, HashSet},
    error::Error,
    net::{SocketAddr, UdpSocket},
    time::{SystemTime, UNIX_EPOCH},
};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use sidereal::ecs::components::sector::Sector;
use sidereal::net::config::DEFAULT_PROTOCOL_ID;
use sidereal::net::shard_communication::{
    REPLICATION_SERVER_SHARD_PORT, SHARD_CHANNEL_RELIABLE, ShardToReplicationMessage,
};

#[derive(Resource, Default)]
pub struct ConnectedShards {
    pub shards: HashMap<u64, ShardInfo>,
}

#[derive(Debug, Clone)]
pub struct ShardInfo {
    pub shard_id: Uuid,
    pub sectors: HashSet<Sector>,
    pub connected_at: std::time::SystemTime,
}

#[derive(Resource)]
pub struct ShardListener {
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
            bind_addr: format!("0.0.0.0:{}", REPLICATION_SERVER_SHARD_PORT)
                .parse()
                .expect("Invalid default bind address"),
            max_shards: 32,
            protocol_id: DEFAULT_PROTOCOL_ID,
        }
    }
}

pub struct Renet2ServerPlugin {
    config: Renet2ServerConfig,
    tracking_enabled: bool,
}

impl Default for Renet2ServerPlugin {
    fn default() -> Self {
        Self {
            config: Renet2ServerConfig::default(),
            tracking_enabled: true,
        }
    }
}

impl Renet2ServerPlugin {
    pub fn with_config(config: Renet2ServerConfig) -> Self {
        Self {
            config,
            tracking_enabled: true,
        }
    }

    pub fn with_tracking(mut self, enabled: bool) -> Self {
        self.tracking_enabled = enabled;
        self
    }
}

impl Plugin for Renet2ServerPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.config.clone());

        match init_renet2_server(&self.config) {
            Ok(listener) => {
                app.insert_resource(listener);
            }
            Err(e) => {
                warn!("Failed to initialize renet2 listener: {}", e);
            }
        }

        if self.tracking_enabled {
            app.init_resource::<ConnectedShards>()
                .add_systems(Update, (handle_server_events, log_shard_stats));
        }

        app.add_systems(
            Update,
            manual_shard_server_update.run_if(resource_exists::<ShardListener>),
        );

        info!("Renet2 shard server plugin initialized");
    }
}

fn init_renet2_server(config: &Renet2ServerConfig) -> Result<ShardListener, Box<dyn Error>> {
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

    Ok(ShardListener { server, transport })
}

fn manual_shard_server_update(mut listener: ResMut<ShardListener>, time: Res<Time>) {
    let ShardListener { server, transport } = listener.as_mut();

    server.update(time.delta());
    if let Err(e) = transport.update(time.delta(), server) {
        error!("Shard transport update error: {:?}", e);
    }
}

fn handle_server_events(
    mut listener: ResMut<ShardListener>,
    mut connected_shards: ResMut<ConnectedShards>,
) {
    let server = &mut listener.server;

    while let Some(event) = server.get_event() {
        match event {
            ServerEvent::ClientConnected { client_id } => {
                info!(client_id = %client_id, "Shard client connected (RenetServer), awaiting identification");
            }
            ServerEvent::ClientDisconnected { client_id, reason } => {
                if let Some(shard) = connected_shards.shards.remove(&client_id) {
                    info!(
                        client_id = %client_id,
                        shard_id = %shard.shard_id,
                        reason = ?reason,
                        "Shard disconnected from replication server"
                    );
                } else {
                    info!(
                        client_id = %client_id,
                        reason = ?reason,
                        "Unidentified client disconnected from shard server"
                    );
                }
            }
        }
    }

    for client_id in server.clients_id() {
        while let Some(message) = server.receive_message(client_id, SHARD_CHANNEL_RELIABLE) {
            match bincode::serde::decode_from_slice::<ShardToReplicationMessage, _>(
                &message,
                bincode::config::standard(),
            )
            .map(|(v, _)| v)
            {
                Ok(ShardToReplicationMessage::IdentifyShard { shard_id, sectors }) => {
                    info!(client_id = %client_id, shard_id = %shard_id, "Shard connected and identified");

                    let shard_info = ShardInfo {
                        shard_id,
                        sectors: sectors.clone().into_iter().collect(),
                        connected_at: std::time::SystemTime::now(),
                    };
                    connected_shards.shards.insert(client_id, shard_info);
                }
                Ok(ShardToReplicationMessage::SectorReady { sector_coords }) => {
                    info!(client_id = %client_id, sector = ?sector_coords, "Shard confirmed SectorReady");
                }
                Ok(ShardToReplicationMessage::SectorRemoved { sector_coords }) => {
                    info!(client_id = %client_id, sector = ?sector_coords, "Shard confirmed SectorRemoved");
                }
                Ok(ShardToReplicationMessage::ShardLoadUpdate {
                    entity_count,
                    player_count,
                }) => {
                    debug!(client_id = %client_id, entity_count = entity_count, player_count = player_count, "Received shard load update");
                }
                Err(e) => {
                    error!(client_id = %client_id, error = %e, "Failed to deserialize message from shard");
                }
            }
        }
    }
}

fn log_shard_stats(
    connected_shards: Res<ConnectedShards>,
    time: Res<Time>,
    mut last_log: Local<f64>,
) {
    // Log every 30 seconds
    let current_time = time.elapsed().as_secs_f64();
    if current_time - *last_log < 30.0 {
        return;
    }
    *last_log = current_time;

    if connected_shards.shards.is_empty() {
        debug!("No shard servers currently connected to replication server");
        return;
    }

    info!("===== SHARD CONNECTION STATUS =====");
    info!("Connected shard servers: {}", connected_shards.shards.len());

    for (client_id, shard) in &connected_shards.shards {
        let uptime = match shard.connected_at.elapsed() {
            Ok(duration) => {
                let hours = duration.as_secs() / 3600;
                let minutes = (duration.as_secs() % 3600) / 60;
                let seconds = duration.as_secs() % 60;
                format!("{}h {}m {}s", hours, minutes, seconds)
            }
            Err(_) => "unknown".to_string(),
        };

        info!(
            shard_id = %shard.shard_id,
            client_id = %client_id,
            uptime = %uptime,
            "Shard server status"
        );
    }
    info!("===================================");
}
