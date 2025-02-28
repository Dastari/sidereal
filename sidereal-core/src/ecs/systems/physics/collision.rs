use bevy::prelude::*;
use crate::ecs::components::*;
use crate::ecs::components::physics::{Velocity, Mass};

/// A system to handle simple collision detection and resolution
pub fn collision_system(
    query: Query<(Entity, &Position, &Velocity, &Mass)>,
    mut velocity_query: Query<&mut Velocity>,
) {
    // We need to collect in a way that works with Bevy's borrow checker
    let mut colliders = Vec::new();
    for (entity, position, velocity, mass) in query.iter() {
        colliders.push((entity, position.clone(), velocity.clone(), mass.0));
    }
    
    // Simple collision detection (very basic)
    for (i, (entity_a, pos_a, vel_a, mass_a)) in colliders.iter().enumerate() {
        for (entity_b, pos_b, vel_b, mass_b) in colliders.iter().skip(i + 1) {
            // Simple distance-based collision (assuming circular objects with radius 1.0)
            let dx = pos_b.x - pos_a.x;
            let dy = pos_b.y - pos_a.y;
            let distance = (dx * dx + dy * dy).sqrt();
            
            // If overlapping, resolve the collision
            if distance < 2.0 {  // 2.0 = radius of A + radius of B (assuming both 1.0)
                // Calculate collision normal
                let nx = dx / distance;
                let ny = dy / distance;
                
                // Calculate relative velocity
                let rvx = vel_b.x - vel_a.x;
                let rvy = vel_b.y - vel_a.y;
                
                // Calculate relative velocity along the normal
                let rv_normal = rvx * nx + rvy * ny;
                
                // Only resolve if objects are moving toward each other
                if rv_normal < 0.0 {
                    // Calculate restitution (bounciness)
                    let restitution = 0.8;
                    
                    // Calculate impulse scalar
                    let inv_mass_sum = 1.0 / mass_a + 1.0 / mass_b;
                    let impulse = -(1.0 + restitution) * rv_normal / inv_mass_sum;
                    
                    // Apply impulse to velocities
                    if let Ok(mut vel_a_mut) = velocity_query.get_mut(*entity_a) {
                        vel_a_mut.x -= impulse * nx / mass_a;
                        vel_a_mut.y -= impulse * ny / mass_a;
                    }
                    
                    if let Ok(mut vel_b_mut) = velocity_query.get_mut(*entity_b) {
                        vel_b_mut.x += impulse * nx / mass_b;
                        vel_b_mut.y += impulse * ny / mass_b;
                    }
                }
            }
        }
    }
} 