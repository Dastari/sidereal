use sidereal_core::ecs::plugins::replication::server::RepliconRenetServerPlugin;
use bevy::prelude::*;
use tracing::{debug, error, info, warn};


/// Plugin for handling all replication tasks
pub struct ReplicationPlugin;

impl Plugin for ReplicationPlugin {
    fn build(&self, app: &mut App) {
        info!("Building replication plugin");

        app.add_plugins(RepliconRenetServerPlugin);
    }
}

