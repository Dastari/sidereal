use bevy::log::info;
use bevy::prelude::*;
use bevy_remote::RemotePlugin;
use bevy_remote::http::RemoteHttpPlugin;
use lightyear::prelude::LocalAddr;
use lightyear::prelude::client::Connected;
use lightyear::prelude::server::{ClientOf, LinkOf, RawServer, ServerUdpIo, Start, Stopped};
use lightyear::prelude::{ReplicationSender, SendUpdatesMode};
use sidereal_core::remote_inspect::RemoteInspectConfig;
use sidereal_persistence::GraphPersistence;
use std::collections::HashMap;
use std::net::SocketAddr;

use crate::{
    BrpAuthToken, HydratedEntityCount, HydratedGraphEntity, PlayerRuntimeViewRegistry,
    ReplicationRuntime,
};

pub fn configure_remote(app: &mut App, cfg: &RemoteInspectConfig) {
    if !cfg.enabled {
        return;
    }

    app.add_plugins(RemotePlugin::default());
    app.add_plugins(
        RemoteHttpPlugin::default()
            .with_address(cfg.bind_addr)
            .with_port(cfg.port),
    );
    app.insert_resource(BrpAuthToken(
        cfg.auth_token.clone().expect("validated token"),
    ));
}

pub fn hydrate_replication_world(mut commands: Commands<'_, '_>) {
    let database_url = std::env::var("REPLICATION_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://sidereal:sidereal@127.0.0.1:5432/sidereal".to_string());

    let mut persistence = match GraphPersistence::connect(&database_url) {
        Ok(v) => v,
        Err(err) => {
            eprintln!("replication hydration skipped; connect failed: {err}");
            return;
        }
    };
    if let Err(err) = persistence.ensure_schema() {
        eprintln!("replication hydration skipped; schema ensure failed: {err}");
        return;
    }

    let records = match persistence.load_graph_records() {
        Ok(v) => v,
        Err(err) => {
            eprintln!("replication hydration skipped; graph load failed: {err}");
            return;
        }
    };

    for record in &records {
        commands.spawn(HydratedGraphEntity {
            entity_id: record.entity_id.clone(),
            labels: record.labels.clone(),
            component_count: record.components.len(),
        });
    }
    commands.insert_resource(HydratedEntityCount(records.len()));
    println!(
        "replication hydrated {} graph entities into Bevy world",
        records.len()
    );
}

pub fn init_replication_runtime(world: &mut World) {
    let database_url = std::env::var("REPLICATION_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://sidereal:sidereal@127.0.0.1:5432/sidereal".to_string());

    let mut persistence = match GraphPersistence::connect(&database_url) {
        Ok(v) => v,
        Err(err) => {
            eprintln!("replication runtime init failed to connect persistence: {err}");
            return;
        }
    };
    if let Err(err) = persistence.ensure_schema() {
        eprintln!("replication runtime init failed to ensure schema: {err}");
        return;
    }
    let loaded_view_states = match persistence.load_player_view_states() {
        Ok(states) => states,
        Err(err) => {
            eprintln!("replication runtime init failed loading player view state: {err}");
            Vec::new()
        }
    };

    world.insert_non_send_resource(ReplicationRuntime { persistence });
    let mut view_registry = world.resource_mut::<PlayerRuntimeViewRegistry>();
    view_registry.by_player_entity_id = loaded_view_states
        .into_iter()
        .map(|state| (state.player_entity_id.clone(), state))
        .collect::<HashMap<_, _>>();
}

pub fn start_lightyear_server(mut commands: Commands<'_, '_>) {
    let bind_addr = std::env::var("REPLICATION_UDP_BIND")
        .unwrap_or_else(|_| "0.0.0.0:7001".to_string())
        .parse::<SocketAddr>();
    let bind_addr = match bind_addr {
        Ok(v) => v,
        Err(err) => {
            eprintln!("invalid REPLICATION_UDP_BIND: {err}");
            return;
        }
    };

    let server = commands
        .spawn((
            Name::new("replication-lightyear-server"),
            RawServer,
            ServerUdpIo::default(),
            LocalAddr(bind_addr),
            Stopped,
        ))
        .id();
    commands.trigger(Start { entity: server });
    info!("replication lightyear UDP server starting on {}", bind_addr);
}

pub fn log_replication_client_connected(
    trigger: On<Add, Connected>,
    clients: Query<'_, '_, (), With<ClientOf>>,
) {
    if clients.get(trigger.entity).is_ok() {
        info!(
            "replication lightyear client connected entity={:?}",
            trigger.entity
        );
    }
}

/// Attaches `ReplicationSender` to each new client link entity so Lightyear
/// can replicate entity state and process visibility for this client.
pub fn setup_client_replication_sender(trigger: On<Add, LinkOf>, mut commands: Commands<'_, '_>) {
    let send_interval = std::time::Duration::from_millis(33); // ~30 Hz, matching server tick
    commands
        .entity(trigger.entity)
        .insert(ReplicationSender::new(
            send_interval,
            SendUpdatesMode::SinceLastAck,
            false,
        ));
    info!(
        "replication attached ReplicationSender to client link entity={:?}",
        trigger.entity
    );
}
