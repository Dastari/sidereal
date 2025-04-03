use serde::{Deserialize, Serialize};
use uuid::Uuid;

// Import Sector struct from its canonical location
use crate::ecs::components::sector::Sector;

/// Messages sent from a Shard Server to the Replication Server
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ShardToReplicationMessage {
    IdentifyShard {
        shard_id: Uuid,
    },
    SectorReady {
        sector_coords: Sector, // Sent by shard when it's ready to simulate an assigned sector
    },
    SectorRemoved {
        sector_coords: Sector, // Sent by shard when it has finished unloading a sector
    },
    ShardLoadUpdate {
        // Sent periodically by shard
        entity_count: u32,
        player_count: u32,
    },
    // Add EntityTransitionRequest here later
}

/// Messages sent from the Replication Server to a Shard Server
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ReplicationToShardMessage {
    AssignSectors { sectors: Vec<Sector> },
    UnassignSector { sector_coords: Sector },
    // Add SectorInitialState here later
    // Add EntityEnterSector here later
    // Add AcknowledgeTransition here later
    // Add ConfirmTransitionExit here later
}
