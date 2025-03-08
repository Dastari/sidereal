// sidereal-core/src/ecs/plugins/networking/sectors.rs
use bevy::prelude::*;
use std::collections::{HashMap, HashSet};
use serde::{Serialize, Deserialize};

use super::{SectorId, ShardServerId, NetworkId};

/// Resource to track sector assignments and entities
#[derive(Resource)]
pub struct SectorManager {
    pub sectors: HashMap<SectorId, SectorInfo>,
    pub entity_sectors: HashMap<NetworkId, SectorId>,
}

/// Information about a sector
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SectorInfo {
    pub status: SectorStatus,
    pub entity_count: usize,
    pub neighboring_sectors: Vec<SectorId>,
}

/// Status of a sector
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SectorStatus {
    Unassigned,
    Assigned(ShardServerId),
    Transitioning { from: ShardServerId, to: ShardServerId },
}

impl Default for SectorManager {
    fn default() -> Self {
        Self {
            sectors: HashMap::new(),
            entity_sectors: HashMap::new(),
        }
    }
}

impl SectorManager {
    /// Calculate which sector contains the given position
    pub fn get_sector_for_position(&self, x: f32, y: f32, sector_size: f32) -> SectorId {
        SectorId {
            x: (x / sector_size).floor() as i32,
            y: (y / sector_size).floor() as i32,
        }
    }
    
    /// Get all entities in a given sector
    pub fn get_entities_in_sector(&self, sector: SectorId) -> Vec<NetworkId> {
        self.entity_sectors
            .iter()
            .filter_map(|(id, &s)| if s == sector { Some(*id) } else { None })
            .collect()
    }
    
    /// Get all neighboring sectors
    pub fn get_neighboring_sectors(&self, sector: SectorId) -> Vec<SectorId> {
        let mut neighbors = Vec::with_capacity(8);
        
        for dx in -1..=1 {
            for dy in -1..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                
                neighbors.push(SectorId {
                    x: sector.x + dx,
                    y: sector.y + dy,
                });
            }
        }
        
        neighbors
    }
    
    /// Update which sector an entity belongs to
    pub fn update_entity_sector(&mut self, entity_id: NetworkId, sector: SectorId) {
        // Remove from old sector count if it exists
        if let Some(old_sector) = self.entity_sectors.get(&entity_id) {
            if let Some(info) = self.sectors.get_mut(old_sector) {
                info.entity_count = info.entity_count.saturating_sub(1);
            }
        }
        
        // Update to new sector
        self.entity_sectors.insert(entity_id, sector);
        
        // Create sector if it doesn't exist
        if !self.sectors.contains_key(&sector) {
            self.sectors.insert(sector, SectorInfo {
                status: SectorStatus::Unassigned,
                entity_count: 0,
                neighboring_sectors: self.get_neighboring_sectors(sector),
            });
        }
        
        // Increment entity count in new sector
        if let Some(info) = self.sectors.get_mut(&sector) {
            info.entity_count += 1;
        }
    }
    
    /// Assign a sector to a shard server
    pub fn assign_sector(&mut self, sector: SectorId, shard_id: ShardServerId) {
        if let Some(info) = self.sectors.get_mut(&sector) {
            info.status = SectorStatus::Assigned(shard_id);
        } else {
            self.sectors.insert(sector, SectorInfo {
                status: SectorStatus::Assigned(shard_id),
                entity_count: 0,
                neighboring_sectors: self.get_neighboring_sectors(sector),
            });
        }
    }
    
    /// Begin transitioning a sector between shard servers
    pub fn begin_sector_transition(&mut self, sector: SectorId, from: ShardServerId, to: ShardServerId) {
        if let Some(info) = self.sectors.get_mut(&sector) {
            info.status = SectorStatus::Transitioning { from, to };
        }
    }
    
    /// Complete a sector transition
    pub fn complete_sector_transition(&mut self, sector: SectorId, to: ShardServerId) {
        if let Some(info) = self.sectors.get_mut(&sector) {
            if matches!(info.status, SectorStatus::Transitioning { .. }) {
                info.status = SectorStatus::Assigned(to);
            }
        }
    }
}