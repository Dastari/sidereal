
use bevy::prelude::*;
use std::collections::HashSet;
use sidereal::sector::Sector;

#[derive(Resource, Default, Debug)]
pub struct AssignedSectors {
    pub sectors: HashSet<Sector>,
    pub dirty: bool,
}

/* 
fn receive_replication_messages(
    mut listener: ResMut<Renet2ClientListener>,
) {
    let client = &mut listener.client;
    if !client.is_connected() {
        return;
    }

    while let Some(message) = client.receive_message(SHARD_CHANNEL_RELIABLE) {
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
*/