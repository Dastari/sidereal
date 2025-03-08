// sidereal-core/src/ecs/plugins/networking/change_tracking.rs
use bevy::prelude::*;
use std::collections::{HashMap, HashSet};

use super::{NetworkId, Networked, EntitySector, SectorId, NetworkTick};

/// Component to mark entities that have crossed sector boundaries
#[derive(Component)]
pub struct SectorCrossing {
    pub from: SectorId,
    pub to: SectorId,
}

/// System to detect entities crossing sector boundaries
pub fn detect_sector_crossings(
    mut commands: Commands,
    mut query: Query<(Entity, &NetworkId, &mut EntitySector, &Transform)>,
    sector_size: Res<SectorSize>,
) {
    for (entity, _network_id, mut entity_sector, transform) in query.iter_mut() {
        let current_sector = SectorId {
            x: (transform.translation.x / sector_size.0).floor() as i32,
            y: (transform.translation.y / sector_size.0).floor() as i32,
        };
        
        if current_sector != entity_sector.sector {
            // Mark that this entity is crossing a boundary
            entity_sector.crossing_boundary = true;
            
            // Add component to track the crossing
            commands.entity(entity).insert(SectorCrossing {
                from: entity_sector.sector,
                to: current_sector,
            });
            
            // Update the sector
            entity_sector.sector = current_sector;
        } else if entity_sector.crossing_boundary {
            // Entity has completed crossing
            entity_sector.crossing_boundary = false;
            
            // Remove the crossing component if it exists
            commands.entity(entity).remove::<SectorCrossing>();
        }
    }
}

/// Resource to track entities that have changed since last sync
#[derive(Resource, Default)]
pub struct ChangedEntities {
    pub entities: HashMap<NetworkId, HashSet<String>>, // NetworkId -> changed component names
    pub last_sync_tick: u64,
}

/// System to update the ChangedEntities resource
pub fn track_changed_entities(
    mut changed_entities: ResMut<ChangedEntities>,
    query: Query<(&NetworkId, &Networked), Changed<Networked>>,
    tick: Res<NetworkTick>,
) {
    for (network_id, networked) in query.iter() {
        if networked.last_modified_tick > changed_entities.last_sync_tick {
            let entry = changed_entities.entities.entry(*network_id).or_default();
            entry.extend(networked.changed_components.iter().cloned());
        }
    }
}

/// System to clear changes after they've been synchronized
pub fn clear_synchronized_changes(
    mut changed_entities: ResMut<ChangedEntities>,
    tick: Res<NetworkTick>,
) {
    changed_entities.entities.clear();
    changed_entities.last_sync_tick = tick.0;
}

/// Resource to define the size of sectors
#[derive(Resource)]
pub struct SectorSize(pub f32);

impl Default for SectorSize {
    fn default() -> Self {
        // Default sector size of 100x100 units
        Self(100.0)
    }
}