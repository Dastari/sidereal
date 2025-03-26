use crate::ecs::components::id::Id;
use crate::ecs::components::*; // Assuming Object and Sector are here
use avian2d::prelude::*;
use bevy::prelude::*;
use bevy_replicon::prelude::*;
pub struct SiderealPlugin;

impl Plugin for SiderealPlugin {
    fn build(&self, app: &mut App) {
        // --- Replication Registration ---
        app.replicate::<Name>()
            .replicate::<Transform>() // Keep this for now, might be redundant later
            .replicate::<Id>()
            .replicate::<LinearVelocity>()
            .replicate::<AngularVelocity>()
            .replicate::<RigidBody>()
            .replicate::<Object>()
            .replicate::<Sector>()
            .replicate::<Position>()
            .replicate::<Rotation>();

        // --- Type Registration (for reflection, inspector, etc.) ---
        app.register_type::<Name>()
            .register_type::<Transform>()
            .register_type::<Id>()
            .register_type::<LinearVelocity>()
            .register_type::<AngularVelocity>()
            .register_type::<RigidBody>()
            .register_type::<Object>()
            .register_type::<Sector>()
            // *** ADD THESE AVIAN COMPONENTS ***
            .register_type::<Position>()
            .register_type::<Rotation>();
    }
}
