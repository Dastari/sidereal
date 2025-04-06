use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    time::{Duration, SystemTime},
};
use tracing::{debug, error, info, trace, warn};
use uuid::Uuid;

use sidereal::ecs::components::sector::Sector;
use sidereal::net::config::SHARD_CHANNEL_RELIABLE;
use sidereal::net::messages::ReplicationToShardMessage;

use crate::net::renet2_server::Renet2ServerListener;

const SECTOR_SIZE: f32 = 1000.0; // Size of a sector in world units
const LOAD_REBALANCE_INTERVAL: f64 = 60.0; // In seconds
const PLAYER_WEIGHT: u32 = 10; // Weight of a player in load calculations
const LOAD_THRESHOLD: u32 = 100; // Load threshold for considering a shard as overloaded
const SECTOR_DEACTIVATION_TIMEOUT: Duration = Duration::from_secs(300); // 5 minutes

#[derive(Resource, Default)]
pub struct EntityTracker {
    sector_to_entities: HashMap<Sector, HashSet<Entity>>,
    entity_to_sector: HashMap<Entity, Sector>,
}

impl EntityTracker {
    fn new() -> Self {
        Self {
            sector_to_entities: HashMap::new(),
            entity_to_sector: HashMap::new(),
        }
    }

    /// Register an entity as being in a sector
    pub fn register_entity(&mut self, entity: Entity, sector: &Sector) {
        // Remove entity from previous sector if it existed
        if let Some(old_sector) = self.entity_to_sector.get(&entity) {
            if old_sector != sector {
                if let Some(entities) = self.sector_to_entities.get_mut(old_sector) {
                    entities.remove(&entity);

                    // If sector is now empty, we might eventually want to deactivate it
                    if entities.is_empty() {
                        self.sector_to_entities.remove(old_sector);
                    }
                }
            } else {
                // Entity is already in this sector
                return;
            }
        }

        // Add entity to new sector
        self.entity_to_sector.insert(entity, sector.clone());
        self.sector_to_entities
            .entry(sector.clone())
            .or_insert_with(HashSet::new)
            .insert(entity);
    }

    /// Remove an entity when it's despawned
    pub fn remove_entity(&mut self, entity: Entity) {
        if let Some(sector) = self.entity_to_sector.remove(&entity) {
            if let Some(entities) = self.sector_to_entities.get_mut(&sector) {
                entities.remove(&entity);

                // If sector is now empty, remove it from tracking
                if entities.is_empty() {
                    self.sector_to_entities.remove(&sector);
                }
            }
        }
    }

    /// Get all entities in a sector
    pub fn get_entities_in_sector(&self, sector: &Sector) -> Option<&HashSet<Entity>> {
        self.sector_to_entities.get(sector)
    }

    /// Get all active sectors (sectors with at least one entity)
    pub fn get_active_sectors(&self) -> Vec<Sector> {
        self.sector_to_entities.keys().cloned().collect()
    }

    /// Get the current sector of an entity
    pub fn get_entity_sector(&self, entity: &Entity) -> Option<&Sector> {
        self.entity_to_sector.get(entity)
    }
}

/// Load statistics for a shard
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardLoadStats {
    pub entity_count: u32,
    pub player_count: u32,
    // Future potential fields:
    // pub avg_tick_time_ms: f32,
    // pub cpu_load_percent: f32,
}

impl Default for ShardLoadStats {
    fn default() -> Self {
        Self {
            entity_count: 0,
            player_count: 0,
        }
    }
}

/// Extended information about a shard for the sector manager
#[derive(Debug, Clone)]
pub struct ShardInfo {
    pub shard_id: Uuid,
    pub client_id: u64,
    pub sectors: HashSet<Sector>,
    pub connected_at: SystemTime,
    pub load_stats: ShardLoadStats,
    pub last_load_update: SystemTime,
}

/// State of a sector in the replication server
#[derive(Debug, Clone)]
pub enum SectorAssignmentState {
    Unloaded,
    Loading { shard_id: Uuid },
    Active { shard_id: Uuid },
    Unloading { shard_id: Uuid },
}

