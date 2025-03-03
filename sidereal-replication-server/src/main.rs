use bevy::hierarchy::HierarchyPlugin;
use bevy::prelude::*;
use bevy::transform::TransformPlugin;
use bevy_state::app::StatesPlugin;
use replication::ReplicationPlugin;
use scene::SceneLoaderPlugin;
use tracing::{info, Level};
use universe::UniverseManagerPlugin;

mod database;
mod replication;
mod scene;
mod universe;

fn main() {
    // Initialize tracing
    tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .init();

    info!("Starting Sidereal Replication Server");

    // Enable debug tracing for netcode to see raw packet details
    #[cfg(debug_assertions)]
    {
        use std::env;
        env::set_var("RUST_LOG", "info,renetcode2=trace,renet2=debug");

        // Initialize tracing if not already done
        if tracing_subscriber::fmt::Subscriber::builder()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .try_init()
            .is_ok()
        {
            println!("Enhanced logging enabled for netcode debugging");
        }
    }

    // Initialize the Bevy app with minimal plugins for headless operation
    App::new()
        .add_plugins(MinimalPlugins)
        .add_plugins((
            HierarchyPlugin,
            TransformPlugin,
            StatesPlugin::default(),
            SceneLoaderPlugin,
            ReplicationPlugin,
            UniverseManagerPlugin,
        ))
        .run();
}
