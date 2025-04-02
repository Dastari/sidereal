use bevy::prelude::*;
use bevy_renet2::netcode::{ClientAuthentication, NativeSocket, NetcodeClientTransport};
use renet2::RenetClient;
use std::{
    collections::HashSet,
    error::Error,
    net::{SocketAddr, UdpSocket},
    time::{SystemTime, UNIX_EPOCH},
};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use sidereal::{create_connection_config, ecs::components::sector::Sector};
use sidereal::net::shard_communication::{
    ReplicationToShardMessage, ShardToReplicationMessage, SHARD_CHANNEL_RELIABLE,
    SHARD_CHANNEL_UNRELIABLE,
};
use sidereal::net::config::DEFAULT_PROTOCOL_ID;
use sidereal::net::shard_communication::REPLICATION_SERVER_SHARD_PORT;

#[derive(Resource, Default, Debug)]
pub struct AssignedSectors {
    pub sectors: HashSet<Sector>,
    pub dirty: bool,
}

#[derive(Resource, Debug, Clone)]
pub struct Renet2ClientConfig {
    pub bind_addr: SocketAddr,
    pub server_addr: SocketAddr,
    pub shard_id: Uuid,
    pub protocol_id: u64,
}

impl Default for Renet2ClientConfig {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1:0".parse().expect("Invalid default bind address"),
            server_addr: format!("127.0.0.1:{}", REPLICATION_SERVER_SHARD_PORT)
                .parse()
                .expect("Invalid default server address"),
            shard_id: Uuid::new_v4(),
            protocol_id: DEFAULT_PROTOCOL_ID,
        }
    }
}

pub struct Renet2ClientPlugin {
    config: Renet2ClientConfig,
    tracking_enabled: bool,
}

impl Default for Renet2ClientPlugin {
    fn default() -> Self {
        Self {
            config: Renet2ClientConfig::default(),
            tracking_enabled: true,
        }
    }
}

impl Renet2ClientPlugin {
    pub fn with_config(config: Renet2ClientConfig) -> Self {
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

impl Plugin for Renet2ClientPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.config.clone());

        app.add_systems(Startup, init_client_system);

        if self.tracking_enabled {
            app.init_resource::<AssignedSectors>()
                .add_systems(
                    Update,
                    (
                        log_connection_status.run_if(resource_exists::<RenetClient>),
                        send_shard_identification.run_if(resource_exists::<RenetClient>),
                        receive_replication_messages.run_if(resource_exists::<RenetClient>),
                        send_load_stats.run_if(resource_exists::<RenetClient>),
                    ).chain(),
                );
        }

        info!("Renet2 client plugin initialized");
    }
}

fn init_client_system(world: &mut World) {
    if let Err(e) = init_renet2_client(world) {
        warn!("Failed to initialize shard client: {}", e);
    } else {
        info!("Initialized shard client for replication connection");
    }
}

fn init_renet2_client(world: &mut World) -> Result<(), Box<dyn Error>> {
    let server_addr = {
        let config = world.resource::<Renet2ClientConfig>();
        config.server_addr
    };
    
    let socket = UdpSocket::bind(world.resource::<Renet2ClientConfig>().bind_addr)?;
    let current_time = SystemTime::now().duration_since(UNIX_EPOCH)?;
    let client_id = world.resource::<Renet2ClientConfig>().shard_id.as_u128() as u64;
    let protocol_id = world.resource::<Renet2ClientConfig>().protocol_id;

    let connection_config = create_connection_config();
    let client = RenetClient::new(connection_config, false);

    let authentication = ClientAuthentication::Unsecure {
        client_id,
        protocol_id,
        server_addr,
        user_data: None,
        socket_id: 0,
    };

    let socket = NativeSocket::new(socket)?;
    let transport = NetcodeClientTransport::new(current_time, authentication, socket)?;

    // Insert resources separately
    world.insert_resource(client);
    world.insert_resource(transport);

    info!(
        "Shard client initialized connecting to {}",
        server_addr
    );

    Ok(())
}

/// System to log connection status periodically
fn log_connection_status(
    client: Option<Res<RenetClient>>,
    time: Res<Time>,
    mut last_log: Local<f64>,
) {
    let current_time = time.elapsed().as_secs_f64();
    if current_time - *last_log < 5.0 {
        return;
    }
    *last_log = current_time;

    if let Some(client) = client {
        if client.is_connected() {
            info!("Shard Status: Connected to Replication Server");
        } else {
            info!("Shard Status: Disconnected from Replication Server");
        }
    } else {
        debug!("Shard Status: RenetClient not available");
    }
}

