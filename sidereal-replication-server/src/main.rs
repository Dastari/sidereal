mod database;
mod game;
mod scene;

use bevy::prelude::*;
use bevy::{hierarchy::HierarchyPlugin, time::common_conditions::on_timer};
use bevy_remote::http::RemoteHttpPlugin;
use bevy_remote::RemotePlugin;
use bevy_state::app::StatesPlugin;

// use scene::SceneLoaderPlugin;
use avian2d::prelude::*;

use game::{
    process_message_queue,
    shard_manager::{balance_sectors, check_assignment_timeouts, handle_shard_connection},
    ShardManager,
};
use sidereal_core::ecs::components::*;
use sidereal_core::ecs::entities::ship::Ship;
use sidereal_core::ecs::plugins::network::NetworkServerPlugin;
use sidereal_core::ecs::plugins::sectors::SectorPlugin;
use sidereal_core::ecs::plugins::serialization::EntitySerializationPlugin;

use tracing::{info, Level};

use fake::faker::company::en::*;
use fake::faker::name::en::*;
use fake::Fake;
use rand::Rng;
use rayon::prelude::*;

// Configuration constant for mock ship generation
const MOCK_SHIPS: usize = 10;
const WORLD_BOUNDS: f32 = 10_000.0;

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
        ))
        .add_plugins(setup_game)
        .run();
}

pub fn setup_game(app: &mut App) {
    app.init_resource::<ShardManager>();
    app.add_plugins((EntitySerializationPlugin, NetworkServerPlugin, SectorPlugin));

    app.add_systems(Startup, setup_game_world);
    // Add shard manager systems
    app.add_systems(
        Update,
        (
            process_message_queue,
            handle_shard_connection,
            check_assignment_timeouts.run_if(on_timer(std::time::Duration::from_secs(5))),
            balance_sectors.run_if(on_timer(std::time::Duration::from_secs(30))),
        ),
    );
}

fn setup_game_world(mut commands: Commands) {
    info!("Generating {} mock ships...", MOCK_SHIPS);
    let start_time = std::time::Instant::now();

    // Pre-generate all ship data in parallel
    let ships: Vec<_> = (0..MOCK_SHIPS)
        .into_par_iter()
        .map(|_| {
            let mut rng = rand::rng();

            // Generate random position within world bounds
            let position = Vec3::new(
                rng.random_range(-WORLD_BOUNDS..WORLD_BOUNDS),
                rng.random_range(-WORLD_BOUNDS..WORLD_BOUNDS),
                0.0,
            );

            // Generate random velocity
            let velocity = Vec2::new(rng.random_range(-100.0..100.0), rng.random_range(-100.0..100.0));

            // Generate random dimensions for the hull
            let hull_width = rng.random_range(30.0..100.0);
            let hull_height = rng.random_range(20.0..60.0);

            // Generate a random ship name using faker
            let ship_name = if rng.random_bool(0.5) {
                // Company name as ship name
                CompanyName().fake::<String>()
            } else {
                // Person's last name as ship name
                LastName().fake::<String>()
            };

            // Add a fancy prefix
            let prefix = match rng.random_range(0..7) {
                0 => "USS",
                1 => "ISS",
                2 => "SSV",
                3 => "RSV",
                4 => "ESS",
                5 => "CSS",
                _ => "TSS",
            };

            let full_name = format!("{} {}", prefix, ship_name);

            // Create random hull blocks
            let num_blocks = rng.gen_range(2..5);
            let mut blocks = Vec::with_capacity(num_blocks);

            for _ in 0..num_blocks {
                blocks.push(Block {
                    x: rng.random_range(-hull_width / 2.0..hull_width / 2.0),
                    y: rng.random_range(-hull_height / 2.0..hull_height / 2.0),
                    direction: match rng.random_range(0..4) {
                        0 => Direction::Fore,
                        1 => Direction::Aft,
                        2 => Direction::Port,
                        _ => Direction::Starboard,
                    },
                });
            }

            (
                position,
                velocity,
                hull_width,
                hull_height,
                full_name,
                blocks,
            )
        })
        .collect();

    // Spawn all ships with the pre-generated data
    for (position, velocity, hull_width, hull_height, name, blocks) in ships {
        commands.spawn((
            Ship::new(),
            Transform::from_translation(position),
            Name::new(name),
            Hull {
                width: hull_width,
                height: hull_height,
                blocks,
            },
            // Avian physics components
            RigidBody::Dynamic,
            Collider::circle(hull_width.min(hull_height) / 2.0),
            LinearVelocity(velocity),
        ));
    }

    let duration = start_time.elapsed();
    info!("Generated {} ships in {:.2?}", MOCK_SHIPS, duration);
}
