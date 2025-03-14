use bevy::{prelude::*, time::common_conditions::on_timer};
use bevy_renet::renet::*;
use sidereal_core::ecs::{
    components::InSector,
    systems::network::{NetworkMessage, NetworkMessageEvent},
    systems::sectors::{SectorCoord, SectorManager},
};
use sidereal_core::EntitySerializer;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

struct PendingAssignment {
    sectors: HashSet<SectorCoord>,
    timestamp: Instant,
}

#[derive(Event)]
pub struct SendEntityUpdatesEvent {
    pub client_id: u64,
    pub sectors: Vec<SectorCoord>,
}

#[derive(Event, Clone)]
pub struct EntitySerializationEvent {
    pub client_id: u64,
    pub entities: Vec<Entity>,
    pub timestamp: f64,
}

#[derive(Resource)]
pub struct ShardManager {
    pub shard_sectors: HashMap<u64, HashSet<SectorCoord>>,
    pub sector_assignments: HashMap<SectorCoord, u64>,
    pub shard_loads: HashMap<u64, f32>,
    pub active_shards: HashSet<u64>,
    pending_assignments: HashMap<u64, PendingAssignment>,
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
            assignment_timeout: Duration::from_secs(10),
        }
    }
}

impl ShardManager {
    pub fn register_shard(&mut self, client_id: u64) {
        self.shard_sectors.insert(client_id, HashSet::new());
        self.shard_loads.insert(client_id, 0.0);
        info!("Registered new shard server with client_id: {}", client_id);
    }
    pub fn remove_shard(&mut self, client_id: u64) -> Vec<SectorCoord> {
        let orphaned_sectors = match self.shard_sectors.remove(&client_id) {
            Some(sectors) => sectors.into_iter().collect::<Vec<_>>(),
            None => Vec::new(),
        };
        for sector in &orphaned_sectors {
            self.sector_assignments.remove(sector);
        }
        self.shard_loads.remove(&client_id);
        self.active_shards.remove(&client_id);
        info!("Removed shard server with client_id: {}", client_id);
        orphaned_sectors
    }
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
            self.pending_assignments
                .entry(client_id)
                .and_modify(|pa| {
                    pa.sectors.insert(sector);
                })
                .or_insert(PendingAssignment {
                    sectors: {
                        let mut hs = HashSet::new();
                        hs.insert(sector);
                        hs
                    },
                    timestamp: Instant::now(),
                });
            return true;
        }
        if let Some(current_shard) = self.sector_assignments.get(&sector) {
            if *current_shard != client_id {
                if let Some(sectors) = self.shard_sectors.get_mut(current_shard) {
                    sectors.remove(&sector);
                }
            }
        }
        if let Some(sectors) = self.shard_sectors.get_mut(&client_id) {
            sectors.insert(sector);
        }
        self.sector_assignments.insert(sector, client_id);
        true
    }
    pub fn confirm_sector_assignments(&mut self, client_id: u64, sectors: &[SectorCoord]) {
        let sectors_to_confirm: Vec<SectorCoord> =
            if let Some(pending) = self.pending_assignments.get_mut(&client_id) {
                let mut confirmed = Vec::new();
                for sector in sectors {
                    if pending.sectors.remove(sector) {
                        confirmed.push(*sector);
                    }
                }
                if pending.sectors.is_empty() {
                    self.pending_assignments.remove(&client_id);
                }
                confirmed
            } else {
                Vec::new()
            };
        for sector in sectors_to_confirm {
            self.assign_sector(client_id, sector, false);
        }
    }
    pub fn check_assignment_timeouts(&mut self) -> Vec<(u64, Vec<SectorCoord>)> {
        let now = Instant::now();
        let mut timed_out = Vec::new();
        let mut to_remove = Vec::new();
        for (client_id, pending) in &self.pending_assignments {
            if now.duration_since(pending.timestamp) > self.assignment_timeout {
                let sectors: Vec<_> = pending.sectors.iter().cloned().collect();
                timed_out.push((*client_id, sectors));
                to_remove.push(*client_id);
            }
        }
        for client_id in to_remove {
            self.pending_assignments.remove(&client_id);
        }
        timed_out
    }
    pub fn find_best_shard_for_sector(&self, _sector: SectorCoord) -> Option<u64> {
        if self.active_shards.is_empty() {
            return None;
        }
        self.active_shards
            .iter()
            .min_by(|&a, &b| {
                let load_a = self.shard_loads.get(a).unwrap_or(&1.0);
                let load_b = self.shard_loads.get(b).unwrap_or(&1.0);
                load_a.partial_cmp(load_b).unwrap()
            })
            .copied()
    }
    pub fn update_shard_load(&mut self, client_id: u64, load_factor: f32) {
        self.shard_loads.insert(client_id, load_factor);
    }
    pub fn activate_shard(&mut self, client_id: u64) {
        if self.shard_sectors.contains_key(&client_id) {
            self.active_shards.insert(client_id);
            info!("Shard {} is now active", client_id);
        }
    }
    pub fn calculate_sector_assignment(
        &self,
        client_id: u64,
        sector_manager: &SectorManager,
    ) -> Vec<SectorCoord> {
        let mut sectors_to_assign = Vec::new();
        for (coord, _sector) in &sector_manager.sectors {
            if self.sector_assignments.contains_key(coord) {
                continue;
            }
            let is_pending = self
                .pending_assignments
                .values()
                .any(|pending| pending.sectors.contains(coord));
            if !is_pending {
                sectors_to_assign.push(*coord);
            }
        }
        if self.active_shards.len() > 1 {
            let target_sectors_per_shard = (sector_manager.sectors.len() as f32
                / self.active_shards.len() as f32)
                .ceil() as usize;
            for &active_shard in &self.active_shards {
                if active_shard == client_id {
                    continue;
                }
                if let Some(shard_sectors) = self.shard_sectors.get(&active_shard) {
                    if shard_sectors.len() > target_sectors_per_shard {
                        let excess = shard_sectors.len() - target_sectors_per_shard;
                        let transfer_count = excess / 2;
                        sectors_to_assign
                            .extend(shard_sectors.iter().take(transfer_count).cloned());
                    }
                }
            }
        }
        sectors_to_assign
    }
}

