use bevy::{prelude::*, time::common_conditions::on_timer};
use bevy_renet::renet::*;
use sidereal_core::ecs::{
    systems::network::{NetworkMessage, NetworkMessageEvent},
    systems::sectors::{SectorCoord, SectorManager},
};
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

// Add this struct to track pending assignments

struct PendingAssignment {
    sectors: HashSet<SectorCoord>,
    timestamp: Instant,
}

#[derive(Resource)]
pub struct ShardManager {
    // Maps client_id to the sectors it's responsible for
    pub shard_sectors: HashMap<u64, HashSet<SectorCoord>>,
    // Maps sector coordinates to the client_id responsible for it
    pub sector_assignments: HashMap<SectorCoord, u64>,
    // Tracks load factor for each shard (0.0 - 1.0)
    pub shard_loads: HashMap<u64, f32>,
    // Whether a shard has fully initialized
    pub active_shards: HashSet<u64>,
    // Pending sector assignments awaiting confirmation
    pending_assignments: HashMap<u64, PendingAssignment>,
    // Timeout for pending assignments
    assignment_timeout: Duration,
}

impl Default for ShardManager {
    fn default() -> Self {
        Self {
            shard_sectors: HashMap::new(),
            sector_assignments: HashMap::new(),
            shard_loads: HashMap::new(),
            active_shards: HashSet::new(),
            pending_assignments: HashMap::new(),
            assignment_timeout: Duration::from_secs(10), // 10 second timeout
        }
    }
}

impl ShardManager {
    // Register a new shard when it connects
    pub fn register_shard(&mut self, client_id: u64) {
        self.shard_sectors.insert(client_id, HashSet::new());
        self.shard_loads.insert(client_id, 0.0);
        info!("Registered new shard server with client_id: {}", client_id);
    }

    // Remove a shard when it disconnects
    pub fn remove_shard(&mut self, client_id: u64) -> Vec<SectorCoord> {
        let orphaned_sectors = match self.shard_sectors.remove(&client_id) {
            Some(sectors) => sectors.into_iter().collect::<Vec<_>>(),
            None => Vec::new(),
        };

        // Remove assignments for orphaned sectors
        for sector in &orphaned_sectors {
            self.sector_assignments.remove(sector);
        }

        self.shard_loads.remove(&client_id);
        self.active_shards.remove(&client_id);

        info!("Removed shard server with client_id: {}", client_id);

        orphaned_sectors
    }

    // Modify assign_sector to handle the pending state
    pub fn assign_sector(&mut self, client_id: u64, sector: SectorCoord, pending: bool) -> bool {
        if !self.shard_sectors.contains_key(&client_id)
            && !self.pending_assignments.contains_key(&client_id)
        {
            warn!(
                "Attempted to assign sector to non-existent shard: {}",
                client_id
            );
            return false;
        }

        if pending {
            // Add to pending assignments
            if let Some(pending_assignment) = self.pending_assignments.get_mut(&client_id) {
                pending_assignment.sectors.insert(sector);
            } else {
                let mut sectors = HashSet::new();
                sectors.insert(sector);
                self.pending_assignments.insert(
                    client_id,
                    PendingAssignment {
                        sectors,
                        timestamp: Instant::now(),
                    },
                );
            }
            return true;
        }

        // Regular assignment logic for confirmed sectors
        // Check if sector is already assigned to another shard
        if let Some(current_shard) = self.sector_assignments.get(&sector) {
            if *current_shard != client_id {
                // Remove sector from previous shard
                if let Some(sectors) = self.shard_sectors.get_mut(current_shard) {
                    sectors.remove(&sector);
                }
            }
        }

        // Assign sector to new shard
        if let Some(sectors) = self.shard_sectors.get_mut(&client_id) {
            sectors.insert(sector);
        }
        self.sector_assignments.insert(sector, client_id);

        true
    }

    // New method to confirm assignments
    pub fn confirm_sector_assignments(&mut self, client_id: u64, sectors: &[SectorCoord]) {
        // First, collect the sectors that need to be confirmed
        let sectors_to_confirm: Vec<SectorCoord> =
            if let Some(pending) = self.pending_assignments.get_mut(&client_id) {
                let mut confirmed = Vec::new();

                // Remove confirmed sectors from pending and collect them
                for sector in sectors {
                    if pending.sectors.remove(sector) {
                        confirmed.push(*sector);
                    }
                }

                // If all pending assignments are confirmed, remove the pending entry
                if pending.sectors.is_empty() {
                    self.pending_assignments.remove(&client_id);
                }

                confirmed
            } else {
                Vec::new()
            };

        // Now assign the confirmed sectors
        for sector in sectors_to_confirm {
            self.assign_sector(client_id, sector, false);
        }
    }

