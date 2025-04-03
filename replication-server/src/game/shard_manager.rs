use std::collections::HashMap;
use uuid::Uuid;

use bevy::prelude::*;
use tracing::{debug, error, info, warn};

use crate::net::renet2_server::Renet2ServerListener;
use sidereal::net::config::{SHARD_CHANNEL_RELIABLE, SHARD_CHANNEL_UNRELIABLE, SHARD_CHANNEL_DEFAULT};
use sidereal::net::messages::ShardToReplicationMessage;

#[derive(Resource)]
pub struct ConnectedShards {
    pub shards: HashMap<u64, ShardInfo>,
}

impl Default for ConnectedShards {
    fn default() -> Self {
        Self { shards: HashMap::new() }
    }
}

#[derive(Debug, Clone)]
pub struct ShardInfo {
    pub shard_id: Uuid,
    pub connected_at: std::time::SystemTime,
}
pub struct ShardManagerPlugin;

impl Plugin for ShardManagerPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ConnectedShards::default());
        // app.add_systems(
        //     Update,
        //     (log_shard_stats, handle_shard_events.run_if(
        //         resource_exists::<Renet2ServerListener>
        //     )),
        // );

        info!("Shard manager plugin initialized");
    }
}

fn handle_shard_events(
    mut listener: ResMut<Renet2ServerListener>,
    mut connected_shards: ResMut<ConnectedShards>,
) {
    let server = &mut listener.server;

    for client_id in server.clients_id() {
        for &channel in &[SHARD_CHANNEL_RELIABLE, SHARD_CHANNEL_UNRELIABLE, SHARD_CHANNEL_DEFAULT] {
            if let Some(message) = server.receive_message(client_id, channel) {
                info!(
                    client_id = %client_id, 
                    channel = channel, 
                    bytes = message.len(), 
                    "Received message on channel {}", 
                    channel
                );
                
                debug!("Received message bytes: {:?}", message);
                
                // Try to determine if this might be an IdentifyShard message by looking at its structure
                if message.len() >= 16 {
                    // A UUID is 16 bytes, so this could be a shard identification message
                    debug!("Possible IdentifyShard message detected");
                }
                
                // Try to decode as ShardToReplicationMessage
                let result = bincode::serde::decode_from_slice::<ShardToReplicationMessage, _>(
                    &message,
                    bincode::config::standard(),
                );
                
                match result {
                    Ok((msg, _)) => match msg {
                        ShardToReplicationMessage::IdentifyShard { shard_id } => {
                            info!(client_id = %client_id, shard_id = %shard_id, "Shard connected and identified");

                            let shard_info = ShardInfo {
                                shard_id,
                                connected_at: std::time::SystemTime::now(),
                            };

                            connected_shards.shards.insert(client_id, shard_info);
                        }
                        ShardToReplicationMessage::SectorReady { sector_coords } => {
                            info!(client_id = %client_id, sector = ?sector_coords, "Shard confirmed SectorReady");
                        }
                        ShardToReplicationMessage::SectorRemoved { sector_coords } => {
                            info!(client_id = %client_id, sector = ?sector_coords, "Shard confirmed SectorRemoved");
                        }
                        ShardToReplicationMessage::ShardLoadUpdate {
                            entity_count,
                            player_count,
                        } => {
                            debug!(client_id = %client_id, entity_count = entity_count, player_count = player_count, "Received shard load update");
                        }
                    },
                    Err(e) => {
                        error!(client_id = %client_id, error = %e, "Failed to deserialize message from shard on channel {}", channel);
                    }
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
