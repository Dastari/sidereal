use bevy::prelude::*;
use crate::ecs::components::*;
use crate::ecs::components::physics::{Velocity, Mass, Fixed};

/// A simple physics system that applies velocity to position
pub fn physics_system(
    mut query: Query<(&mut Position, &Velocity, &Mass), Without<Fixed>>,
    time: Res<Time>,
) {
    for (mut pos, vel, _mass) in query.iter_mut() {
        // Scale by delta time for frame-rate independence
        let dt = time.delta();
        
        // Update position based on velocity and time (we don't apply inverse mass here)
        let dx = vel.x * dt.as_secs_f32();
        let dy = vel.y * dt.as_secs_f32();
        
        pos.x += dx;
        pos.y += dy;
    }
} 