    // New method to check for timed-out assignments
    pub fn check_assignment_timeouts(&mut self) -> Vec<(u64, Vec<SectorCoord>)> {
        let now = Instant::now();
        let mut timed_out = Vec::new();
        let mut to_remove = Vec::new();

        for (client_id, pending) in &self.pending_assignments {
            if now.duration_since(pending.timestamp) > self.assignment_timeout {
                // This assignment has timed out
                let sectors: Vec<_> = pending.sectors.iter().cloned().collect();
                timed_out.push((*client_id, sectors));
                to_remove.push(*client_id);
            }
        }

        // Remove timed-out assignments
        for client_id in to_remove {
            self.pending_assignments.remove(&client_id);
        }

        timed_out
    }

    // Get all sectors assigned to a shard
    // pub fn get_shard_sectors(&self, client_id: u64) -> Vec<SectorCoord> {
    //     match self.shard_sectors.get(&client_id) {
    //         Some(sectors) => sectors.iter().cloned().collect(),
    //         None => Vec::new(),
    //     }
    // }

    // Find the best shard to handle a specific sector
    pub fn find_best_shard_for_sector(&self, _sector: SectorCoord) -> Option<u64> {
        if self.active_shards.is_empty() {
            return None;
        }

        // Simplistic approach for now: find shard with lowest load
        // In a real implementation, you'd also consider proximity to other sectors
        self.active_shards
            .iter()
            .min_by(|&a, &b| {
                let load_a = self.shard_loads.get(a).unwrap_or(&1.0);
                let load_b = self.shard_loads.get(b).unwrap_or(&1.0);
                load_a.partial_cmp(load_b).unwrap()
            })
            .copied()
    }

    // Update load information for a shard
    pub fn update_shard_load(&mut self, client_id: u64, load_factor: f32) {
        self.shard_loads.insert(client_id, load_factor);
    }

    // Activate a shard (mark it as fully initialized and ready to take sectors)
    pub fn activate_shard(&mut self, client_id: u64) {
        if self.shard_sectors.contains_key(&client_id) {
            self.active_shards.insert(client_id);
            info!("Shard {} is now active", client_id);
        }
    }

    // Calculate which sectors should be assigned to a new shard
    pub fn calculate_sector_assignment(
        &self,
        client_id: u64,
        sector_manager: &SectorManager,
    ) -> Vec<SectorCoord> {
        let mut sectors_to_assign = Vec::new();

        // For initial implementation, just evenly distribute unassigned sectors
        for (coord, _sector) in &sector_manager.sectors {
            // Skip sectors that are assigned or pending assignment
            if self.sector_assignments.contains_key(coord) {
                continue;
            }

            // Also check if this sector is in any pending assignment
            let is_pending = self
                .pending_assignments
                .values()
                .any(|pending| pending.sectors.contains(coord));

            if !is_pending {
                sectors_to_assign.push(*coord);
            }
        }

        // If we have other active shards, try to redistribute some sectors
        if self.active_shards.len() > 1 {
            let target_sectors_per_shard = (sector_manager.sectors.len() as f32
                / self.active_shards.len() as f32)
                .ceil() as usize;

            for &active_shard in &self.active_shards {
                if active_shard == client_id {
                    continue;
                }

                if let Some(shard_sectors) = self.shard_sectors.get(&active_shard) {
                    // If this shard has more than the target number of sectors, redistribute some
                    if shard_sectors.len() > target_sectors_per_shard {
                        // Take up to half of the excess sectors from this overloaded shard
                        let excess = shard_sectors.len() - target_sectors_per_shard;
                        let transfer_count = excess / 2;

                        // For simplicity, just take any sectors
                        // In a real implementation, you'd consider proximity and load
                        sectors_to_assign
                            .extend(shard_sectors.iter().take(transfer_count).cloned());
                    }
                }
            }
        }

        sectors_to_assign
    }
}

