use bevy::hierarchy::HierarchyPlugin;
use bevy::prelude::*;
use bevy::transform::TransformPlugin;
use bevy_state::app::StatesPlugin;
use std::env;
use tracing::info;

pub mod cluster;
pub mod config;
pub mod physics;
pub mod replication;
pub mod shadow;

use cluster::ClusterManagerPlugin;
use config::ConfigPlugin;
use physics::ShardPhysicsPlugin;
use replication::ReplicationPlugin;
use shadow::ShadowEntityPlugin;

#[derive(Resource)]
struct EntityCount {
    last_printed: f32,
    print_interval: f32,
}

impl Default for EntityCount {
    fn default() -> Self {
        Self {
            last_printed: 0.0,
            print_interval: 5.0,
        }
    }
}

fn main() {
    // Enable detailed networking logs - match test client configuration exactly
    std::env::set_var("RUST_LOG", "info,renetcode2=debug,renet2=debug");

    // Initialize tracing if not already done
    tracing_subscriber::fmt::Subscriber::builder()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init(); // Init returns () so we can't call .ok() on it

    info!("Starting Sidereal Shard Server");

    // Initialize the Bevy app with minimal plugins for headless operation
    let mut app = App::new();

    // Add all plugins - ensure the same core plugins as test client first
    app.add_plugins(MinimalPlugins);

    // Add core Bevy plugins needed for headless operation
    app.add_plugins((HierarchyPlugin, TransformPlugin, StatesPlugin::default()));

    // Initialize state before other plugins
    app.init_state::<config::ShardState>();

    // Add Sidereal plugins in order of dependency
    app.add_plugins((
        ConfigPlugin, // Load config first since other plugins need it
        ClusterManagerPlugin,
        ShardPhysicsPlugin,
        ReplicationPlugin, // This will add the RepliconClientPlugin internally
        ShadowEntityPlugin,
    ));
    // Run the app
    app.run();
}
