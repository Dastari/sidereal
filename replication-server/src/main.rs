mod database;
mod game;
mod net;

use bevy::hierarchy::HierarchyPlugin;
use bevy::prelude::*;
use bevy_state::app::StatesPlugin;
use game::SceneLoaderPlugin;
use std::time::Duration;

use sidereal::ecs::plugins::SiderealPlugin;
use sidereal::net::config::{RepliconServerConfig, DEFAULT_PROTOCOL_ID, DEFAULT_REPLICON_PORT};

use net::renet2_server::Renet2ServerPlugin;
use net::replicon_server::RepliconServerPlugin;
use game::sector_manager::SectorManagerPlugin;

use tracing::{Level, debug, info};

fn main() {
    #[cfg(debug_assertions)]
    unsafe {
        std::env::set_var(
            "RUST_LOG",
            "info,bevy_app=info,bevy_ecs=info,renetcode2=info,renet2=info,bevy_replicon=warn,replication_server=debug",
        );
    }
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_max_level(Level::DEBUG)
        .init();

    info!("Starting Sidereal Replicon Server");

    let replicon_bind_addr = format!("0.0.0.0:{}", DEFAULT_REPLICON_PORT)
        .parse()
        .expect("Failed to parse default replicon server address");

    let replicon_config = RepliconServerConfig {
        bind_addr: replicon_bind_addr,
        protocol_id: DEFAULT_PROTOCOL_ID,
    };

    info!("Replicon server configuration: {:?}", replicon_config);

    App::new()
        .add_plugins(
            MinimalPlugins
                .set(bevy::app::ScheduleRunnerPlugin::run_loop(
                    Duration::from_secs_f64(1.0 / 20.0),
                ))
                .build(),
        )
        .add_plugins((HierarchyPlugin, TransformPlugin, StatesPlugin))
        .add_plugins((
            RepliconServerPlugin::with_config(replicon_config),
            Renet2ServerPlugin::default(),
            
        ))
        .add_plugins((
            SectorManagerPlugin,
            SiderealPlugin::default().with_replicon(true),
            SceneLoaderPlugin,
        ))
        .run();
}