// Modified system to handle shard connection
pub fn handle_shard_connection(
    mut shard_manager: ResMut<ShardManager>,
    mut server: ResMut<RenetServer>,
    mut network_events: EventReader<NetworkMessageEvent>,
    sector_manager: Res<SectorManager>,
) {
    for event in network_events.read() {
        match &event.message {
            NetworkMessage::ShardConnected => {
                info!("Shard server connected: {}", event.client_id);

                // Register the new shard
                shard_manager.register_shard(event.client_id);

                // Calculate which sectors to assign to this shard
                let sectors_to_assign =
                    shard_manager.calculate_sector_assignment(event.client_id, &sector_manager);

                // Mark these sectors as pending assignment
                for sector in &sectors_to_assign {
                    shard_manager.assign_sector(event.client_id, *sector, true);
                }

                // Send the sector assignments to the shard
                let message = bincode::encode_to_vec(
                    &NetworkMessage::AssignSectors {
                        sectors: sectors_to_assign,
                    },
                    bincode::config::standard(),
                )
                .unwrap();

                server.send_message(event.client_id, DefaultChannel::ReliableOrdered, message);

                // Note: We don't mark the shard as active until it confirms the assignments
            }
            NetworkMessage::ShardDisconnected => {
                info!("Shard server disconnected: {}", event.client_id);

                // Get sectors that need reassignment
                let orphaned_sectors = shard_manager.remove_shard(event.client_id);

                // Reassign orphaned sectors to other shards
                for sector in orphaned_sectors {
                    if let Some(new_shard) = shard_manager.find_best_shard_for_sector(sector) {
                        shard_manager.assign_sector(new_shard, sector, true);

                        // Notify shard of new assignment
                        let message = bincode::encode_to_vec(
                            &NetworkMessage::AssignSectors {
                                sectors: vec![sector],
                            },
                            bincode::config::standard(),
                        )
                        .unwrap();

                        server.send_message(new_shard, DefaultChannel::ReliableOrdered, message);
                    }
                }
            }
            NetworkMessage::SectorAssignmentConfirm { sectors } => {
                // Shard confirms it has taken responsibility for sectors
                info!(
                    "Shard {} confirmed {} sector assignments",
                    event.client_id,
                    sectors.len()
                );

                // Finalize the sector assignments
                shard_manager.confirm_sector_assignments(event.client_id, sectors);

                // Mark the shard as active if it wasn't already
                shard_manager.activate_shard(event.client_id);
            }
            NetworkMessage::SectorLoadReport { load_factor } => {
                // Update the load factor for this shard
                shard_manager.update_shard_load(event.client_id, *load_factor);

                // You could implement load balancing here by redistributing sectors
                // based on updated load information
            }
            _ => {} // Ignore other messages
        }
    }
}

// Add a new system to handle timeout checks
pub fn check_assignment_timeouts(
    mut shard_manager: ResMut<ShardManager>,
    mut server: ResMut<RenetServer>,
) {
    let timed_out = shard_manager.check_assignment_timeouts();

    for (client_id, sectors) in timed_out {
        warn!(
            "Assignment timeout for shard {}: {} sectors",
            client_id,
            sectors.len()
        );

        // You could choose to:
        // 1. Retry sending the assignments
        // 2. Reassign the sectors to other shards
        // 3. Mark the shard as problematic

        // For now, let's just try to reassign the sectors
        for sector in sectors {
            if let Some(new_shard) = shard_manager.find_best_shard_for_sector(sector) {
                if new_shard != client_id {
                    shard_manager.assign_sector(new_shard, sector, true);

                    // Notify the new shard
                    let message = bincode::encode_to_vec(
                        &NetworkMessage::AssignSectors {
                            sectors: vec![sector],
                        },
                        bincode::config::standard(),
                    )
                    .unwrap();

                    server.send_message(new_shard, DefaultChannel::ReliableOrdered, message);
                }
            }
        }
    }
}

pub fn balance_sectors(
    mut _shard_manager: ResMut<ShardManager>,
    mut _server: ResMut<RenetServer>,
    _time: Res<Time>,
) {
    // This would be the code to periodically rebalance sectors
    // Make sure to use the pending assignment mechanism here too
    // by marking sectors as pending when reassigning them
}

pub struct ShardManagerPlugin;

impl Plugin for ShardManagerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ShardManager>();
        app.add_systems(
            Update,
            (
                handle_shard_connection,
                check_assignment_timeouts.run_if(on_timer(std::time::Duration::from_secs(5))),
                balance_sectors.run_if(on_timer(std::time::Duration::from_secs(30))),
            ),
        );
    }
}
