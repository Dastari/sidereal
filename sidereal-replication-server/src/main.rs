mod database;
mod game;

use bevy::hierarchy::HierarchyPlugin;
use bevy::prelude::*;
use bevy_state::app::StatesPlugin;

use game::{process_message_queue, ShardManagerPlugin, SceneLoaderPlugin};

use sidereal_core::ecs::plugins::{NetworkServerPlugin, SectorPlugin, EntitySerializationPlugin };
use sidereal_core::ecs::systems::mock_game_world;
use tracing::{info, Level};

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
            
            // RemotePlugin::default(),
            // RemoteHttpPlugin::default(),
        ))
        .add_plugins(setup_replication_server)
        .run();
}

pub fn setup_replication_server(app: &mut App) {
    app.add_plugins((
        EntitySerializationPlugin,
        NetworkServerPlugin,
        SectorPlugin,
        ShardManagerPlugin,
        SceneLoaderPlugin,
    ));

    // app.add_systems(Startup, mock_game_world);
    // Add shard manager systems
    app.add_systems(Update, process_message_queue);
    
}
