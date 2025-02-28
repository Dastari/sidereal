use bevy::prelude::*;
use crate::ecs::components::*;
use crate::ecs::components::physics::{Velocity, AngularVelocity, Mass};

/// A system to apply torque from collisions (rotational effect of collisions)
pub fn collision_torque_system(
    query: Query<(Entity, &Position, &Velocity, &Mass)>,
    mut angular_velocity_query: Query<&mut AngularVelocity>,
) {
    // Similar to collision system but for angular effects
    let mut colliders = Vec::new();
    for (entity, position, velocity, mass) in query.iter() {
        colliders.push((entity, position.clone(), velocity.clone(), mass.0));
    }
    
    for (i, (entity_a, pos_a, vel_a, mass_a)) in colliders.iter().enumerate() {
        for (entity_b, pos_b, vel_b, mass_b) in colliders.iter().skip(i + 1) {
            // Distance calculation 
            let dx = pos_b.x - pos_a.x;
            let dy = pos_b.y - pos_a.y;
            let distance = (dx * dx + dy * dy).sqrt();
            
            if distance < 2.0 {
                // Calculate collision normal
                let nx = dx / distance;
                let ny = dy / distance;
                
                // Calculate relative velocity
                let rvx = vel_b.x - vel_a.x;
                let rvy = vel_b.y - vel_a.y;
                
                // Calculate tangential component (perpendicular to normal)
                let tx = -ny;  // Perpendicular to normal
                let ty = nx;
                
                // Calculate tangential velocity component
                let tangent_vel = rvx * tx + rvy * ty;
                
                // Calculate torque based on tangential velocity
                let torque_factor = 0.2; // Adjust for more/less rotation effect
                
                // Apply angular velocity change based on tangential component
                if let Ok(mut ang_vel_a) = angular_velocity_query.get_mut(*entity_a) {
                    ang_vel_a.0 += tangent_vel * torque_factor / mass_a;
                }
                
                if let Ok(mut ang_vel_b) = angular_velocity_query.get_mut(*entity_b) {
                    ang_vel_b.0 -= tangent_vel * torque_factor / mass_b;
                }
            }
        }
    }
} 