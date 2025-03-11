mod game;

use bevy::hierarchy::HierarchyPlugin;
use bevy::prelude::*;
use bevy::log::*;
use bevy::transform::TransformPlugin;
use bevy_state::app::StatesPlugin;
use sidereal_core::ecs::plugins::network::client::NetworkClientPlugin;
use sidereal_core::ecs::plugins::serialization::EntitySerializationPlugin;
use game::process_message_queue;

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
            StatesPlugin::default(),
            PhysicsPlugins::default(),
            EntitySerializationPlugin,
            NetworkClientPlugin,
        ))
        .add_systems(Update, process_message_queue)
        .run();
}
