use bevy::math::Vec2;
use bevy::prelude::*;
use bevy::time::Time;
use bevy_rapier2d::prelude::Velocity;
use serde::Serialize;
use std::collections::HashMap;
use std::time::Duration;
use uuid::Uuid;

pub use sidereal_core::ecs::plugins::replication::common::{
    EntityState, EntityUpdateType, ReplicationClientStatus,
};
use sidereal_core::ecs::components::spatial::SpatialPosition;

/// Resource to track handshake attempts and timeouts
#[derive(Resource)]
pub struct HandshakeTracker {
    /// When was the last handshake attempt made
    pub last_attempt: f64,
    /// Number of retries attempted
    pub retries: u32,
    /// Maximum number of retries before timing out
    pub max_retries: u32,
    /// Interval between retry attempts in seconds
    pub retry_interval: f64,
    /// Overall timeout for handshake process in seconds
    pub timeout: f64,
    /// Timestamp of when the connection was established (using system time)
    pub connection_time: f64,
}

impl Default for HandshakeTracker {
    fn default() -> Self {
        Self {
            last_attempt: 0.0,
            retries: 0,
            max_retries: 5,
            retry_interval: 2.0,
            timeout: 30.0,
            connection_time: 0.0,
        }
    }
}

impl HandshakeTracker {
    /// Reset the handshake tracker
    pub fn reset(&mut self) {
        self.last_attempt = 0.0;
        self.retries = 0;
        self.connection_time = 0.0;
    }

    /// Check if we should retry sending a handshake
    pub fn should_retry(&self, current_time: f64) -> bool {
        if self.last_attempt == 0.0 || (current_time - self.last_attempt) >= self.retry_interval {
            if self.retries < self.max_retries {
                return true;
            }
        }
        false
    }

    /// Record a handshake attempt
    pub fn record_attempt(&mut self, current_time: f64) {
        self.last_attempt = current_time;
        self.retries += 1;
    }

    /// Check if the handshake has timed out
    pub fn is_timed_out(&self, current_time: f64, custom_timeout: Option<f64>) -> bool {
        let timeout = custom_timeout.unwrap_or(self.timeout);
        self.last_attempt > 0.0 && (current_time - self.last_attempt) > timeout
    }

    pub fn create_handshake_message(&self, shard_id: &Uuid) -> String {
        serde_json::json!({
            "type": "handshake",
            "shard_id": shard_id.to_string(),
            "timestamp": self.last_attempt,
            "version": env!("CARGO_PKG_VERSION"),
        })
        .to_string()
    }
}

/// Track the state of the connection to the replication server
#[derive(Resource, Default)]
pub struct ShardConnectionState {
    pub is_connected: bool,
}

/// Resource to track entity state changes
#[derive(Resource, Default)]
pub struct EntityChangeTracker {
    entity_positions: HashMap<Entity, (Vec2, Vec2)>, // Entity -> (Position, Velocity)
    last_update_time: f64,
}

impl EntityChangeTracker {
    /// Check if an entity has changed position or velocity enough to warrant an update
    pub fn entity_needs_update(
        &mut self,
        entity: Entity,
        transform: &Transform,
        velocity: Vec2,
    ) -> bool {
        let current_pos = Vec2::new(transform.translation.x, transform.translation.y);
        
        if let Some((prev_pos, prev_vel)) = self.entity_positions.get(&entity) {
            let distance = current_pos.distance(*prev_pos);
            let vel_change = (velocity - *prev_vel).length();
            
            self.entity_positions.insert(entity, (current_pos, velocity));
            
            let velocity_magnitude = velocity.length();
            let distance_threshold = if velocity_magnitude > 10.0 {
                0.5
            } else {
                1.0
            };
            
            return distance > distance_threshold || vel_change > 1.0;
        } else {
            self.entity_positions.insert(entity, (current_pos, velocity));
            return true;
        }
    }

    /// Check if it's time to send entity updates
    pub fn should_send_entity_updates(&mut self, time: &Time, interval: f64) -> bool {
        let current_time = time.elapsed_secs_f64();
        if (current_time - self.last_update_time) >= interval {
            self.last_update_time = current_time;
            return true;
        }
        false
    }
}
