use bevy::prelude::*;
use bevy::time::Time;
use bevy::math::{Vec2, IVec2};
use tracing::{info, warn, error};
use std::collections::{HashMap, HashSet};
use std::time::Duration;
use uuid::Uuid;
use bevy_replicon::prelude::*;
use bevy_replicon_renet2::renet2::RenetClient;
use bevy_replicon_renet2::netcode::NetcodeClientTransport;

use super::common::{
    ReplicationClientStatus, EntityState, EntityUpdateType,
    ClientStreamEvent, get_backoff_time, MAX_CONNECTION_ATTEMPTS
};
use super::network::{ConnectionConfig, RepliconSetup};

/// Resource to track replication client state
#[derive(Resource)]
pub struct ReplicationClient {
    pub status: ReplicationClientStatus,
    pub server_id: Option<Uuid>,
    pub assigned_clusters: Vec<(IVec2, Uuid)>, // Cluster coordinates and their IDs
    pub last_heartbeat: Duration,
    pub last_entity_update: Duration,
    
    // Connection tracking
    pub connection_attempts: u32,
    pub last_connection_attempt: f64,
    
    // Tracks entities that need to be sent to the replication server
    pub pending_entity_updates: HashSet<Entity>,
    pub entity_update_types: HashMap<Entity, EntityUpdateType>,
    
    // Last known position cache for determining which entities need updates
    pub entity_last_known_state: HashMap<Entity, EntityState>,
}

impl Default for ReplicationClient {
    fn default() -> Self {
        Self {
            status: ReplicationClientStatus::Disconnected,
            server_id: None,
            assigned_clusters: Vec::new(),
            last_heartbeat: Duration::ZERO,
            last_entity_update: Duration::ZERO,
            connection_attempts: 0,
            last_connection_attempt: 0.0,
            pending_entity_updates: HashSet::new(),
            entity_update_types: HashMap::new(),
            entity_last_known_state: HashMap::new(),
        }
    }
}

impl ReplicationClient {
    /// Queue an entity for update to be sent to the replication server
    pub fn queue_entity_update(&mut self, entity: Entity, update_type: EntityUpdateType) {
        // For Create/Update types, we always send the latest state
        // For Delete, we only queue it if it's not already queued for Create/Update
        match update_type {
            EntityUpdateType::Create | EntityUpdateType::Update => {
                self.entity_update_types.insert(entity, update_type);
                self.pending_entity_updates.insert(entity);
            }
            EntityUpdateType::Delete => {
                if !matches!(self.entity_update_types.get(&entity), 
                    Some(EntityUpdateType::Create) | Some(EntityUpdateType::Update)) {
                    self.entity_update_types.insert(entity, EntityUpdateType::Delete);
                    self.pending_entity_updates.insert(entity);
                }
            }
        }
    }
    
    /// Update the last heartbeat time
    pub fn update_heartbeat(&mut self, time: &Time) {
        self.last_heartbeat = time.elapsed();
    }
    
    /// Check if we're due to send another heartbeat
    pub fn should_send_heartbeat(&self, time: &Time, interval_seconds: f64) -> bool {
        let interval = Duration::from_secs_f64(interval_seconds);
        time.elapsed() - self.last_heartbeat >= interval
    }
    
    /// Update the last entity update time
    pub fn update_entity_update_time(&mut self, time: &Time) {
        self.last_entity_update = time.elapsed();
    }
    
    /// Check if we're due to send entity updates
    pub fn should_send_entity_updates(&self, time: &Time, interval_seconds: f64) -> bool {
        let interval = Duration::from_secs_f64(interval_seconds);
        !self.pending_entity_updates.is_empty() && 
        time.elapsed() - self.last_entity_update >= interval
    }
    
    /// Check if an entity needs to be updated based on its current state
    pub fn entity_needs_update(&mut self, entity: Entity, transform: &Transform, velocity: Vec2) -> bool {
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();
            
        let position = transform.translation;
        
        // If the entity is already pending, it needs an update
        if self.pending_entity_updates.contains(&entity) {
            return true;
        }
        
        // If we've never seen this entity before, it needs an update
        if !self.entity_last_known_state.contains_key(&entity) {
            self.entity_last_known_state.insert(entity, EntityState {
                position,
                velocity,
                last_update: current_time,
            });
            return true;
        }
        
        // Get the last known state
        let last_state = self.entity_last_known_state.get(&entity).unwrap();
        
        // Check if position or velocity has changed significantly
        let position_changed = (last_state.position - position).length() > 0.1;
        let velocity_changed = (last_state.velocity - velocity).length() > 0.1;
        
        // If something changed, update the last known state and return true
        if position_changed || velocity_changed {
            self.entity_last_known_state.insert(entity, EntityState {
                position,
                velocity,
                last_update: current_time,
            });
            return true;
        }
        
        // Check if we need a periodic update regardless of change
        let periodic_update_interval = 5.0; // Send an update every 5 seconds at minimum
        if current_time - last_state.last_update > periodic_update_interval {
            self.entity_last_known_state.insert(entity, EntityState {
                position,
                velocity,
                last_update: current_time,
            });
            return true;
        }
        
        false
    }
    
