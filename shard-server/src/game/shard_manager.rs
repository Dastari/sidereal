use crate::net::renet2_client::{ClientState, Renet2ClientConfig, Renet2ClientListener};
use bevy::prelude::*;
use sidereal::net::config::SHARD_CHANNEL_RELIABLE;
use sidereal::net::messages::{ReplicationToShardMessage, ShardToReplicationMessage};

use super::sector_manager::AssignedSectors;

pub struct ShardManagerPlugin;

impl Plugin for ShardManagerPlugin {
    fn build(&self, app: &mut App) {
        if app.world().contains_resource::<Renet2ClientListener>() {
            app.add_systems(
                OnEnter(ClientState::Connected),
                send_shard_identification,
            );
            app.add_systems(
                Update,
                receive_replication_messages.run_if(in_state(ClientState::Connected)),
            );
        } else {
            warn!("ShardManagerPlugin: Renet2ClientListener not found");
        }
    }
}

fn receive_replication_messages(
    mut listener: ResMut<Renet2ClientListener>,
    mut assigned_sectors: ResMut<AssignedSectors>,
) {
    let client = &mut listener.client;
    if !client.is_connected() {
        return;
    }

    while let Some(message) = client.receive_message(SHARD_CHANNEL_RELIABLE) {
        debug!("Received message on RELIABLE channel");
        match bincode::serde::decode_from_slice::<ReplicationToShardMessage, _>(
            &message,
            bincode::config::standard(),
        )
        .map(|(v, _)| v)
        {
            Ok(ReplicationToShardMessage::AssignSectors { sectors }) => {
                info!(
                    count = sectors.len(),
                    "Received AssignSectors command (RELIABLE)"
                );
                let mut changed = false;
                for sector in sectors {
                    if assigned_sectors.sectors.insert(sector.clone()) {
                        info!(sector = ?sector, "Added assigned sector");
                        changed = true;
                        // Send confirmation back immediately
                        let confirm_message = ShardToReplicationMessage::SectorReady {
                            sector_coords: sector.clone(),
                        };
                        if let Ok(bytes) = bincode::serde::encode_to_vec(
                            &confirm_message,
                            bincode::config::standard(),
                        ) {
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
                    let confirm_message = ShardToReplicationMessage::SectorRemoved {
                        sector_coords: sector_coords.clone(),
                    };
                    if let Ok(bytes) =
                        bincode::serde::encode_to_vec(&confirm_message, bincode::config::standard())
                    {
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
}

fn send_shard_identification(
    mut listener: ResMut<Renet2ClientListener>,
    config: Res<Renet2ClientConfig>,
    mut sent: Local<bool>,
) {
    let client = &mut listener.client;
    if !client.is_connected() {
        *sent = false;
        return;
    }

    if !*sent {
        info!(shard_id = %config.shard_id, "Sending shard identification to replication server");
        let message = ShardToReplicationMessage::IdentifyShard {
            shard_id: config.shard_id,
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
    mut listener: ResMut<Renet2ClientListener>,

    time: Res<Time>,
    mut last_update: Local<f64>,
) {
    let Renet2ClientListener { client, transport } = listener.as_mut();
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
    let player_count = 5; // TODO: Replace with query for players

    let message = ShardToReplicationMessage::ShardLoadUpdate {
        entity_count,
        player_count,
    };

    match bincode::serde::encode_to_vec(&message, bincode::config::standard()) {
        Ok(bytes) => {
            client.send_message(SHARD_CHANNEL_RELIABLE, bytes);
            debug!(
                "Sent load update (entities={}, players={})",
                entity_count, player_count
            );
        }
        Err(e) => error!("Failed to serialize load update: {:?}", e),
    }
}