/// System to receive messages from the replication server
fn receive_replication_messages(
    mut client: ResMut<RenetClient>,
    mut assigned_sectors: ResMut<AssignedSectors>,
) {
    if !client.is_connected() {
        return;
    }
    
    // Process reliable messages first (more critical)
    while let Some(message) = client.receive_message(SHARD_CHANNEL_RELIABLE) {
        debug!("Received message on RELIABLE channel");
        match bincode::serde::decode_from_slice::<ReplicationToShardMessage, _>(&message, bincode::config::standard()).map(|(v, _)| v)
        {
            Ok(ReplicationToShardMessage::AssignSectors { sectors }) => {
                info!(count = sectors.len(), "Received AssignSectors command (RELIABLE)");
                let mut changed = false;
                for sector in sectors {
                    if assigned_sectors.sectors.insert(sector.clone()) {
                        info!(sector = ?sector, "Added assigned sector");
                        changed = true;
                        // Send confirmation back immediately
                        let confirm_message = ShardToReplicationMessage::SectorReady { sector_coords: sector.clone() };
                        if let Ok(bytes) = bincode::serde::encode_to_vec(&confirm_message, bincode::config::standard()) {
                            client.send_message(SHARD_CHANNEL_RELIABLE, bytes);
                            info!(sector = ?sector, "Sent SectorReady confirmation");
                        } else {
                            error!(sector = ?sector, "Failed to serialize SectorReady message");
                        }
                    }
                }
                if changed {
                    assigned_sectors.dirty = true;
                    info!("Marked assigned sectors as dirty due to AssignSectors");
                }
            }
            Ok(ReplicationToShardMessage::UnassignSector { sector_coords }) => {
                info!(sector = ?sector_coords, "Received UnassignSector command (RELIABLE)");
                if assigned_sectors.sectors.remove(&sector_coords) {
                    info!(sector = ?sector_coords, "Removed assigned sector");
                    assigned_sectors.dirty = true;
                    info!("Marked assigned sectors as dirty due to UnassignSector");
                    // Send confirmation back
                    let confirm_message = ShardToReplicationMessage::SectorRemoved { sector_coords: sector_coords.clone() };
                     if let Ok(bytes) = bincode::serde::encode_to_vec(&confirm_message, bincode::config::standard()) {
                        client.send_message(SHARD_CHANNEL_RELIABLE, bytes);
                        info!(sector = ?sector_coords, "Sent SectorRemoved confirmation");
                    } else {
                        error!(sector = ?sector_coords, "Failed to serialize SectorRemoved message");
                    }
                } else {
                    warn!(sector = ?sector_coords, "Received unassignment for sector not currently assigned");
                }
            }
            Err(e) => error!("Failed to deserialize reliable message: {:?}", e),
        }
    }

    // Process unreliable messages (less critical state updates)
     while let Some(message) = client.receive_message(SHARD_CHANNEL_UNRELIABLE) {
        debug!("Received message on UNRELIABLE channel");
        match bincode::serde::decode_from_slice::<ReplicationToShardMessage, _>(&message, bincode::config::standard()).map(|(v, _)| v)
        {
            Ok(msg) => {
                warn!("Received unhandled unreliable message type: {:?}", std::any::type_name_of_val(&msg));
            }
            Err(e) => error!("Failed to deserialize unreliable message: {:?}", e),
        }
    }
}

/// Send shard identification to replication server on connection
fn send_shard_identification(
    mut client: ResMut<RenetClient>,
    config: Res<Renet2ClientConfig>,
    mut sent: Local<bool>,
) {
    if !client.is_connected() {
        *sent = false;
        return;
    }

    // Only send identification once when we connect
    if !*sent {
        info!(shard_id = %config.shard_id, "Sending shard identification to replication server");
        let message = ShardToReplicationMessage::IdentifyShard {
            shard_id: config.shard_id,
            sectors: Vec::new(), // Start with empty, let server assign
        };
        match bincode::serde::encode_to_vec(&message, bincode::config::standard()) {
            Ok(bytes) => {
                client.send_message(SHARD_CHANNEL_RELIABLE, bytes);
                *sent = true;
                info!("Shard identification sent.");
            }
            Err(e) => error!("Failed to serialize shard identification: {:?}", e),
        }
    }
}

fn send_load_stats(
    mut client: ResMut<RenetClient>,
    time: Res<Time>,
    mut last_update: Local<f64>,
) {
    if !client.is_connected() {
        return;
    }
    
    let current_time = time.elapsed().as_secs_f64();
    if current_time - *last_update < 10.0 {
        return;
    }
    *last_update = current_time;
    
    // Placeholder counts - replace with actual queries
    let entity_count = 100; // TODO: Replace with query.iter().count() or similar
    let player_count = 5;   // TODO: Replace with query for players
    
    let message = ShardToReplicationMessage::ShardLoadUpdate {
        entity_count,
        player_count,
    };
    
    match bincode::serde::encode_to_vec(&message, bincode::config::standard()) {
        Ok(bytes) => {
            client.send_message(SHARD_CHANNEL_RELIABLE, bytes);
            debug!("Sent load update (entities={}, players={})", entity_count, player_count);
        }
        Err(e) => error!("Failed to serialize load update: {:?}", e),
    }
} 