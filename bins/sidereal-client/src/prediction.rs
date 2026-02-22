/// Client-side prediction, reconciliation, and interpolation for networked entities.
///
/// Architecture:
/// - **Controlled Entity**: Client predicts locally, server corrects via reconciliation
/// - **Remote Entities**: Interpolated between buffered server snapshots
///
/// Prediction Flow:
/// 1. Client generates input → stores in history
/// 2. Client predicts state forward using sidereal-sim-core
/// 3. Server sends authoritative state + tick
/// 4. Client reconciles: rollback to server state, replay unacked inputs
///
/// Design constraints (from sidereal_design_document.md §5):
/// - No prediction for remote entities (interpolation only)
/// - Shared deterministic math in sidereal-sim-core
/// - Hard snap only for large divergence
/// - Velocity-adaptive correction smoothing
use bevy::prelude::*;
use sidereal_sim_core::EntityKinematics;
use std::collections::VecDeque;

// ===== Controlled Entity Prediction =====

/// Input history for reconciliation
#[derive(Component)]
pub struct InputHistory {
    /// Ordered by tick (oldest first)
    pub entries: VecDeque<InputHistoryEntry>,
    /// Maximum entries to retain
    pub max_size: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct InputHistoryEntry {
    pub tick: u64,
    pub thrust: f32,
    pub turn: f32,
    pub brake: bool,
    pub predicted_state: EntityKinematics,
}

impl Default for InputHistory {
    fn default() -> Self {
        Self {
            entries: VecDeque::with_capacity(128),
            max_size: 128, // ~2 seconds at 60Hz
        }
    }
}

impl InputHistory {
    pub fn push(&mut self, entry: InputHistoryEntry) {
        self.entries.push_back(entry);

        // Prune old entries
        while self.entries.len() > self.max_size {
            self.entries.pop_front();
        }
    }

    pub fn prune_before_tick(&mut self, acked_tick: u64) {
        self.entries.retain(|e| e.tick > acked_tick);
    }
}

use sidereal_game::flight::compute_flight_forces;
use sidereal_game::generated::components::FlightTuning;

pub fn replay_predicted_state_from_authoritative(
    authoritative: EntityKinematics,
    history: &InputHistory,
    acked_tick: u64,
    mass_kg: f32,
    flight_tuning: Option<&FlightTuning>,
    available_thrust: f32,
    brake_available_thrust: f32,
) -> (EntityKinematics, f32) {
    let mut current_state = authoritative;
    let mut current_angular_velocity_z = 0.0;
    const DT: f32 = 1.0 / 30.0;

    for entry in &history.entries {
        if entry.tick <= acked_tick {
            continue;
        }

        let throttle = entry.thrust;
        let yaw_input = entry.turn;
        let brake_active = entry.brake;
        let turn_rate_deg_s = 45.0; // Assume 45.0 for now, should ideally be read from FlightComputer

        let velocity = Vec3::from_array(current_state.velocity_mps);
        let angular_velocity = Vec3::new(0.0, 0.0, current_angular_velocity_z);
        let rotation = Quat::from_rotation_z(current_state.heading_rad);

        let (force, torque) = compute_flight_forces(
            (throttle, yaw_input, turn_rate_deg_s, brake_active),
            velocity,
            angular_velocity,
            rotation,
            mass_kg,
            flight_tuning,
            available_thrust,
            brake_available_thrust,
            DT,
        );

        // Integrate exactly as Avian does (Semi-Implicit Euler)
        // v_new = v_old + (f / m) * dt
        let new_velocity = velocity + (force / mass_kg) * DT;
        
        // p_new = p_old + v_new * dt
        let position = Vec3::from_array(current_state.position_m);
        let new_position = position + new_velocity * DT;

        // angular integration
        // w_new = w_old + (torque / inertia) * dt
        let inertia = mass_kg * 10.0; // Rough approximation of moment of inertia
        let new_angular_velocity = angular_velocity + (torque / inertia) * DT;

        // rotation integration (2D approximation)
        let new_heading_rad = current_state.heading_rad + new_angular_velocity.z * DT;

        current_state.position_m = new_position.to_array();
        current_state.velocity_mps = new_velocity.to_array();
        current_state.heading_rad = new_heading_rad;
        current_angular_velocity_z = new_angular_velocity.z;
    }

    (current_state, current_angular_velocity_z)
}

/// Reconciliation state
#[derive(Component)]
pub struct ReconciliationState {
    pub last_server_tick: u64,
    pub last_acked_input_tick: u64,
    pub last_authoritative_state: Option<EntityKinematics>,
    pub correction_error_m: f32,
    pub correction_timer: f32,
}

impl Default for ReconciliationState {
    fn default() -> Self {
        Self {
            last_server_tick: 0,
            last_acked_input_tick: 0,
            last_authoritative_state: None,
            correction_error_m: 0.0,
            correction_timer: 0.0,
        }
    }
}

// ===== Remote Entity Interpolation =====

/// Component for remote (non-controlled) entities
#[derive(Component)]
pub struct RemoteEntity;

/// Snapshot buffer for interpolation
#[derive(Component)]
pub struct SnapshotBuffer {
    pub snapshots: VecDeque<EntitySnapshot>,
    pub interpolation_delay_s: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct EntitySnapshot {
    pub server_time: f64,
    pub position_m: [f32; 3],
    pub rotation: [f32; 4], // Quaternion
}

impl Default for SnapshotBuffer {
    fn default() -> Self {
        Self {
            snapshots: VecDeque::with_capacity(10),
            interpolation_delay_s: 0.1, // 100ms interpolation delay
        }
    }
}

impl SnapshotBuffer {
    pub fn push(&mut self, snapshot: EntitySnapshot) {
        self.snapshots.push_back(snapshot);

        // Keep last ~1 second of snapshots
        while self.snapshots.len() > 60 {
            self.snapshots.pop_front();
        }
    }

