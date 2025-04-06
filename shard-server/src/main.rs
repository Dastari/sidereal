mod game;
mod net;

use avian2d::prelude::*;
use bevy::asset::AssetPlugin;
use bevy::hierarchy::HierarchyPlugin;
use bevy::prelude::*;
use bevy::scene::ScenePlugin;
use bevy::transform::TransformPlugin;
use bevy_remote::RemotePlugin;
use bevy_remote::http::RemoteHttpPlugin;
use bevy_state::app::StatesPlugin;
use game::shard_manager::ShardManagerPlugin;
use net::renet2_client::{Renet2ClientConfig, Renet2ClientPlugin};
use sidereal::ecs::plugins::SiderealPlugin;
use sidereal::net::config::{DEFAULT_PROTOCOL_ID, DEFAULT_RENET2_PORT};
use std::time::Duration;
use uuid::Uuid;

use tracing::{Level, info};

fn main() {
    #[cfg(debug_assertions)]
    unsafe {
        std::env::set_var(
            "RUST_LOG",
            "info,bevy_app=info,bevy_ecs=info,renetcode2=info,renet2=info,sidereal=debug,shard_server=debug",
        );
    }

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_max_level(Level::DEBUG)
        .init();

    let client_config = Renet2ClientConfig {
        shard_id: Uuid::new_v4(),
        protocol_id: DEFAULT_PROTOCOL_ID,
        server_addr: format!("127.0.0.1:{}", DEFAULT_RENET2_PORT)
            .parse()
            .expect("Failed to parse replication server address"),
        bind_addr: "127.0.0.1:0".parse().expect("Failed to parse bind address"),
    };

    info!("Initial shard configuration: {:?}", client_config);

    App::new()
        .add_plugins(
            MinimalPlugins
                .set(bevy::app::ScheduleRunnerPlugin::run_loop(
                    Duration::from_secs_f64(1.0 / 30.0),
                ))
                .build(),
        )
        .add_plugins((
            TransformPlugin,
            AssetPlugin::default(),
            ScenePlugin,
            HierarchyPlugin,
            PhysicsPlugins::default(),
            StatesPlugin::default(),
            RemotePlugin::default(),
            RemoteHttpPlugin::default()
                .with_header("Access-Control-Allow-Origin", "http://localhost:3000")
                .with_header(
                    "Access-Control-Allow-Headers",
                    "content-type, authorization",
                )
                .with_header(
                    "Access-Control-Allow-Methods",
                    "GET, POST, PUT, DELETE, OPTIONS",
                ),
        ))
        .add_plugins(Renet2ClientPlugin::with_config(client_config))
        .add_plugins((SiderealPlugin::without_replicon(), ShardManagerPlugin))
        .run();
}
