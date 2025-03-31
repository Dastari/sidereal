use crate::ecs::components::id::Id;
use crate::ecs::components::*; // Assuming Object and Sector are here
use avian2d::prelude::*;
use bevy::prelude::*;
#[cfg(feature = "replicon")]
use bevy_replicon::prelude::*;

/// Main plugin for Sidereal type registration and replication
/// 
/// Features:
/// - "replicon": Enables Replicon integration for entity replication (for game clients and replication server)
/// - Without any features: Only registers types (for shard servers)
pub struct SiderealPlugin;

impl Plugin for SiderealPlugin {
    fn build(&self, app: &mut App) {
        // --- Replication Registration (only when replicon feature is enabled) ---
        #[cfg(feature = "replicon")]
        {
            // Access to the "replicate" extension method is only available when "replicon" feature is enabled
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
            
            // Make sure the ReplicationRegistry resource exists
            // (this avoids panics in other parts of the codebase that might expect it)
            if !app.world().contains_resource::<bevy_replicon::shared::replication::replication_registry::ReplicationRegistry>() {
                app.init_resource::<bevy_replicon::shared::replication::replication_registry::ReplicationRegistry>();
            }
        }

        // --- Type Registration (for reflection, inspector, etc.) ---
        // Always enabled regardless of features
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
