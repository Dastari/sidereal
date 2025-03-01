use bevy::prelude::*;
use tracing::{info, Level};
use bevy::hierarchy::HierarchyPlugin;
use bevy::transform::TransformPlugin;
use bevy_state::app::StatesPlugin;
use scene::SceneLoaderPlugin;
use replication::ReplicationPlugin;
use universe::UniverseManagerPlugin;

mod database;
mod scene;
mod replication;
mod universe;

fn main() {
    // Initialize tracing
    tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .init();
    
    info!("Starting Sidereal Replication Server");
    
    // Initialize the Bevy app with minimal plugins for headless operation
    App::new()
        .add_plugins(MinimalPlugins)
        .add_plugins((
            HierarchyPlugin,
            TransformPlugin,
            StatesPlugin::default(),
            SceneLoaderPlugin,
            ReplicationPlugin,        // Uses the RepliconSetup from sidereal_core to listen on port 5000
            UniverseManagerPlugin,
        ))
        .run();
}