    pub fn interpolate_at(&self, render_time: f64) -> Option<EntitySnapshot> {
        if self.snapshots.is_empty() {
            return None;
        }
        if self.snapshots.len() == 1 {
            let only = *self.snapshots.front()?;
            const MAX_EXTRAPOLATION_S: f64 = 0.05; // 50ms cap
            if render_time - only.server_time < MAX_EXTRAPOLATION_S {
                return Some(only);
            }
            return None;
        }

        // Find bracketing snapshots
        let mut before = None;
        let mut after = None;

        for snap in &self.snapshots {
            if snap.server_time <= render_time {
                before = Some(*snap);
            } else {
                after = Some(*snap);
                break;
            }
        }

        match (before, after) {
            (Some(b), Some(a)) => {
                // Interpolate between snapshots
                let total_time = a.server_time - b.server_time;
                if total_time <= 0.0 {
                    return Some(b);
                }

                let t = ((render_time - b.server_time) / total_time).clamp(0.0, 1.0) as f32;

                Some(EntitySnapshot {
                    server_time: render_time,
                    position_m: [
                        b.position_m[0] + (a.position_m[0] - b.position_m[0]) * t,
                        b.position_m[1] + (a.position_m[1] - b.position_m[1]) * t,
                        b.position_m[2] + (a.position_m[2] - b.position_m[2]) * t,
                    ],
                    rotation: b.rotation, // TODO: slerp quaternions
                })
            }
            (Some(b), None) => {
                // Extrapolate (bounded)
                const MAX_EXTRAPOLATION_S: f64 = 0.05; // 50ms cap
                if render_time - b.server_time < MAX_EXTRAPOLATION_S {
                    Some(b) // Use latest snapshot
                } else {
                    None // Too far ahead
                }
            }
            _ => None,
        }
    }
}

/// Interpolate remote entities from snapshot buffer
pub fn interpolate_remote_entities(
    mut query: Query<(&SnapshotBuffer, &mut Transform), With<RemoteEntity>>,
    _time: Res<Time>,
) {
    for (buffer, mut transform) in &mut query {
        let Some(latest) = buffer.snapshots.back() else {
            continue;
        };
        let render_time = latest.server_time - buffer.interpolation_delay_s as f64;

        if let Some(interpolated) = buffer.interpolate_at(render_time) {
            transform.translation = Vec3::from_array(interpolated.position_m);
            transform.rotation = Quat::from_array(interpolated.rotation);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_history_prunes_old_entries() {
        let mut history = InputHistory::default();

        for tick in 0..150 {
            history.push(InputHistoryEntry {
                tick,
                thrust: 0.0,
                turn: 0.0,
                brake: false,
                predicted_state: EntityKinematics::default(),
            });
        }

        assert!(history.entries.len() <= history.max_size);
        assert_eq!(
            history.entries.front().unwrap().tick,
            150 - history.max_size as u64
        );
    }

    #[test]
    fn input_history_finds_tick() {
        let mut history = InputHistory::default();

        history.push(InputHistoryEntry {
            tick: 100,
            thrust: 0.0,
            turn: 0.0,
            brake: false,
            predicted_state: EntityKinematics::default(),
        });

        assert!(history.entries.iter().any(|e| e.tick == 100));
        assert!(!history.entries.iter().any(|e| e.tick == 99));
    }

    #[test]
    fn replay_from_authoritative_replays_unacked_inputs_only() {
        let mut history = InputHistory::default();
        history.push(InputHistoryEntry {
            tick: 10,
            thrust: 0.0,
            turn: 0.0,
            brake: false,
            predicted_state: EntityKinematics::default(),
        });
        history.push(InputHistoryEntry {
            tick: 11,
            thrust: 0.0,
            turn: 1.0,
            brake: false,
            predicted_state: EntityKinematics {
                heading_rad: 1.25,
                ..Default::default()
            },
        });
        let authoritative = EntityKinematics::default();
        let tuning = sidereal_game::generated::components::FlightTuning {
            max_linear_speed_mps: 600.0,
            max_linear_accel_mps2: 60.0,
            passive_brake_accel_mps2: 3.0,
            active_brake_accel_mps2: 12.0,
            drag_per_s: 0.1,
        };
        let (replayed, _) = replay_predicted_state_from_authoritative(
            authoritative,
            &history,
            10,
            15000.0,
            Some(&tuning),
            0.0,
            0.0,
        );
        assert!(replayed.heading_rad.abs() > f32::EPSILON);
    }

    #[test]
    fn snapshot_buffer_interpolates_between_two_snapshots() {
        let mut buffer = SnapshotBuffer::default();

        buffer.push(EntitySnapshot {
            server_time: 1.0,
            position_m: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
        });

        buffer.push(EntitySnapshot {
            server_time: 2.0,
            position_m: [10.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
        });

        let result = buffer.interpolate_at(1.5).unwrap();

        // Should be halfway between
        assert!((result.position_m[0] - 5.0).abs() < 0.01);
    }

    #[test]
    fn snapshot_buffer_extrapolates_within_bound() {
        let mut buffer = SnapshotBuffer::default();

        buffer.push(EntitySnapshot {
            server_time: 1.0,
            position_m: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
        });

        // Slight extrapolation (within 50ms cap)
        let result = buffer.interpolate_at(1.03);
        assert!(result.is_some());

        // Too far ahead
        let result = buffer.interpolate_at(1.1);
        assert!(result.is_none());
    }
}
