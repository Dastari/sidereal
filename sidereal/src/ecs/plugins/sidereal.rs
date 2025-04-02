use crate::ecs::components::id::Id;
use crate::ecs::components::*; // Assuming Object and Sector are here
use avian2d::prelude::*;
use bevy::prelude::*;
use bevy_replicon::prelude::*;
use bevy_replicon::shared::replication::replication_registry::ReplicationRegistry;  
use tracing::warn;

/// Main plugin for Sidereal type registration and replication
///
/// Usage options:
/// - Use with default settings for replication server and game clients
/// - Use with `with_replicon(false)` for shard servers
pub struct SiderealPlugin {
    /// Whether to use Replicon features
    replicon_enabled: bool,
}

impl Default for SiderealPlugin {
    fn default() -> Self {
        Self {
            replicon_enabled: true,
        }
    }
}

impl SiderealPlugin {
    /// Create a new SiderealPlugin with Replicon disabled
    pub fn without_replicon() -> Self {
        Self {
            replicon_enabled: false,
        }
    }

    /// Set whether to use Replicon
    pub fn with_replicon(mut self, enabled: bool) -> Self {
        self.replicon_enabled = enabled;
        self
    }
}

impl Plugin for SiderealPlugin {
    fn build(&self, app: &mut App) {
        // --- Replication Registration (only when replicon is enabled) ---
        if self.replicon_enabled {
            // Only try to add Replicon components if the app has RepliconPlugin
            let has_replicon = app.world().contains_resource::<ReplicationRegistry>();

            if has_replicon {
                // Register the components for replication
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
                if !app.world().contains_resource::<ReplicationRegistry>() {
                    app.init_resource::<ReplicationRegistry>();
                }
            } else {
                warn!(
                    "SiderealPlugin is configured to use Replicon, but RepliconPlugin has not been added. Skipping replication setup."
                );
            }
        }

        // --- Type Registration (for reflection, inspector, etc.) ---
        // Always enabled regardless of Replicon mode
        app.register_type::<Name>()
            .register_type::<Transform>()
            .register_type::<Id>()
            .register_type::<LinearVelocity>()
            .register_type::<AngularVelocity>()
            .register_type::<RigidBody>()
            .register_type::<Object>()
            .register_type::<Sector>()
            .register_type::<Position>()
            .register_type::<Rotation>();
    }
}