/// Resource that manages active shards and their assigned sectors
#[derive(Resource)]
pub struct SectorManager {
    /// Maps shard ID to shard information
    pub shards: HashMap<Uuid, ShardInfo>,
    /// Maps sector coordinates to sector state
    pub sector_map: HashMap<Sector, SectorAssignmentState>,
    /// Maps client IDs to shard IDs for quick lookups
    client_to_shard: HashMap<u64, Uuid>,
    /// Track empty sectors and when they became empty
    empty_sectors: HashMap<Sector, SystemTime>,
}

impl Default for SectorManager {
    fn default() -> Self {
        Self {
            shards: HashMap::new(),
            sector_map: HashMap::new(),
            client_to_shard: HashMap::new(),
            empty_sectors: HashMap::new(),
        }
    }
}

impl SectorManager {
    /// Register a new shard with the sector manager
    pub fn register_shard(&mut self, shard_id: Uuid, client_id: u64) {
        let shard_info = ShardInfo {
            shard_id,
            client_id,
            sectors: HashSet::new(),
            connected_at: SystemTime::now(),
            load_stats: ShardLoadStats::default(),
            last_load_update: SystemTime::now(),
        };

        self.shards.insert(shard_id, shard_info);
        self.client_to_shard.insert(client_id, shard_id);

        info!(shard_id = %shard_id, client_id = %client_id, "Registered new shard");
    }

    /// Remove a shard from the sector manager
    pub fn remove_shard(&mut self, shard_id: Uuid) {
        if let Some(shard) = self.shards.remove(&shard_id) {
            self.client_to_shard.remove(&shard.client_id);

            // Mark all sectors managed by this shard as unloaded
            for sector in shard.sectors.iter() {
                self.sector_map
                    .insert(sector.clone(), SectorAssignmentState::Unloaded);
            }

            info!(
                shard_id = %shard_id,
                sectors_count = %shard.sectors.len(),
                "Removed shard and marked its sectors as unloaded"
            );
        }
    }

    /// Update load statistics for a shard
    pub fn update_shard_load(&mut self, shard_id: Uuid, stats: ShardLoadStats) {
        if let Some(shard) = self.shards.get_mut(&shard_id) {
            let entity_count = stats.entity_count;
            let player_count = stats.player_count;
            shard.load_stats = stats;
            shard.last_load_update = SystemTime::now();
            trace!(shard_id = %shard_id, "Updated shard load stats: {} entities, {} players", entity_count, player_count);
        }
    }

    /// Calculate the load score for a shard
    fn calculate_load_score(&self, shard_id: &Uuid) -> u32 {
        if let Some(shard) = self.shards.get(shard_id) {
            shard.load_stats.entity_count + (shard.load_stats.player_count * PLAYER_WEIGHT)
        } else {
            0
        }
    }

    /// Calculate proximity score for a shard (lower is better - prefers adjacent sectors)
    fn calculate_proximity_score(&self, shard_id: &Uuid, sector: &Sector) -> i32 {
        if let Some(shard) = self.shards.get(shard_id) {
            // Check if shard manages any adjacent sectors
            let x = sector.x;
            let y = sector.y;
            let adjacent_coords = [
                Sector::new(x - 1, y),
                Sector::new(x + 1, y),
                Sector::new(x, y - 1),
                Sector::new(x, y + 1),
            ];

            let adjacent_count = adjacent_coords
                .iter()
                .filter(|coord| shard.sectors.contains(coord))
                .count();

            // If shard manages adjacent sectors, give it a bonus (negative score)
            if adjacent_count > 0 {
                -(adjacent_count as i32 * 10) // Negative score means preference
            } else {
                // Penalty for non-adjacent sectors
                10
            }
        } else {
            100 // Large penalty for unknown shards
        }
    }

    /// Select the best shard for a sector based on load and proximity
    fn select_best_shard_for_sector(&self, sector: &Sector) -> Option<Uuid> {
        if self.shards.is_empty() {
            return None;
        }

        // Find the shard with the lowest combined score
        self.shards
            .keys()
            .min_by_key(|shard_id| {
                let load_score = self.calculate_load_score(shard_id) as i32;
                let proximity_score = self.calculate_proximity_score(shard_id, sector);
                load_score + proximity_score
            })
            .copied()
    }

