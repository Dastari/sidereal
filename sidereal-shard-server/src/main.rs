mod game;

use bevy::hierarchy::HierarchyPlugin;
use bevy::log::*;
use bevy::prelude::*;
use bevy::transform::TransformPlugin;
use bevy_remote::http::RemoteHttpPlugin;
use bevy_remote::RemotePlugin;
use bevy_state::app::StatesPlugin;
use game::process_message_queue;
use game::sector_assignemnt::*;
use sidereal_core::ecs::plugins::network::client::NetworkClientPlugin;
use sidereal_core::ecs::plugins::serialization::EntitySerializationPlugin;

use avian2d::prelude::*;
use tracing::{info, Level};

fn main() {
    // Initialize tracing
    tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .init();

    info!("Starting Sidereal Shard Server");

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
            TransformPlugin,
            bevy::asset::AssetPlugin::default(),
            bevy::scene::ScenePlugin,
        ))
        .init_resource::<Assets<Mesh>>()
        .add_plugins((
            HierarchyPlugin,
            RemotePlugin::default(),
            RemoteHttpPlugin::default(),
            StatesPlugin::default(),
            PhysicsPlugins::default(),
        ))
        .add_plugins(setup_shard_server)
        .run();
}

pub fn setup_shard_server(app: &mut App) {
    app.add_plugins((
        EntitySerializationPlugin,
        SectorAssignmentPlugin,
        NetworkClientPlugin,
    ));

    // Add shard manager systems
    app.add_systems(Update, process_message_queue);
}
