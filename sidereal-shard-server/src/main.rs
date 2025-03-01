use bevy::prelude::*;
use tracing::{info, Level};
use bevy::hierarchy::HierarchyPlugin;
use bevy::transform::TransformPlugin;
use bevy_state::app::StatesPlugin;
use std::env;

pub mod cluster;
pub mod physics;
pub mod replication;
pub mod shadow;
pub mod config;

use cluster::ClusterManagerPlugin;
use physics::ShardPhysicsPlugin;
use replication::ReplicationPlugin;
use shadow::ShadowEntityPlugin;
use config::ConfigPlugin;

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
    // Initialize tracing
    tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .init();
    
    info!("Starting Sidereal Shard Server");

    // Check for debug flag - collect args first to avoid lifetime issues
    let args: Vec<String> = env::args().collect();
    let debug_entities = args.contains(&"--debug-entities".to_string());
    
    // Initialize the Bevy app with minimal plugins for headless operation
    let mut app = App::new();
    
    // Add all plugins
    app.add_plugins(MinimalPlugins);
    app.add_plugins((
        HierarchyPlugin,
        TransformPlugin,
        StatesPlugin::default(),
        // Sidereal plugins
        ConfigPlugin,
        ClusterManagerPlugin,
        ShardPhysicsPlugin,
        ReplicationPlugin,
        ShadowEntityPlugin,
    ));
    
    // Initialize state
    app.init_state::<config::ShardState>();
    
    // Add debug systems if needed
    if debug_entities {
        info!("Entity debugging enabled");
        app.init_resource::<EntityCount>();
        app.add_systems(Update, debug_entity_details);
    }
    
    // Run the app
    app.run();
}

fn debug_entity_details(
    mut entity_count: ResMut<EntityCount>,
    time: Res<Time>,
    entities: Query<Entity>,
    shadows: Query<Entity, With<shadow::ShadowEntity>>,
    rapier_bodies: Query<Entity, With<bevy_rapier2d::prelude::RigidBody>>,
) {
    entity_count.last_printed += time.delta_secs();
    
    // Only print every 5 seconds to avoid log spam
    if entity_count.last_printed >= entity_count.print_interval {
        let total_count = entities.iter().count();
        let shadow_count = shadows.iter().count();
        let physics_count = rapier_bodies.iter().count();
        let other_count = total_count - shadow_count;
        
        info!("=== ENTITY DEBUG ===");
        info!("Total entities: {}", total_count);
        info!("Shadow entities: {}", shadow_count);
        info!("Physics entities: {}", physics_count);
        info!("Regular entities: {}", other_count);
        info!("====================");
        
        entity_count.last_printed = 0.0;
    }
}
