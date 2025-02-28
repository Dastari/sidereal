mod common;
mod movement;
mod rotation;
mod gravity;
mod collision;
mod collision_torque;

use bevy::prelude::*;

pub use common::*;
pub use movement::*;
pub use rotation::*;
pub use gravity::*;
pub use collision::*;
pub use collision_torque::*;

/// A plugin to set up the physics systems
pub struct PhysicsPlugin;

impl Plugin for PhysicsPlugin {
    fn build(&self, app: &mut App) {
        app
            // Register component reflection
            .add_systems(Startup, register_reflection)
            // Initialize gravity multiplier resource with default value
            .init_resource::<GravityMultiplier>()
            .add_systems(Update, (
                gravitational_well_system,
                collision_system,
                collision_torque_system,
                physics_system,
                rotation_system,
            ).chain());
    }
} 