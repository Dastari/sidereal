use bevy::log::{error, info, warn};
use bevy::prelude::*;
use bevy_remote::RemotePlugin;
use bevy_remote::http::RemoteHttpPlugin;
use lightyear::prelude::LocalAddr;
use lightyear::prelude::client::Connected;
use lightyear::prelude::server::{ClientOf, LinkOf, RawServer, ServerUdpIo, Start, Stopped};
use lightyear::prelude::{ChannelRegistry, ReplicationSender, SendUpdatesMode, Transport, Unlink};
use sidereal_core::remote_inspect::RemoteInspectConfig;
use sidereal_persistence::GraphPersistence;
use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;

use crate::replication::persistence::PersistenceSchemaInitState;

#[derive(Debug, Resource, Clone)]
pub(crate) struct BrpAuthToken {
    pub(crate) _token: String,
}

#[derive(Debug, Resource, Clone, Copy)]
pub(crate) struct HydratedEntityCount {
    pub(crate) _count: usize,
}

#[derive(Debug, Component)]
pub(crate) struct HydratedGraphEntity {
    pub(crate) _entity_id: String,
    pub(crate) _labels: Vec<String>,
    pub(crate) _component_count: usize,
}

/// Tracks last time we received any message from each client (by client entity).
/// Used to disconnect idle clients so the server stops sending to dead sockets.
#[derive(Resource, Default)]
pub(crate) struct ClientLastActivity(pub(crate) HashMap<Entity, f64>);

/// Client entities we have already triggered Unlink for (idle timeout). Cleared when the
/// entity is no longer in the clients query, so we only trigger once per client.
#[derive(Resource, Default)]
pub(crate) struct PendingIdleUnlink(pub(crate) HashSet<Entity>);

/// Idle disconnect timeout (seconds), read once at startup from REPLICATION_IDLE_DISCONNECT_SECONDS.
#[derive(Resource, Clone, Copy)]
pub(crate) struct IdleDisconnectSeconds(pub(crate) f64);

/// Default idle time (seconds) after which we disconnect a client we have not heard from.
/// With raw UDP, the server never learns the client closed; it keeps sending. We disconnect
/// so we stop sending to a dead socket and free resources.
pub(crate) const DEFAULT_IDLE_DISCONNECT_SECONDS: f64 = 15.0;

pub fn init_resources(app: &mut App) {
    app.insert_resource(ClientLastActivity::default());
    app.insert_resource(PendingIdleUnlink::default());
    let idle_disconnect_seconds = std::env::var("REPLICATION_IDLE_DISCONNECT_SECONDS")
        .ok()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(DEFAULT_IDLE_DISCONNECT_SECONDS);
    app.insert_resource(IdleDisconnectSeconds(idle_disconnect_seconds));
}

pub fn configure_remote(app: &mut App, cfg: &RemoteInspectConfig) {
    if !cfg.enabled {
        return;
    }

    app.add_plugins(RemotePlugin::default());
    let remote_http = RemoteHttpPlugin::default()
        .with_address(cfg.bind_addr)
        .with_port(cfg.port);
    app.add_plugins(remote_http);
    app.insert_resource(BrpAuthToken {
        _token: cfg.auth_token.clone().expect("validated token"),
    });
}

pub fn hydrate_replication_world(
    mut commands: Commands<'_, '_>,
    mut schema_init_state: ResMut<'_, PersistenceSchemaInitState>,
) {
    let database_url = std::env::var("REPLICATION_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://sidereal:sidereal@127.0.0.1:5432/sidereal".to_string());

    let mut persistence = match GraphPersistence::connect(&database_url) {
        Ok(v) => v,
        Err(err) => {
            warn!("replication hydration skipped; connect failed: {err}");
            return;
        }
    };
    if let Err(err) = persistence.ensure_schema() {
        warn!("replication hydration skipped; schema init failed: {err}");
        return;
    }
    schema_init_state.0 = true;
    let records = match persistence.load_graph_records() {
        Ok(v) => v,
        Err(err) => {
            warn!("replication hydration skipped; graph load failed: {err}");
            return;
        }
    };

    for record in &records {
        commands.spawn(HydratedGraphEntity {
            _entity_id: record.entity_id.clone(),
            _labels: record.labels.clone(),
            _component_count: record.components.len(),
        });
    }
    commands.insert_resource(HydratedEntityCount {
        _count: records.len(),
    });
    info!(
        "replication hydrated {} graph entities into Bevy world",
        records.len()
    );
}

pub fn start_lightyear_server(mut commands: Commands<'_, '_>) {
    let bind_addr = std::env::var("REPLICATION_UDP_BIND")
        .unwrap_or_else(|_| "0.0.0.0:7001".to_string())
        .parse::<SocketAddr>();
    let bind_addr = match bind_addr {
        Ok(v) => v,
        Err(err) => {
            error!("invalid REPLICATION_UDP_BIND: {err}");
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

pub fn ensure_server_transport_channels(
    mut transports: Query<'_, '_, &'_ mut Transport, With<ClientOf>>,
    registry: Res<'_, ChannelRegistry>,
) {
    for mut transport in &mut transports {
        if !transport.has_receiver::<sidereal_net::ControlChannel>() {
            transport.add_receiver_from_registry::<sidereal_net::ControlChannel>(&registry);
        }
        if !transport.has_sender::<sidereal_net::ControlChannel>() {
            transport.add_sender_from_registry::<sidereal_net::ControlChannel>(&registry);
        }
    }
}

pub fn disconnect_idle_clients(
    time: Res<Time<Real>>,
    idle_config: Res<IdleDisconnectSeconds>,
    mut last_activity: ResMut<ClientLastActivity>,
    mut pending_unlink: ResMut<PendingIdleUnlink>,
    clients: Query<'_, '_, Entity, With<ClientOf>>,
    mut commands: Commands<'_, '_>,
) {
    pending_unlink.0.retain(|e| clients.get(*e).is_ok());

    let now_s = time.elapsed_secs_f64();
    let timeout_s = idle_config.0;
    for client_entity in &clients {
        if pending_unlink.0.contains(&client_entity) {
            continue;
        }
        let last = *last_activity.0.entry(client_entity).or_insert(now_s);
        if now_s - last > timeout_s {
            info!(
                "replication disconnecting idle client entity={:?} (no activity for {:.0}s)",
                client_entity,
                now_s - last
            );
            pending_unlink.0.insert(client_entity);
            commands.trigger(Unlink {
                entity: client_entity,
                reason: "idle_timeout".to_string(),
            });
        }
    }
}
