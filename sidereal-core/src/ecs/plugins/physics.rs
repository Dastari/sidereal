use bevy::prelude::*;
use bevy_rapier2d::prelude::*;
use crate::ecs::systems::physics::n_body_gravity_system;
pub struct PhysicsPlugin;

impl Plugin for PhysicsPlugin {
    fn build(&self, app: &mut App) {
        // Activate Rapier physics 
        app.add_plugins(RapierPhysicsPlugin::<NoUserData>::default())
           .add_plugins(RapierDebugRenderPlugin::default())
           .add_systems(FixedUpdate, n_body_gravity_system);
    }
}
