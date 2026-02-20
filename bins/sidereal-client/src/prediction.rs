#![allow(dead_code, unused_imports, unused_variables, unused_mut)]

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
use avian3d::prelude::*;
use bevy::prelude::*;
use sidereal_sim_core::{ControlTuning, EntityKinematics, InputSnapshot};
use std::collections::VecDeque;

// ===== Controlled Entity Prediction =====

/// Component marking the locally-controlled entity
#[derive(Component)]
pub struct ControlledEntity {
    pub control_tuning: ControlTuning,
}

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
    pub input: InputSnapshot,
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

    pub fn find_at_tick(&self, tick: u64) -> Option<&InputHistoryEntry> {
        self.entries.iter().find(|e| e.tick == tick)
    }

    pub fn prune_before_tick(&mut self, acked_tick: u64) {
        self.entries.retain(|e| e.tick >= acked_tick);
    }

    pub fn get_unacked_since(&self, server_tick: u64) -> impl Iterator<Item = &InputHistoryEntry> {
        self.entries.iter().filter(move |e| e.tick > server_tick)
    }
}

/// Reconciliation state
#[derive(Component)]
pub struct ReconciliationState {
    pub last_server_tick: u64,
    pub correction_error_m: f32,
    pub correction_timer: f32,
    pub correction_duration: f32,
}

impl Default for ReconciliationState {
    fn default() -> Self {
        Self {
            last_server_tick: 0,
            correction_error_m: 0.0,
            correction_timer: 0.0,
            correction_duration: 0.15, // 150ms blend
        }
    }
}

/// Client local tick counter
#[derive(Resource, Default)]
pub struct ClientTick(pub u64);

/// Apply client prediction for controlled entity
pub fn predict_controlled_entity(
    mut query: Query<
        (&ControlledEntity, &mut InputHistory, &mut Transform),
        With<ControlledEntity>,
    >,
    time: Res<Time>,
    input: Res<ButtonInput<KeyCode>>,
    mut client_tick: ResMut<ClientTick>,
) {
    let Ok((controlled, mut history, mut transform)) = query.single_mut() else {
        return;
    };

    client_tick.0 += 1;
    let current_tick = client_tick.0;

    // Capture current input
    let input_snap = InputSnapshot {
        thrust_forward: input.pressed(KeyCode::KeyW),
        thrust_reverse: input.pressed(KeyCode::KeyS),
        yaw_left: input.pressed(KeyCode::KeyA),
        yaw_right: input.pressed(KeyCode::KeyD),
    };

    // Get current state from transform
    let current_state = EntityKinematics {
        position_m: transform.translation.to_array(),
        velocity_mps: [0.0, 0.0, 0.0], // TODO: track velocity component
        heading_rad: -transform.rotation.to_euler(EulerRot::ZYX).0, // Z rotation
    };

    // Step forward
    let dt = time.delta_secs();
    // TODO: Reimplement with Avian physics
    let next_state = current_state; // Placeholder: no stepping until Avian integrated
    // let next_state = step_entity_kinematics(&current_state, input_snap, &controlled.control_tuning, dt);

    // Store in history
    history.push(InputHistoryEntry {
        tick: current_tick,
        input: input_snap,
        predicted_state: next_state,
    });

    // Apply to transform
    transform.translation = Vec3::from_array(next_state.position_m);
    transform.rotation = Quat::from_rotation_z(-next_state.heading_rad);
}

/// Reconcile client prediction with authoritative server state
pub fn reconcile_controlled_entity(
    mut query: Query<
        (
            &ControlledEntity,
            &mut InputHistory,
            &mut Transform,
            &mut ReconciliationState,
        ),
        With<ControlledEntity>,
    >,
    time: Res<Time>,
) {
    let Ok((controlled, mut history, mut transform, mut recon)) = query.single_mut() else {
        return;
    };

    // TODO: This will be called when server state arrives via Lightyear messages
    // For now, this is a placeholder showing the reconciliation flow

    // Example reconciliation flow (triggered when server state message arrives):
    // let server_state = /* from network message */;
    // let server_tick = /* from network message */;
    //
    // 1. Find prediction at server tick
    // if let Some(historical) = history.find_at_tick(server_tick) {
    //     let error = calculate_error(&historical.predicted_state, &server_state);
    //
    //     // 2. Check divergence threshold
    //     if error > HARD_SNAP_THRESHOLD {
    //         // Hard snap for large errors
    //         transform.translation = Vec3::from_array(server_state.position_m);
    //         transform.rotation = Quat::from_rotation_z(-server_state.heading_rad);
    //     } else if error > CORRECTION_THRESHOLD {
    //         // Smooth correction
    //         recon.correction_error_m = error;
    //         recon.correction_timer = 0.0;
    //
    //         // 3. Rollback to server state
    //         let mut replay_state = server_state;
    //
    //         // 4. Replay unacked inputs
    //         for entry in history.get_unacked_since(server_tick) {
    //             replay_state = step_entity_kinematics(
    //                 &replay_state,
    //                 entry.input,
    //                 &controlled.control_tuning,
    //                 TICK_DT,
    //             );
    //         }
    //
    //         // Apply replayed state with smooth blend
    //         transform.translation = Vec3::from_array(replay_state.position_m);
    //         transform.rotation = Quat::from_rotation_z(-replay_state.heading_rad);
    //     }
    //
    //     // 5. Prune acknowledged inputs
    //     history.prune_before_tick(server_tick);
    //     recon.last_server_tick = server_tick;
    // }

    // Apply correction smoothing
    if recon.correction_timer < recon.correction_duration {
        recon.correction_timer += time.delta_secs();
        // Blend factor calculation would go here
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
        if self.snapshots.len() < 2 {
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
    time: Res<Time>,
) {
    let current_time = time.elapsed_secs_f64();

    for (buffer, mut transform) in &mut query {
        let render_time = current_time - buffer.interpolation_delay_s as f64;

        if let Some(interpolated) = buffer.interpolate_at(render_time) {
            transform.translation = Vec3::from_array(interpolated.position_m);
            transform.rotation = Quat::from_array(interpolated.rotation);
        }
    }
}

// ===== Constants =====

const HARD_SNAP_THRESHOLD: f32 = 5.0; // 5 meters
const CORRECTION_THRESHOLD: f32 = 0.5; // 0.5 meters
const TICK_DT: f32 = 0.016; // ~60 Hz

fn calculate_error(predicted: &EntityKinematics, authoritative: &EntityKinematics) -> f32 {
    let dx = predicted.position_m[0] - authoritative.position_m[0];
    let dy = predicted.position_m[1] - authoritative.position_m[1];
    let dz = predicted.position_m[2] - authoritative.position_m[2];
    (dx * dx + dy * dy + dz * dz).sqrt()
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
                input: InputSnapshot::default(),
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
            input: InputSnapshot::default(),
            predicted_state: EntityKinematics::default(),
        });

        assert!(history.find_at_tick(100).is_some());
        assert!(history.find_at_tick(99).is_none());
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
