mod database;
mod game;
mod scene;

use bevy::hierarchy::HierarchyPlugin;
use bevy::prelude::*;
use bevy_remote::http::RemoteHttpPlugin;
use bevy_remote::RemotePlugin;
use bevy_state::app::StatesPlugin;
// use scene::SceneLoaderPlugin;
use avian2d::prelude::*;
use game::process_message_queue;
use sidereal_core::ecs::components::*;
use sidereal_core::ecs::entities::ship::Ship;
use sidereal_core::ecs::plugins::network::NetworkServerPlugin;
use sidereal_core::ecs::plugins::serialization::EntitySerializationPlugin;
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
            RemotePlugin::default(),
            RemoteHttpPlugin::default(),
            EntitySerializationPlugin,
            NetworkServerPlugin,
        ))
        .add_systems(Startup, setup_game_world)
        .add_systems(Update, process_message_queue)
        .run();
}

fn setup_game_world(mut commands: Commands) {
    // Spawn entities with NetworkId and Networked components
    commands.spawn((
        Ship::new(),
        Transform::from_translation(Vec3::new(0.0, 0.0, 0.0)),
        Name::new("Ship"),
        Hull {
            width: 50.0,
            height: 30.0,
            blocks: vec![
                Block {
                    x: 0.0,
                    y: 0.0,
                    direction: Direction::Fore,
                },
                Block {
                    x: 10.0,
                    y: 0.0,
                    direction: Direction::Starboard,
                },
            ],
        },
        // Avian physics components
        RigidBody::Dynamic,
        Collider::circle(25.0),
        LinearVelocity(Vec2::new(100.0, 0.0)),
    ));

    // More entities...
}
