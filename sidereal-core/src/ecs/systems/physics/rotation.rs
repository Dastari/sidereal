use bevy::prelude::*;
use crate::ecs::components::*;
use crate::ecs::components::physics::{AngularVelocity, Mass};
use std::f32::consts::TAU;

/// A system that applies angular velocity to rotation
pub fn rotation_system(
    mut query: Query<(&mut Rotation, &AngularVelocity, &Mass)>,
    time: Res<Time>,
) {
    for (mut rotation, angular_vel, mass) in query.iter_mut() {
        // Scale rotation by inverse mass - heavier objects rotate slower
        let inverse_mass = 1.0 / mass.0;
        
        // Apply rotation using delta time for frame independence
        let dt = time.delta();
        
        // Update rotation (in radians) based on angular velocity
        rotation.0 += angular_vel.0 * inverse_mass * dt.as_secs_f32();
        
        // Keep rotation within the range of 0 to 2π
        rotation.0 %= TAU; // TAU is 2π
        
        // Ensure we don't have negative angles
        if rotation.0 < 0.0 {
            rotation.0 += TAU;
        }
    }
} 