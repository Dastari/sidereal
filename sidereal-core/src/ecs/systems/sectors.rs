use crate::ecs::components::in_sector::InSector;
use bevy::prelude::*;
use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
// Identifies a specific sector in the grid
#[derive(
    Clone,
    Copy,
    Default,
    Debug,
    Eq,
    Hash,
    PartialEq,
    Reflect,
    Serialize,
    Deserialize,
    Encode,
    Decode,
)]
pub struct SectorCoord {
    pub x: i32,
    pub y: i32,
}

impl SectorCoord {
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

// System to update entity sectors based on their position
pub fn update_entity_sectors(
    mut commands: Commands,
    mut sector_manager: ResMut<SectorManager>,
    query: Query<(Entity, &Transform, Option<&InSector>)>,
) {
    for (entity, transform, maybe_in_sector) in query.iter() {
        let current_sector = sector_manager.get_sector_coord(&transform.translation);

        match maybe_in_sector {
            Some(in_sector) if in_sector.0 != current_sector => {
                // Entity has moved to a new sector
                sector_manager.move_entity(entity, in_sector.0, current_sector);
                commands.entity(entity).insert(InSector(current_sector));
            }
            None => {
                // Entity doesn't have a sector yet, add it
                sector_manager.add_entity_to_sector(entity, current_sector);
                commands.entity(entity).insert(InSector(current_sector));
            }
            _ => {} // Entity is still in the same sector, do nothing
        }
    }
}

// A sector that contains entities
pub struct Sector {
    pub coord: SectorCoord,
    pub entities: Vec<Entity>,
}

// The main sector manager resource
#[derive(Resource)]
pub struct SectorManager {
    pub sectors: HashMap<SectorCoord, Sector>,
    pub sector_size: f32,
}

impl Default for SectorManager {
    fn default() -> Self {
        Self {
            sectors: HashMap::new(),
            sector_size: 1000.0,
        }
    }
}

impl SectorManager {
    // Calculate which sector a position belongs to
    pub fn get_sector_coord(&self, position: &Vec3) -> SectorCoord {
        SectorCoord {
            x: (position.x / self.sector_size).floor() as i32,
            y: (position.y / self.sector_size).floor() as i32,
        }
    }

    // Get or create a sector at the given coordinates
    pub fn get_or_create_sector(&mut self, coord: SectorCoord) -> &mut Sector {
        if !self.sectors.contains_key(&coord) {
            self.sectors.insert(
                coord,
                Sector {
                    coord,
                    entities: Vec::new(),
                },
            );
        }
        self.sectors.get_mut(&coord).unwrap()
    }

    // Add an entity to a sector
    pub fn add_entity_to_sector(&mut self, entity: Entity, coord: SectorCoord) {
        let sector = self.get_or_create_sector(coord);
        if !sector.entities.contains(&entity) {
            sector.entities.push(entity);
        }
    }

    // Remove an entity from a sector
    pub fn remove_entity_from_sector(&mut self, entity: Entity, coord: SectorCoord) {
        if let Some(sector) = self.sectors.get_mut(&coord) {
            sector.entities.retain(|e| *e != entity);

            // Clean up empty sectors (optional)
            if sector.entities.is_empty() {
                self.sectors.remove(&coord);
            }
        }
    }

    // Move an entity between sectors
    pub fn move_entity(&mut self, entity: Entity, from: SectorCoord, to: SectorCoord) {
        if from != to {
            self.remove_entity_from_sector(entity, from);
            self.add_entity_to_sector(entity, to);
        }
    }
}