    /// Handle cluster assignment from replication server
    pub fn assign_cluster(&mut self, coordinates: IVec2, cluster_id: Uuid) {
        if !self.assigned_clusters.iter().any(|(coords, _)| *coords == coordinates) {
            self.assigned_clusters.push((coordinates, cluster_id));
        }
    }
    
    /// Handle cluster unassignment from replication server
    pub fn unassign_cluster(&mut self, coordinates: IVec2) {
        self.assigned_clusters.retain(|(coords, _)| *coords != coordinates);
    }
}

/// Replication client plugin for connecting to the replication server
pub struct ReplicationClientPlugin;

impl Plugin for ReplicationClientPlugin {
    fn build(&self, app: &mut App) {
        info!("Building core replication client plugin");
        
        // Register events
        app.add_event::<ClientStreamEvent>();
        
        // Add client resource
        app.init_resource::<ReplicationClient>();
        
        // Add systems
        app.add_systems(Update, (
            monitor_connection_status,
        ));
    }
}

/// Monitor connection status and handle reconnection
fn monitor_connection_status(
    mut client: ResMut<ReplicationClient>,
    renet_client: Option<Res<RenetClient>>,
    transport: Option<Res<NetcodeClientTransport>>,
    time: Res<Time>,
) {
    let current_time = time.elapsed_secs_f64();
    
    // Handle different connection states
    match client.status {
        ReplicationClientStatus::Disconnected => {
            // Check if we should attempt to reconnect
            let backoff_time = get_backoff_time(client.connection_attempts);
            if current_time - client.last_connection_attempt >= backoff_time {
                info!("Initiating reconnection to replication server (attempt {})", 
                      client.connection_attempts + 1);
                
                // Set status to pending - concrete implementations should handle the actual reconnection
                client.status = ReplicationClientStatus::ConnectionPending;
            }
        },
        ReplicationClientStatus::Connecting => {
            // Check if client and transport exist
            if let (Some(renet_client), Some(transport)) = (renet_client, transport) {
                if renet_client.is_connected() {
                    info!("Successfully connected to replication server");
                    client.status = ReplicationClientStatus::Connected;
                    client.connection_attempts = 0;
                } else if transport.is_connected() {
                    // Client authenticated but not fully connected
                    info!("Authenticated with replication server, completing connection");
                } else if current_time - client.last_connection_attempt > 5.0 {
                    // Connection timeout
                    warn!("Connection attempt timed out");
                    client.status = ReplicationClientStatus::ConnectionFailed;
                    client.connection_attempts += 1;
                }
            }
        },
        ReplicationClientStatus::Connected => {
            // Check if we're still connected
            if let (Some(renet_client), _) = (renet_client, transport) {
                if !renet_client.is_connected() {
                    warn!("Lost connection to replication server");
                    client.status = ReplicationClientStatus::Disconnected;
                    client.last_connection_attempt = current_time;
                } else if client.should_send_heartbeat(&time, 5.0) {
                    // Send heartbeat
                    info!("Connected to replication server - heartbeat");
                    
                    // Update the last heartbeat time to prevent spamming
                    client.update_heartbeat(&time);
                }
            }
        },
        ReplicationClientStatus::ConnectionFailed => {
            // Check if we should retry
            if client.connection_attempts >= MAX_CONNECTION_ATTEMPTS {
                error!("Failed to connect to replication server after {} attempts", 
                       MAX_CONNECTION_ATTEMPTS);
            } else {
                let backoff_time = get_backoff_time(client.connection_attempts);
                if current_time - client.last_connection_attempt >= backoff_time {
                    info!("Retrying connection to replication server (attempt {})", 
                          client.connection_attempts + 1);
                    client.status = ReplicationClientStatus::ConnectionPending;
                }
            }
        },
        _ => {}
    }
} 