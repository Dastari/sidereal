use crate::ecs::components::*;
use bevy_replicon::prelude::*;
use bevy_replicon::shared::replication::replication_registry::ReplicationRegistry;

use avian2d::prelude::*;
use bevy::prelude::*;
use tracing::warn;

pub struct SiderealPlugin {
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
    pub fn without_replicon() -> Self {
        Self {
            replicon_enabled: false,
        }
    }

    pub fn with_replicon(mut self, enabled: bool) -> Self {
        self.replicon_enabled = enabled;
        self
    }
}

impl Plugin for SiderealPlugin {
    fn build(&self, app: &mut App) {
        if self.replicon_enabled {
            let has_replicon = app.world().contains_resource::<ReplicationRegistry>();

            if has_replicon {
                app.replicate::<Name>()
                    .replicate::<Transform>()
                    .replicate::<Id>()
                    .replicate::<LinearVelocity>()
                    .replicate::<AngularVelocity>()
                    .replicate::<RigidBody>()
                    .replicate::<Object>()
                    .replicate::<Sector>()
                    .replicate::<Position>()
                    .replicate::<Rotation>();

                if !app.world().contains_resource::<ReplicationRegistry>() {
                    app.init_resource::<ReplicationRegistry>();
                }
            } else {
                warn!(
                    "SiderealPlugin is configured to use Replicon, but RepliconPlugin has not been added. Skipping replication setup."
                );
            }
        }

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