    /// Activate a sector (assign it to a shard)
    pub fn activate_sector(&mut self, sector: Sector) -> Option<(Uuid, u64)> {
        // Check if sector is already active or loading
        match self.sector_map.get(&sector) {
            Some(SectorAssignmentState::Active { shard_id }) => {
                // Already active
                if let Some(shard) = self.shards.get(shard_id) {
                    trace!(
                        shard_id = %shard_id,
                        sector = ?sector,
                        "Sector already active on shard"
                    );
                    return Some((*shard_id, shard.client_id));
                }
                return None;
            }
            Some(SectorAssignmentState::Loading { shard_id }) => {
                // Already loading
                if let Some(shard) = self.shards.get(shard_id) {
                    trace!(
                        shard_id = %shard_id,
                        sector = ?sector,
                        "Sector already loading on shard"
                    );
                    return Some((*shard_id, shard.client_id));
                }
                return None;
            }
            Some(SectorAssignmentState::Unloading { .. }) => {
                // Currently unloading, wait for it to complete
                debug!(
                    sector = ?sector,
                    "Sector is currently unloading, will wait for completion before activating"
                );
                return None;
            }
            _ => {
                // Not active or loading, continue with activation
            }
        }

        // If no shards are available, return None
        if self.shards.is_empty() {
            return None;
        }

        // Select the best shard for this sector
        if let Some(shard_id) = self.select_best_shard_for_sector(&sector) {
            if let Some(shard) = self.shards.get_mut(&shard_id) {
                // Mark the sector as loading
                self.sector_map
                    .insert(sector.clone(), SectorAssignmentState::Loading { shard_id });

                // Add the sector to the shard's managed sectors
                shard.sectors.insert(sector.clone());

                // Remove from empty sectors if it was there
                self.empty_sectors.remove(&sector);

                debug!(
                    shard_id = %shard_id,
                    sector = ?sector,
                    "Activating sector on shard"
                );

                return Some((shard_id, shard.client_id));
            }
        } else {
            warn!(
                sector = ?sector,
                "Failed to select a suitable shard for sector activation"
            );
        }

        None
    }

    /// Mark a sector as active (after the shard has confirmed it's ready)
    pub fn mark_sector_active(&mut self, sector_coords: (i32, i32), shard_id: Uuid) {
        let sector = Sector::new(sector_coords.0, sector_coords.1);

        // Ensure the sector exists and is in Loading state for this shard
        if let Some(SectorAssignmentState::Loading {
            shard_id: loading_shard,
        }) = self.sector_map.get(&sector)
        {
            if *loading_shard == shard_id {
                self.sector_map
                    .insert(sector.clone(), SectorAssignmentState::Active { shard_id });
                info!(
                    shard_id = %shard_id,
                    sector = ?sector,
                    "Sector now active on shard (confirmation received)"
                );
            } else {
                warn!(
                    expected_shard = %loading_shard,
                    actual_shard = %shard_id,
                    sector = ?sector,
                    "Sector readiness reported by unexpected shard"
                );
            }
        } else {
            warn!(
                shard_id = %shard_id,
                sector = ?sector,
                "Cannot mark sector as active, not in Loading state"
            );
        }
    }

    /// Initiate deactivation of a sector
    pub fn deactivate_sector(&mut self, sector: Sector) -> Option<(Uuid, u64)> {
        // Check if sector is active
        if let Some(SectorAssignmentState::Active { shard_id }) = self.sector_map.get(&sector) {
            let shard_id = *shard_id;
            if let Some(shard) = self.shards.get(&shard_id) {
                // Mark sector as unloading
                self.sector_map.insert(
                    sector.clone(),
                    SectorAssignmentState::Unloading { shard_id },
                );

                info!(
                    shard_id = %shard_id,
                    sector = ?sector,
                    "Deactivating sector"
                );

                return Some((shard_id, shard.client_id));
            }
        }

        None
    }

