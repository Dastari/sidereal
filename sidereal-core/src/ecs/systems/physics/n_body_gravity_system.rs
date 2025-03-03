use bevy::prelude::*;
use bevy_rapier2d::prelude::{RigidBody, Velocity};

pub fn n_body_gravity_system(
    mut query: Query<(Entity, &Transform, &mut Velocity, Option<&RigidBody>)>,
    time: Res<Time>,
) {
    // Collect all entities with mass (based on RigidBody type)
    let entities = query
        .iter()
        .filter_map(|(entity, transform, _, rigid_body)| {
            // Estimate mass based on rigid body type - greatly increased for better simulation
            match rigid_body {
                Some(RigidBody::Dynamic) => Some((entity, transform.translation, 50.0)),
                Some(RigidBody::Fixed) => Some((entity, transform.translation, 5000.0)),
                _ => None,
            }
        })
        .collect::<Vec<(Entity, Vec3, f32)>>();

    // Use a higher base gravitational constant for game scale
    // The real G is too small for game physics, so we multiply by a large factor
    let g = 10.0; // Significantly increased for game physics

    // Apply gravity between each pair of entities
    for (entity_a, pos_a, mass_a) in &entities {
        // Skip if we can't get the entity
        if let Ok((_, _, mut velocity, _)) = query.get_mut(*entity_a) {
            // Accumulate acceleration from all other bodies
            let mut total_acceleration = Vec2::ZERO;

            for (entity_b, pos_b, mass_b) in entities.iter() {
                // Skip self-interaction
                if entity_a == entity_b {
                    continue;
                }

                // Calculate direction vector
                let dir = *pos_b - *pos_a;
                let dir_2d = Vec2::new(dir.x, dir.y);
                let distance_squared = dir_2d.length_squared().max(1.0); // Prevent division by zero

                // Calculate gravitational force using F = G * (m1 * m2) / r²
                let force_magnitude = g * mass_a * mass_b / distance_squared;

                // Convert to acceleration: a = F/m
                let acceleration = force_magnitude / mass_a;

                // Add to total acceleration, ensuring we have a normalized direction
                let direction = dir_2d.normalize_or_zero();
                total_acceleration += direction * acceleration;
            }

            // Print debug info when acceleration is significant
            if total_acceleration.length() > 1.0 {
                //println!("Entity {:?} acceleration: {:?}", entity_a, total_acceleration);
            }

            // Apply the accumulated acceleration to velocity
            // Multiply by delta time to make the effect frame-rate independent
            velocity.linvel += total_acceleration * time.delta_secs();
        }
    }
}
