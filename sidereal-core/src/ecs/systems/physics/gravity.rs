use bevy::prelude::*;
use crate::ecs::components::*;
use crate::ecs::components::physics::{Velocity, Mass, Fixed};
use super::common::GravityMultiplier;

/// A system to apply gravitational fields from massive objects
pub fn gravitational_well_system(
    massive_objects: Query<(Entity, &Position, &Mass), With<Mass>>,
    mut affected_objects: Query<(Entity, &Position, &mut Velocity, &Mass), Without<Fixed>>,
    time: Res<Time>,
    gravity_mult: Option<Res<GravityMultiplier>>,
) {
    // Use the gravity multiplier if available, otherwise default to 1.0
    let gravity_multiplier = gravity_mult.map_or(1.0, |res| res.value);
    
    // Apply the multiplier to the gravitational constant
    let gravitational_constant = 1.0 * gravity_multiplier;
    
    const MIN_MASS_FOR_GRAVITY: f32 = 10.0;
    const MIN_DISTANCE: f32 = 5.0; // Minimum distance to prevent infinite forces
    
    let dt = time.delta().as_secs_f32();
    
    // For each pair of objects, calculate gravitational force
    for (mass_entity, mass_pos, mass_component) in massive_objects.iter() {
        // Skip objects that aren't massive enough to generate gravity
        if mass_component.0 < MIN_MASS_FOR_GRAVITY {
            continue;
        }
        
        for (affected_entity, affected_pos, mut affected_vel, affected_mass) in affected_objects.iter_mut() {
            // Don't apply gravity to self
            if mass_entity == affected_entity {
                continue;
            }
            
            // Calculate distance and direction
            let dx = mass_pos.x - affected_pos.x;
            let dy = mass_pos.y - affected_pos.y;
            let distance_squared = dx * dx + dy * dy;
            let distance = distance_squared.sqrt();
            
            // Skip if too close to prevent extreme forces
            if distance < MIN_DISTANCE {
                continue;
            }
            
            // Normalize direction
            let nx = dx / distance;
            let ny = dy / distance;
            
            // Calculate gravitational force magnitude: F = G * m1 * m2 / r^2
            let force_magnitude = gravitational_constant * (mass_component.0 * affected_mass.0) / distance_squared;
            
            // Convert force to acceleration (F = ma, so a = F/m)
            let acceleration = force_magnitude / affected_mass.0;
            
            // Apply acceleration to velocity
            affected_vel.x += nx * acceleration * dt;
            affected_vel.y += ny * acceleration * dt;
        }
    }
} 