    /// Mark a sector as unloaded (after the shard has confirmed removal)
    pub fn mark_sector_unloaded(&mut self, sector_coords: (i32, i32), shard_id: Uuid) {
        let sector = Sector::new(sector_coords.0, sector_coords.1);

        // Ensure the sector exists and is in Unloading state for this shard
        if let Some(SectorAssignmentState::Unloading {
            shard_id: unloading_shard,
        }) = self.sector_map.get(&sector)
        {
            if *unloading_shard == shard_id {
                self.sector_map
                    .insert(sector.clone(), SectorAssignmentState::Unloaded);

                // Remove sector from shard's managed sectors
                if let Some(shard) = self.shards.get_mut(&shard_id) {
                    shard.sectors.remove(&sector);
                }

                info!(
                    shard_id = %shard_id,
                    sector = ?sector,
                    "Sector now unloaded"
                );
            } else {
                warn!(
                    expected_shard = %unloading_shard,
                    actual_shard = %shard_id,
                    sector = ?sector,
                    "Sector unload reported by unexpected shard"
                );
            }
        } else {
            warn!(
                shard_id = %shard_id,
                sector = ?sector,
                "Cannot mark sector as unloaded, not in Unloading state"
            );
        }
    }

    /// Get the shard ID and client ID responsible for a sector
    pub fn get_sector_shard(&self, sector: &Sector) -> Option<(Uuid, u64)> {
        match self.sector_map.get(sector) {
            Some(SectorAssignmentState::Active { shard_id })
            | Some(SectorAssignmentState::Loading { shard_id }) => {
                if let Some(shard) = self.shards.get(shard_id) {
                    Some((*shard_id, shard.client_id))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Mark a sector as empty of significant entities
    pub fn mark_sector_empty(&mut self, sector: Sector) {
        if !self.empty_sectors.contains_key(&sector) {
            self.empty_sectors.insert(sector.clone(), SystemTime::now());
            trace!(sector = ?sector, "Marked sector as empty");
        }
    }

    /// Mark a sector as non-empty (has significant entities)
    pub fn mark_sector_non_empty(&mut self, sector: Sector) {
        if self.empty_sectors.remove(&sector).is_some() {
            trace!(sector = ?sector, "Marked sector as non-empty");
        }
    }

    /// Get sectors that have been empty for too long and should be deactivated
    pub fn get_deactivation_candidates(&self) -> Vec<Sector> {
        let now = SystemTime::now();
        self.empty_sectors
            .iter()
            .filter_map(
                |(sector, empty_since)| match now.duration_since(*empty_since) {
                    Ok(duration) if duration >= SECTOR_DEACTIVATION_TIMEOUT => Some(sector.clone()),
                    _ => None,
                },
            )
            .collect()
    }

    /// Check for sectors that need to be rebalanced (moved from overloaded to underloaded shards)
    pub fn get_rebalance_candidates(&self) -> Vec<(Sector, Uuid)> {
        if self.shards.len() <= 1 {
            return Vec::new(); // No rebalancing needed with 0 or 1 shards
        }

        let mut candidates = Vec::new();

        // Identify overloaded shards
        let overloaded_shards: Vec<_> = self
            .shards
            .iter()
            .filter(|(_, shard)| {
                let load =
                    shard.load_stats.entity_count + (shard.load_stats.player_count * PLAYER_WEIGHT);
                load > LOAD_THRESHOLD
            })
            .collect();

        if overloaded_shards.is_empty() {
            return Vec::new(); // No overloaded shards
        }

        // For each overloaded shard, find sectors that could be migrated
        for (shard_id, shard) in overloaded_shards {
            // Find sectors on the edge of this shard's territory
            // (those that have the fewest adjacent sectors also managed by this shard)
            let edge_sectors: Vec<_> = shard
                .sectors
                .iter()
                .filter(|sector| {
                    // A sector is on the edge if it has fewer than 4 adjacent sectors
                    // also managed by this shard
                    let x = sector.x;
                    let y = sector.y;
                    let adjacent = [
                        Sector::new(x - 1, y),
                        Sector::new(x + 1, y),
                        Sector::new(x, y - 1),
                        Sector::new(x, y + 1),
                    ];

                    let adjacent_managed = adjacent
                        .iter()
                        .filter(|coord| shard.sectors.contains(coord))
                        .count();

                    adjacent_managed < 4
                })
                .cloned()
                .collect();

            // Add up to 2 edge sectors as candidates for migration
            for sector in edge_sectors.iter().take(2) {
                candidates.push((sector.clone(), *shard_id));
            }
        }

        candidates
    }

    /// Convert a world position to a sector coordinate
    pub fn world_pos_to_sector(&self, pos: (f32, f32)) -> Sector {
        let x = (pos.0 / SECTOR_SIZE).floor() as i32;
        let y = (pos.1 / SECTOR_SIZE).floor() as i32;
        Sector::new(x, y)
    }

    /// Get sectors around a position within a radius
    pub fn get_sectors_in_radius(&self, center: (f32, f32), radius: f32) -> Vec<Sector> {
        let center_sector = self.world_pos_to_sector(center);
        let sector_radius = (radius / SECTOR_SIZE).ceil() as i32;

        let mut sectors = Vec::new();
        for dx in -sector_radius..=sector_radius {
            for dy in -sector_radius..=sector_radius {
                sectors.push(Sector::new(center_sector.x + dx, center_sector.y + dy));
            }
        }

        sectors
    }

    /// Update sector map based on active entities
    pub fn update_active_sectors(&mut self, active_sectors: &[Sector]) {
        // First collect sectors that need to be marked as empty
        let to_mark_empty: Vec<Sector> = self
            .sector_map
            .keys()
            .filter(|sector| {
                !active_sectors.contains(sector)
                    && !self.empty_sectors.contains_key(sector)
                    && matches!(
                        self.sector_map.get(sector),
                        Some(SectorAssignmentState::Active { .. })
                    )
            })
            .cloned()
            .collect();

        // Now mark them as empty
        for sector in to_mark_empty {
            self.mark_sector_empty(sector);
        }

        // Then ensure we have active sectors in the map
        for sector in active_sectors {
            // If sector is marked as empty but now has entities, mark it as non-empty
            if self.empty_sectors.contains_key(sector) {
                self.mark_sector_non_empty(sector.clone());
            }

            // If sector is not in our map or is unloaded, try to activate it
            if !self.sector_map.contains_key(sector)
                || matches!(
                    self.sector_map.get(sector),
                    Some(SectorAssignmentState::Unloaded)
                )
            {
                self.activate_sector(sector.clone());
            }
        }
    }

    /// Handle a shard identification message to register or update a shard
    pub fn handle_shard_identification(
        &mut self,
        client_id: u64,
        shard_id: Uuid,
        sectors: Vec<(i32, i32)>,
    ) -> Vec<(i32, i32)> {
        // Check if this is a known shard
        if let Some(existing_shard_id) = self.client_to_shard.get(&client_id).copied() {
            if existing_shard_id != shard_id {
                warn!(
                    client_id = %client_id,
                    old_shard_id = %existing_shard_id,
                    new_shard_id = %shard_id,
                    "Shard ID changed for existing client!"
                );
                // Remove old shard entry and create a new one
                self.remove_shard(existing_shard_id);
                self.register_shard(shard_id, client_id);
            }
        } else {
            // New shard
            self.register_shard(shard_id, client_id);
        }

        let sectors_set: HashSet<Sector> =
            sectors.iter().map(|(x, y)| Sector::new(*x, *y)).collect();
        let mut new_sectors = Vec::new();

        if sectors_set.is_empty() {
            info!(
                shard_id = %shard_id,
                "New shard connected with no sectors, assigning initial sectors"
            );

            // Shard is requesting initial sector assignment
            // Assign some initial sectors based on load balancing
            for x in 0..2 {
                for y in 0..2 {
                    let sector = Sector::new(x, y);

                    // Only assign if sector is unloaded
                    if !self.sector_map.contains_key(&sector)
                        || matches!(
                            self.sector_map.get(&sector),
                            Some(SectorAssignmentState::Unloaded)
                        )
                    {
                        // Mark as loading
                        self.sector_map
                            .insert(sector.clone(), SectorAssignmentState::Loading { shard_id });

                        // Add to shard's managed sectors
                        if let Some(shard) = self.shards.get_mut(&shard_id) {
                            shard.sectors.insert(sector.clone());
                        }

                        new_sectors.push((sector.x, sector.y));
                        info!(
                            shard_id = %shard_id,
                            sector = ?sector,
                            "Assigned sector to shard (will transition to Loading state)"
                        );
                    }
                }
            }

            if new_sectors.is_empty() {
                info!(
                    shard_id = %shard_id,
                    "Default sectors already assigned, finding alternative sectors"
                );

                // If all default sectors were already assigned, find some other unloaded sectors
                for x in -5..5 {
                    for y in -5..5 {
                        let sector = Sector::new(x, y);

                        if !self.sector_map.contains_key(&sector)
                            || matches!(
                                self.sector_map.get(&sector),
                                Some(SectorAssignmentState::Unloaded)
                            )
                        {
                            // Mark as loading
                            self.sector_map.insert(
                                sector.clone(),
                                SectorAssignmentState::Loading { shard_id },
                            );

                            // Add to shard's managed sectors
                            if let Some(shard) = self.shards.get_mut(&shard_id) {
                                shard.sectors.insert(sector.clone());
                            }

                            new_sectors.push((sector.x, sector.y));
                            info!(
                                shard_id = %shard_id,
                                sector = ?sector,
                                "Assigned alternative sector to shard"
                            );

                            // Only assign a few sectors
                            if new_sectors.len() >= 4 {
                                break;
                            }
                        }
                    }
                    if new_sectors.len() >= 4 {
                        break;
                    }
                }
            }

            if !new_sectors.is_empty() {
                info!(
                    shard_id = %shard_id,
                    sectors = ?new_sectors,
                    "Assigned {} initial sectors to shard",
                    new_sectors.len()
                );
            } else {
                warn!(
                    shard_id = %shard_id,
                    "Could not find any unloaded sectors to assign to shard"
                );
            }
        } else {
            // Shard is reporting sectors it's already managing
            info!(
                shard_id = %shard_id,
                sectors = ?sectors_set,
                "Shard reported it's already managing {} sectors",
                sectors_set.len()
            );

            // Just update our records
            if let Some(shard) = self.shards.get_mut(&shard_id) {
                for sector in &sectors_set {
                    // Only update if not already tracked
                    if !shard.sectors.contains(sector) {
                        shard.sectors.insert(sector.clone());

                        // Update sector map
                        self.sector_map
                            .insert(sector.clone(), SectorAssignmentState::Active { shard_id });
                        info!(
                            shard_id = %shard_id,
                            sector = ?sector,
                            "Updated records to show shard is managing this sector"
                        );
                    }
                }
            }
        }

        new_sectors
    }
}

/// System to handle entity transitions between sectors
fn handle_entity_transitions(
    mut commands: Commands,
    mut entity_tracker: ResMut<EntityTracker>,
    mut sector_manager: ResMut<SectorManager>,
    query: Query<(Entity, &Transform, Option<&Sector>)>,
) {
    for (entity, transform, maybe_sector) in query.iter() {
        let position = (transform.translation.x, transform.translation.y);
        let current_sector = sector_manager.world_pos_to_sector(position);

        // Check if sector needs to be updated
        match maybe_sector {
            Some(sector) if sector.x == current_sector.x && sector.y == current_sector.y => {
                // Sector hasn't changed, just register with tracker
                entity_tracker.register_entity(entity, &current_sector);
            }
            _ => {
                // Sector missing or has changed, update it
                commands.entity(entity).insert(current_sector.clone());
                entity_tracker.register_entity(entity, &current_sector);
            }
        }
    }

    // Update sector manager with current active sectors
    let active_sectors = entity_tracker.get_active_sectors();
    sector_manager.update_active_sectors(&active_sectors);
}

/// System to check for sectors that need deactivation
fn check_deactivation_candidates(
    entity_tracker: Res<EntityTracker>,
    mut sector_manager: ResMut<SectorManager>,
    mut listener: ResMut<Renet2ServerListener>,
    time: Res<Time>,
    mut last_check: Local<f64>,
) {
    let server = &mut listener.server;
    // Only check periodically
    let current_time = time.elapsed().as_secs_f64();
    if current_time - *last_check < 30.0 {
        return;
    }
    *last_check = current_time;

    // Get candidates for deactivation
    let candidates = sector_manager.get_deactivation_candidates();
    if candidates.is_empty() {
        return;
    }

    // Filter out sectors that still have entities
    let actual_candidates: Vec<_> = candidates
        .into_iter()
        .filter(|sector| {
            let entities = entity_tracker.get_entities_in_sector(sector);
            entities.is_none() || entities.unwrap().is_empty()
        })
        .collect();

    if actual_candidates.is_empty() {
        return;
    }
    // Initiate deactivation for each candidate
    for sector in actual_candidates {
        if let Some((shard_id, client_id)) = sector_manager.deactivate_sector(sector.clone()) {
            // Send unassign message to the shard
            let message = ReplicationToShardMessage::UnassignSector {
                sector_coords: sector.clone(),
            };

            match bincode::serde::encode_to_vec(&message, bincode::config::standard()) {
                Ok(bytes) => {
                    server.send_message(client_id, SHARD_CHANNEL_RELIABLE, bytes);
                    info!(
                        shard_id = %shard_id,
                        sector = ?sector,
                        "Sent sector unassignment to shard"
                    );
                }
                Err(e) => error!("Failed to serialize sector unassignment: {:?}", e),
            }
        }
    }
}

/// System to periodically rebalance sector load
fn rebalance_sectors(
    entity_tracker: Res<EntityTracker>,
    mut sector_manager: ResMut<SectorManager>,
    mut listener: ResMut<Renet2ServerListener>,
    time: Res<Time>,
    mut last_rebalance: Local<f64>,
) {
    let server = &mut listener.server;
    // Only rebalance periodically
    let current_time = time.elapsed().as_secs_f64();
    if current_time - *last_rebalance < LOAD_REBALANCE_INTERVAL {
        return;
    }
    *last_rebalance = current_time;

    // Skip rebalancing if we have fewer than 2 shards
    if sector_manager.shards.len() <= 1 {
        return;
    }

    // Get candidates for rebalancing
    let candidates = sector_manager.get_rebalance_candidates();
    if candidates.is_empty() {
        return;
    }

    // Process each candidate for rebalancing
    for (sector, source_shard_id) in candidates {
        // Verify sector still has entities before rebalancing
        if let Some(entities) = entity_tracker.get_entities_in_sector(&sector) {
            if entities.is_empty() {
                continue; // Skip empty sectors
            }
        } else {
            continue; // Skip if sector has no entities
        }

        // Find a better shard for this sector
        if let Some(target_shard_id) = sector_manager.select_best_shard_for_sector(&sector) {
            // Skip if best shard is the current one
            if target_shard_id == source_shard_id {
                continue;
            }

            // Get client IDs for both shards
            let source_client_id = if let Some(shard) = sector_manager.shards.get(&source_shard_id)
            {
                shard.client_id
            } else {
                continue;
            };

            let _target_client_id = if let Some(shard) = sector_manager.shards.get(&target_shard_id)
            {
                shard.client_id
            } else {
                continue;
            };

            // Step 1: Send unassign message to source shard
            let unassign_message = ReplicationToShardMessage::UnassignSector {
                sector_coords: sector.clone(),
            };

            match bincode::serde::encode_to_vec(&unassign_message, bincode::config::standard()) {
                Ok(bytes) => {
                    server.send_message(source_client_id, SHARD_CHANNEL_RELIABLE, bytes);
                    info!(
                        source_shard = %source_shard_id,
                        target_shard = %target_shard_id,
                        sector = ?sector,
                        "Initiating sector rebalance: unassigning from source shard"
                    );

                    // Update sector map to show it's unloading from source shard
                    sector_manager.sector_map.insert(
                        sector.clone(),
                        SectorAssignmentState::Unloading {
                            shard_id: source_shard_id,
                        },
                    );

                    // Step 2: Assign to target shard (will be done when source shard confirms removal)
                    // The flow will be:
                    // 1. Source shard confirms removal via SectorRemoved message
                    // 2. Then the sector will be automatically activated on a new shard
                }
                Err(e) => error!(
                    "Failed to serialize sector unassignment for rebalance: {:?}",
                    e
                ),
            }
        }
    }
}

/// System to clean up entities when they're despawned
fn cleanup_despawned_entities(
    mut entity_tracker: ResMut<EntityTracker>,
    mut removed_entities: RemovedComponents<Transform>,
) {
    for entity in removed_entities.read() {
        entity_tracker.remove_entity(entity);
    }
}

/// System to periodically log sector assignment status
fn log_sector_assignment_status(
    entity_tracker: Res<EntityTracker>,
    sector_manager: Res<SectorManager>,
    time: Res<Time>,
    mut last_log: Local<f64>,
) {
    // Log every 30 seconds instead of 10 to reduce spam
    let current_time = time.elapsed().as_secs_f64();
    if current_time - *last_log < 30.0 {
        return;
    }
    *last_log = current_time;

    if sector_manager.sector_map.is_empty() {
        debug!("No sectors are currently tracked by the sector manager");
        return;
    }

    // Count sectors in each state
    let mut unloaded_count = 0;
    let mut loading_count = 0;
    let mut active_count = 0;
    let mut unloading_count = 0;

    for (_sector, state) in &sector_manager.sector_map {
        match state {
            SectorAssignmentState::Unloaded => unloaded_count += 1,
            SectorAssignmentState::Loading { .. } => loading_count += 1,
            SectorAssignmentState::Active { .. } => active_count += 1,
            SectorAssignmentState::Unloading { .. } => unloading_count += 1,
        }
    }

    // Count active entities by sector
    let entity_count: usize = entity_tracker
        .get_active_sectors()
        .iter()
        .map(|sector| {
            if let Some(entities) = entity_tracker.get_entities_in_sector(sector) {
                entities.len()
            } else {
                0
            }
        })
        .sum();

    // Use debug level for regular status updates instead of info
    debug!("===== SECTOR ASSIGNMENT STATUS =====");
    debug!("Total tracked sectors: {}", sector_manager.sector_map.len());
    debug!("  - Unloaded: {}", unloaded_count);
    debug!("  - Loading: {}", loading_count);
    debug!("  - Active: {}", active_count);
    debug!("  - Unloading: {}", unloading_count);
    debug!("Total entities tracked: {}", entity_count);

    // Log important state changes at info level
    if loading_count > 0 || unloading_count > 0 {
        info!(
            "Sector state changes in progress: {} loading, {} unloading",
            loading_count, unloading_count
        );
    }

    // If there are any sectors in Loading state, list them at debug level
    if loading_count > 0 {
        let loading_sectors: Vec<_> = sector_manager
            .sector_map
            .iter()
            .filter_map(|(sector, state)| match state {
                SectorAssignmentState::Loading { shard_id } => Some((sector, shard_id)),
                _ => None,
            })
            .collect();

        debug!("Sectors in Loading state:");
        for (sector, shard_id) in loading_sectors {
            // Try to find the client_id for this shard_id
            let client_id = sector_manager
                .shards
                .get(shard_id)
                .map(|info| info.client_id.to_string())
                .unwrap_or_else(|| "unknown".to_string());

            debug!(
                "  - Sector {:?} being loaded by shard {} (client_id: {})",
                sector, shard_id, client_id
            );
        }
    }

    // Only log active sectors at trace level to minimize spam
    if active_count > 0 && tracing::level_enabled!(tracing::Level::TRACE) {
        let active_sectors: Vec<_> = sector_manager
            .sector_map
            .iter()
            .filter_map(|(sector, state)| match state {
                SectorAssignmentState::Active { shard_id } => Some((sector, shard_id)),
                _ => None,
            })
            .collect();

        trace!("Sectors in Active state:");
        for (sector, shard_id) in active_sectors {
            // Count entities in this sector
            let entity_count = entity_tracker
                .get_entities_in_sector(sector)
                .map(|set| set.len())
                .unwrap_or(0);

            trace!(
                "  - Sector {:?} active on shard {} with {} entities",
                sector, shard_id, entity_count
            );
        }
    }

    debug!("====================================");
}

/// Plugin that sets up the sector management systems
pub struct SectorManagerPlugin;

impl Plugin for SectorManagerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SectorManager>()
            .init_resource::<EntityTracker>()
            .add_systems(
                Update,
                (
                    handle_entity_transitions,
                    cleanup_despawned_entities,
                    check_deactivation_candidates,
                    rebalance_sectors,
                    log_sector_assignment_status,
                ),
            );
    }
}
