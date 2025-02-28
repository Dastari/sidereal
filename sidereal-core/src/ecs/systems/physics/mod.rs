mod common;
// Original custom physics modules are still here for reference
// They can be removed once the transition is complete
mod n_body_gravity_system;

use bevy::prelude::*;
use bevy_rapier2d::prelude::*;
// Removing the redundant import
// use n_body_gravity_system::*;
// Import the EntityBundle for testing - commented out since it's only used in the dev feature
// use crate::ecs::entities::entity::EntityBundle;

pub use common::*;
// We'll keep the original physics systems exports for now

pub use n_body_gravity_system::*;

/// A plugin to set up the physics systems with bevy_rapier2d
pub struct PhysicsPlugin;

impl Plugin for PhysicsPlugin {
    fn build(&self, app: &mut App) {
        // Activate Rapier physics 
        app.add_plugins(RapierPhysicsPlugin::<NoUserData>::default())
           .add_plugins(RapierDebugRenderPlugin::default())
           .add_systems(FixedUpdate, n_body_gravity_system);
        
        // Add test system - only use during development
        #[cfg(feature = "dev")]
        app.add_systems(Startup, setup_physics_test);
    }
}


/// Test system for spawning physics entities during development
#[cfg(feature = "dev")]
fn setup_physics_test(mut commands: Commands) {
    // Create a central star
    commands.spawn(EntityBundle::star(
        Vec2::new(0.0, 0.0),
        50.0,  // radius
        1000.0, // mass
    ))
    .insert(Name::new("Central Star"));
    
    // Create some planets
    commands.spawn(EntityBundle::planet(
        Vec2::new(200.0, 0.0),
        10.0,   // radius
        10.0,   // mass
    ))
    .insert(Name::new("Planet 1"))
    .insert(Velocity {
        linvel: Vec2::new(0.0, 20.0), // Initial velocity for orbit
        angvel: 0.0,
    });
    
    commands.spawn(EntityBundle::planet(
        Vec2::new(0.0, 300.0),
        15.0,   // radius
        20.0,   // mass
    ))
    .insert(Name::new("Planet 2"))
    .insert(Velocity {
        linvel: Vec2::new(-15.0, 0.0), // Initial velocity for orbit
        angvel: 0.0,
    });
    
    // Spawn some smaller objects with different velocities
    for i in 0..5 {
        let angle = (i as f32) * std::f32::consts::PI * 0.4;
        let distance = 100.0 + (i as f32) * 30.0;
        let position = Vec2::new(angle.cos() * distance, angle.sin() * distance);
        
        // Calculate velocity for a rough orbit (perpendicular to position vector)
        let orbit_speed = (1000.0 / distance).sqrt() * 4.0; // Based on mass of star
        let velocity = Vec2::new(-position.y, position.x).normalize() * orbit_speed;
        
        commands.spawn(EntityBundle::new(
            position,
            5.0,     // radius
            1.0,     // mass
            false,   // not fixed
            Some(velocity),
        ))
        .insert(Name::new(format!("Object {}", i+1)));
    }
}

/// Helper functions for working with Rapier components

// Create a Rapier RigidBody based on mass and whether it's fixed
pub fn create_rigid_body(mass: f32, is_fixed: bool) -> RigidBody {
    if is_fixed {
        RigidBody::Fixed
    } else if mass <= 0.0 {
        RigidBody::KinematicVelocityBased
    } else {
        RigidBody::Dynamic
    }
}

// Create a circular Rapier Collider
pub fn create_collider(radius: f32) -> Collider {
    Collider::ball(radius)
} 