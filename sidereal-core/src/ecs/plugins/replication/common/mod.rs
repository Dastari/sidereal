use bevy::prelude::*;
use bevy::math::Vec2;
use uuid::Uuid;
use std::collections::{HashMap, HashSet};
use std::time::Duration;

/// Type of entity update being sent or received
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityUpdateType {
    Create,
    Update,
    Delete,
}

/// Client status for replication connection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplicationClientStatus {
    Disconnected,
    ConnectionPending,
    Connecting,
    Connected,
    Authenticated,
    ConnectionFailed,
}

/// Structure to track entity state for change detection
#[derive(Clone)]
pub struct EntityState {
    pub position: Vec3,
    pub velocity: Vec2,
    pub last_update: f64,
}

/// Common events for entity updates
#[derive(Event)]
pub struct EntityUpdateEvent {
    pub entity: Entity,
    pub position: Vec2,
    pub velocity: Vec2,
    pub update_type: EntityUpdateType,
}

/// Event for sending client data to server
#[derive(Event)]
pub struct ClientStreamEvent {
    pub event_type: String,
    pub data: String,
}

// Network constants
pub const CONNECTION_RETRY_BASE_DELAY: f64 = 1.0;
pub const MAX_CONNECTION_ATTEMPTS: u32 = 5;

// Helper to calculate exponential backoff time for reconnection attempts
pub fn get_backoff_time(retry_count: u32) -> f64 {
    let base_time = CONNECTION_RETRY_BASE_DELAY;
    let max_time = 60.0;
    let backoff = base_time * (2.0_f64).powi(retry_count as i32);
    backoff.min(max_time)
} 