pub fn handle_shard_connection(
    mut shard_manager: ResMut<ShardManager>,
    mut server: ResMut<RenetServer>,
    mut network_events: EventReader<NetworkMessageEvent>,
    sector_manager: Res<SectorManager>,
    mut entity_update_events: EventWriter<SendEntityUpdatesEvent>,
) {
    for event in network_events.read() {
        match &event.message {
            NetworkMessage::ShardConnected => {
                info!("Shard server connected: {}", event.client_id);
                shard_manager.register_shard(event.client_id);
                let sectors_to_assign =
                    shard_manager.calculate_sector_assignment(event.client_id, &sector_manager);
                for sector in &sectors_to_assign {
                    shard_manager.assign_sector(event.client_id, *sector, true);
                }
                let message = bincode::encode_to_vec(
                    &NetworkMessage::AssignSectors {
                        sectors: sectors_to_assign,
                    },
                    bincode::config::standard(),
                )
                .unwrap();
                server.send_message(event.client_id, DefaultChannel::ReliableOrdered, message);
            }
            NetworkMessage::ShardDisconnected => {
                info!("Shard server disconnected: {}", event.client_id);
                let orphaned_sectors = shard_manager.remove_shard(event.client_id);
                for sector in orphaned_sectors {
                    if let Some(new_shard) = shard_manager.find_best_shard_for_sector(sector) {
                        shard_manager.assign_sector(new_shard, sector, true);
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
                info!(
                    "Shard {} confirmed {} sector assignments",
                    event.client_id,
                    sectors.len()
                );
                shard_manager.confirm_sector_assignments(event.client_id, sectors);
                shard_manager.activate_shard(event.client_id);
                entity_update_events.send(SendEntityUpdatesEvent {
                    client_id: event.client_id,
                    sectors: sectors.clone(),
                });
            }
            NetworkMessage::SectorLoadReport { load_factor } => {
                shard_manager.update_shard_load(event.client_id, *load_factor);
            }
            _ => {}
        }
    }
}

pub fn process_entity_updates(
    mut events: EventReader<SendEntityUpdatesEvent>,
    query: Query<(Entity, &InSector)>,
    time: Res<Time>,
    mut serialization_events: EventWriter<EntitySerializationEvent>,
) {
    for event in events.read() {
        let sector_set: HashSet<SectorCoord> = event.sectors.iter().cloned().collect();
        let mut entities_to_sync = Vec::new();
        for (entity, in_sector) in query.iter() {
            if sector_set.contains(&in_sector.0) {
                entities_to_sync.push(entity);
            }
        }
        if entities_to_sync.is_empty() {
            info!(
                "No entities found in newly assigned sectors for shard {}",
                event.client_id
            );
            continue;
        }
        info!(
            "Sending {} entities to shard {} for newly assigned sectors",
            entities_to_sync.len(),
            event.client_id
        );
        serialization_events.send(EntitySerializationEvent {
            client_id: event.client_id,
            entities: entities_to_sync,
            timestamp: time.elapsed().as_secs_f64(),
        });
    }
}

pub fn serialize_and_send_entities_exclusive(world: &mut World) {
    let event_iter: Vec<EntitySerializationEvent> = {
        let mut events = world
            .get_resource_mut::<Events<EntitySerializationEvent>>()
            .unwrap();
        events.drain().collect()
    };
    if event_iter.is_empty() {
        return;
    }
    
    // Define a reasonable batch size - adjust this value based on entity size and network capacity
    const BATCH_SIZE: usize = 50;
    
    let mut serialized_results = Vec::new();
    for event in event_iter {
        let mut serialized_entities = Vec::new();
        for entity in event.entities {
            if let Ok(serialized) = world.serialize_entity(entity) {
                serialized_entities.push(serialized);
            } else {
                warn!("Failed to serialize entity {}", entity.index());
            }
        }
        serialized_results.push((event.client_id, event.timestamp, serialized_entities));
    }
    
    let mut server = world.get_resource_mut::<RenetServer>().unwrap();
    for (client_id, timestamp, serialized_entities) in serialized_results {
        if serialized_entities.is_empty() {
            continue;
        }
        
        let total_count = serialized_entities.len();
        info!("Batching {} entities to shard {}", total_count, client_id);
        
        // Split entities into batches and send each batch separately
        for (batch_index, batch) in serialized_entities.chunks(BATCH_SIZE).enumerate() {
            let batch_update = NetworkMessage::EntityUpdates {
                updated_entities: batch.to_vec(),
                timestamp,
            };
            
            match bincode::encode_to_vec(&batch_update, bincode::config::standard()) {
                Ok(message) => {
                    server.send_message(client_id, DefaultChannel::ReliableOrdered, message);
                    info!(
                        "Sent batch {}/{} ({} entities) to shard {}",
                        batch_index + 1,
                        (total_count + BATCH_SIZE - 1) / BATCH_SIZE,
                        batch.len(),
                        client_id
                    );
                    
                    // Optional: Add a small delay between batches to prevent network congestion
                    // Uncomment if needed
                    std::thread::sleep(std::time::Duration::from_millis(100));
                },
                Err(err) => {
                    error!("Failed to encode entity batch: {}", err);
                }
            }
        }
        
        info!(
            "Completed sending {} entities in {} batches to shard {}",
            total_count,
            (total_count + BATCH_SIZE - 1) / BATCH_SIZE,
            client_id
        );
    }
}

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
        for sector in sectors {
            if let Some(new_shard) = shard_manager.find_best_shard_for_sector(sector) {
                if new_shard != client_id {
                    shard_manager.assign_sector(new_shard, sector, true);
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

pub struct ShardManagerPlugin;

impl Plugin for ShardManagerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ShardManager>();
        app.add_event::<SendEntityUpdatesEvent>()
            .add_event::<EntitySerializationEvent>();
        app.add_systems(
            Update,
            (
                handle_shard_connection,
                process_entity_updates,
                check_assignment_timeouts.run_if(on_timer(Duration::from_secs(5))),
                serialize_and_send_entities_exclusive,
            ),
        );
    }
}
