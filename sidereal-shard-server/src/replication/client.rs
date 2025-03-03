use bevy::math::Vec2;
use bevy::prelude::*;
use bevy::time::Time;
use std::collections::HashMap;
use uuid::Uuid;

//pub use sidereal_core::ecs::plugins::replication::client::ReplicationClient;
pub use sidereal_core::ecs::plugins::replication::common::{
    EntityState, EntityUpdateType, ReplicationClientStatus,
};

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
            max_retries: 10,
            retry_interval: 2.0, // Increased from 0.5
            timeout: 60.0,       // Increased from 30.0
            connection_time: 0.0,
        }
    }
}

impl HandshakeTracker {
    /// Reset the handshake tracker
    pub fn reset(&mut self) {
        self.last_attempt = 0.0;
        self.retries = 0;
        // Keep connection_time unchanged to preserve reference to original time
    }

    /// Check if we should retry sending a handshake
    pub fn should_retry(&self, current_time: f64) -> bool {
        // No retry needed if we're over the retry limit
        if self.retries >= self.max_retries {
            return false;
        }

        // First attempt (retries=0) should happen immediately
        if self.retries == 0 && self.last_attempt == 0.0 {
            return true;
        }

        // Use a dynamic retry interval based on the retry count to be more aggressive
        let dynamic_interval = match self.retries {
            0..=2 => 0.5, // First 3 retries: every 0.5 seconds
            3..=9 => 1.0, // Next 7 retries: every 1 second
            _ => 3.0,     // Remaining retries: every 3 seconds
        };

        // Check if enough time has passed since the last attempt
        current_time - self.last_attempt >= dynamic_interval
    }

    /// Record a handshake attempt
    pub fn record_attempt(&mut self, current_time: f64) {
        self.last_attempt = current_time;
        self.retries += 1;
    }

    /// Check if the handshake has timed out
    pub fn is_timed_out(&self, current_time: f64, custom_timeout: Option<f64>) -> bool {
        let effective_timeout = custom_timeout.unwrap_or(self.timeout);
        self.retries >= self.max_retries
            || (self.last_attempt > 0.0 && current_time - self.last_attempt > effective_timeout)
    }

    pub fn create_handshake_message(&self, shard_id: &Uuid) -> String {
        format!(
            "{{\"version\":\"1.0\",\"shard_id\":\"{}\",\"protocol_version\":\"{}.{}\"}}",
            shard_id,
            env!("CARGO_PKG_VERSION_MAJOR"),
            env!("CARGO_PKG_VERSION_MINOR")
        )
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
        let position = Vec2::new(transform.translation.x, transform.translation.y);

        // Get the last known position and velocity
        if let Some((last_pos, last_vel)) = self.entity_positions.get(&entity) {
            // Check if position has changed significantly (>0.5 units)
            let pos_changed = (position - *last_pos).length_squared() > 0.25;

            // Check if velocity has changed significantly (>0.1 units/s)
            let vel_changed = (velocity - *last_vel).length_squared() > 0.01;

            // If either has changed, update our tracking and return true
            if pos_changed || vel_changed {
                self.entity_positions.insert(entity, (position, velocity));
                return true;
            }

            false
        } else {
            // First time seeing this entity, always needs update
            self.entity_positions.insert(entity, (position, velocity));
            true
        }
    }

    /// Check if it's time to send entity updates
    pub fn should_send_entity_updates(&mut self, time: &Time, interval: f64) -> bool {
        let current_time = time.elapsed_secs_f64();

        if current_time - self.last_update_time > interval {
            self.last_update_time = current_time;
            return true;
        }

        false
    }
}
