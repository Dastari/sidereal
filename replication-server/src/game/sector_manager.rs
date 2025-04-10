use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    time::{Duration, SystemTime},
};
use tracing::{debug, error, info, trace, warn};
use uuid::Uuid;

use sidereal::ecs::components::sector::Sector;

use super::ShardEvent;

const SECTOR_SIZE: f32 = 1000.0; // Size of a sector in world units
const LOAD_REBALANCE_INTERVAL: f64 = 60.0; // In seconds
const SECTOR_DEACTIVATION_TIMEOUT: Duration = Duration::from_secs(300); // 5 minutes

fn handle_shard_events(
    mut shard_events: EventReader<ShardEvent>,
    mut sector_manager: ResMut<SectorManager>,
) {
    for event in shard_events.read() {
        match event {
            ShardEvent::OnConnect { client_id, shard_id } => {
                info!(client_id = %client_id, shard_id = %shard_id, "Shard connected");
                sector_manager.register_shard(*shard_id, *client_id);
            }
            ShardEvent::OnDisconnect { client_id, shard_id } => {
                info!(client_id = %client_id, shard_id = %shard_id, "Shard disconnected");
                sector_manager.remove_shard(*shard_id);
            }
        }
    }
}

pub struct SectorManagerPlugin;

impl Plugin for SectorManagerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SectorManager>()
            .add_systems(
                Update,
                (
                    handle_shard_events,
                ),
            );
    }
}

#[derive(Resource)]
pub struct SectorManager {
    sectors: HashMap<Sector, HashSet<Uuid>>,
}

impl SectorManager {
    pub fn new() -> Self {
        Self {
            sectors: HashMap::new(),
        }
    }

    pub fn register_shard(&mut self, shard_id: Uuid, client_id: Uuid) {
        let sector = Sector::from_coords(shard_id.x, shard_id.y);
        self.sectors.entry(sector).or_insert_with(HashSet::new).insert(client_id);
    }
    
}

impl Default for SectorManager {
    fn default() -> Self {
        Self::new()
    }
}
