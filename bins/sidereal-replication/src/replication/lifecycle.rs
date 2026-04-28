use bevy::log::{error, info};
use bevy::prelude::*;
use bevy_remote::RemotePlugin;
use bevy_remote::http::RemoteHttpPlugin;
use lightyear::prelude::client::Connected;
use lightyear::prelude::server::{
    ClientOf, LinkOf, RawServer, ServerUdpIo, Start, Stopped, WebTransportServerIo,
};
use lightyear::prelude::{
    ChannelRegistry, MessageReceiver, MessageSender, Replicate, ReplicationGroup,
    ReplicationSender, SendUpdatesMode, Transport, Unlink,
};
use lightyear::prelude::{Identity, LocalAddr};
use sidereal_core::SIM_TICK_HZ;
use sidereal_core::remote_inspect::RemoteInspectConfig;
use sidereal_net::{
    ClientAuthMessage, ClientControlRequestMessage, ClientDisconnectNotifyMessage,
    ClientLocalViewModeMessage, ClientNotificationDismissedMessage, ClientRealtimeInputMessage,
    ClientTacticalResnapshotRequestMessage, ServerControlAckMessage, ServerControlRejectMessage,
    ServerNotificationMessage, ServerSessionDeniedMessage, ServerSessionReadyMessage,
};
use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;

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

    if let Some((bind_addr, certificate)) = webtransport_server_config_from_env() {
        let server = commands
            .spawn((
                Name::new("replication-lightyear-webtransport-server"),
                RawServer,
                WebTransportServerIo { certificate },
                LocalAddr(bind_addr),
                Stopped,
            ))
            .id();
        commands.trigger(Start { entity: server });
        info!(
            "replication lightyear WebTransport server starting on {}",
            bind_addr
        );
    }
}

fn webtransport_server_config_from_env() -> Option<(SocketAddr, Identity)> {
    let bind_addr = std::env::var("REPLICATION_WEBTRANSPORT_BIND")
        .ok()?
        .parse::<SocketAddr>()
        .map_err(|err| error!("invalid REPLICATION_WEBTRANSPORT_BIND: {err}"))
        .ok()?;

    let certificate = match (
        std::env::var("REPLICATION_WEBTRANSPORT_CERT_PEM"),
        std::env::var("REPLICATION_WEBTRANSPORT_KEY_PEM"),
    ) {
        (Ok(cert_pem), Ok(key_pem)) => {
            let runtime = tokio::runtime::Runtime::new()
                .map_err(|err| {
                    error!("failed creating tokio runtime for WebTransport cert load: {err}")
                })
                .ok()?;
            match runtime.block_on(Identity::load_pemfiles(cert_pem, key_pem)) {
                Ok(identity) => identity,
                Err(err) => {
                    error!("failed loading WebTransport certificate PEM files: {err}");
                    return None;
                }
            }
        }
        _ => {
            let sans = vec![
                "localhost".to_string(),
                "127.0.0.1".to_string(),
                "::1".to_string(),
            ];
            match Identity::self_signed(sans) {
                Ok(identity) => identity,
                Err(err) => {
                    error!("failed generating self-signed WebTransport certificate: {err}");
                    return None;
                }
            }
        }
    };

    let digest = certificate.certificate_chain().as_slice()[0].hash();
    info!("replication WebTransport certificate digest {}", digest);
    Some((bind_addr, certificate))
}

#[allow(clippy::type_complexity)]
pub fn log_replication_client_connected(
    trigger: On<Add, Connected>,
    clients: Query<
        '_,
        '_,
        (
            Option<&'_ LinkOf>,
            Option<&'_ Transport>,
            Has<MessageReceiver<ClientAuthMessage>>,
            Has<MessageSender<ServerSessionReadyMessage>>,
            Has<MessageSender<ServerNotificationMessage>>,
        ),
        With<ClientOf>,
    >,
) {
    if let Ok((
        link_of,
        transport,
        has_auth_receiver,
        has_session_ready_sender,
        has_notification_sender,
    )) = clients.get(trigger.entity)
    {
        let has_control_receiver = transport
            .is_some_and(|transport| transport.has_receiver::<sidereal_net::ControlChannel>());
        let has_control_sender = transport
            .is_some_and(|transport| transport.has_sender::<sidereal_net::ControlChannel>());
        info!(
            "replication lightyear client connected entity={:?} server={:?} has_auth_receiver={} has_session_ready_sender={} has_notification_sender={} has_control_receiver={} has_control_sender={}",
            trigger.entity,
            link_of.map(|link| link.server),
            has_auth_receiver,
            has_session_ready_sender,
            has_notification_sender,
            has_control_receiver,
            has_control_sender
        );
    }
}

/// Attaches `ReplicationSender` to each new client link entity so Lightyear
/// can replicate entity state and process visibility for this client.
pub fn setup_client_replication_sender(trigger: On<Add, LinkOf>, mut commands: Commands<'_, '_>) {
    let send_interval = std::time::Duration::from_secs_f64(1.0 / f64::from(SIM_TICK_HZ));
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

pub fn prime_client_link_transport_on_insert(
    trigger: On<Insert, (Transport, ClientOf)>,
    mut commands: Commands<'_, '_>,
    mut transports: Query<'_, '_, &'_ mut Transport, With<ClientOf>>,
    registry: Res<'_, ChannelRegistry>,
) {
    let Ok(mut transport) = transports.get_mut(trigger.entity) else {
        return;
    };

    if !transport.has_receiver::<sidereal_net::ControlChannel>() {
        transport.add_receiver_from_registry::<sidereal_net::ControlChannel>(&registry);
    }
    if !transport.has_sender::<sidereal_net::ControlChannel>() {
        transport.add_sender_from_registry::<sidereal_net::ControlChannel>(&registry);
    }
    if !transport.has_receiver::<sidereal_net::InputChannel>() {
        transport.add_receiver_from_registry::<sidereal_net::InputChannel>(&registry);
    }
    if !transport.has_sender::<sidereal_net::InputChannel>() {
        transport.add_sender_from_registry::<sidereal_net::InputChannel>(&registry);
    }
    if !transport.has_sender::<sidereal_net::TacticalSnapshotChannel>() {
        transport.add_sender_from_registry::<sidereal_net::TacticalSnapshotChannel>(&registry);
    }
    if !transport.has_sender::<sidereal_net::TacticalDeltaChannel>() {
        transport.add_sender_from_registry::<sidereal_net::TacticalDeltaChannel>(&registry);
    }
    if !transport.has_sender::<sidereal_net::ManifestChannel>() {
        transport.add_sender_from_registry::<sidereal_net::ManifestChannel>(&registry);
    }
    if !transport.has_receiver::<sidereal_net::NotificationChannel>() {
        transport.add_receiver_from_registry::<sidereal_net::NotificationChannel>(&registry);
    }
    if !transport.has_sender::<sidereal_net::NotificationChannel>() {
        transport.add_sender_from_registry::<sidereal_net::NotificationChannel>(&registry);
    }

    commands.entity(trigger.entity).insert((
        MessageReceiver::<ClientAuthMessage>::default(),
        MessageReceiver::<ClientDisconnectNotifyMessage>::default(),
        MessageReceiver::<ClientControlRequestMessage>::default(),
        MessageReceiver::<ClientRealtimeInputMessage>::default(),
        MessageReceiver::<ClientLocalViewModeMessage>::default(),
        MessageReceiver::<ClientTacticalResnapshotRequestMessage>::default(),
        MessageReceiver::<ClientNotificationDismissedMessage>::default(),
        MessageSender::<ServerSessionReadyMessage>::default(),
        MessageSender::<ServerSessionDeniedMessage>::default(),
        MessageSender::<ServerControlAckMessage>::default(),
        MessageSender::<ServerControlRejectMessage>::default(),
        MessageSender::<ServerNotificationMessage>::default(),
    ));

    info!(
        "replication primed transport/message components for client link entity={:?}",
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
        if !transport.has_receiver::<sidereal_net::InputChannel>() {
            transport.add_receiver_from_registry::<sidereal_net::InputChannel>(&registry);
        }
        if !transport.has_sender::<sidereal_net::InputChannel>() {
            transport.add_sender_from_registry::<sidereal_net::InputChannel>(&registry);
        }
        if !transport.has_sender::<sidereal_net::TacticalSnapshotChannel>() {
            transport.add_sender_from_registry::<sidereal_net::TacticalSnapshotChannel>(&registry);
        }
        if !transport.has_sender::<sidereal_net::TacticalDeltaChannel>() {
            transport.add_sender_from_registry::<sidereal_net::TacticalDeltaChannel>(&registry);
        }
        if !transport.has_sender::<sidereal_net::ManifestChannel>() {
            transport.add_sender_from_registry::<sidereal_net::ManifestChannel>(&registry);
        }
        if !transport.has_receiver::<sidereal_net::NotificationChannel>() {
            transport.add_receiver_from_registry::<sidereal_net::NotificationChannel>(&registry);
        }
        if !transport.has_sender::<sidereal_net::NotificationChannel>() {
            transport.add_sender_from_registry::<sidereal_net::NotificationChannel>(&registry);
        }
    }
}

#[allow(clippy::type_complexity)]
pub fn ensure_server_message_components(
    mut commands: Commands<'_, '_>,
    clients: Query<
        '_,
        '_,
        (
            Entity,
            Has<MessageReceiver<ClientAuthMessage>>,
            Has<MessageReceiver<ClientDisconnectNotifyMessage>>,
            Has<MessageReceiver<ClientControlRequestMessage>>,
            Has<MessageReceiver<ClientRealtimeInputMessage>>,
            Has<MessageReceiver<ClientLocalViewModeMessage>>,
            Has<MessageReceiver<ClientTacticalResnapshotRequestMessage>>,
            Has<MessageReceiver<ClientNotificationDismissedMessage>>,
            Has<MessageSender<ServerSessionReadyMessage>>,
            Has<MessageSender<ServerSessionDeniedMessage>>,
            Has<MessageSender<ServerControlAckMessage>>,
            Has<MessageSender<ServerControlRejectMessage>>,
            Has<MessageSender<ServerNotificationMessage>>,
        ),
        With<ClientOf>,
    >,
) {
    for (
        client_entity,
        has_auth_recv,
        has_disconnect_recv,
        has_control_recv,
        has_input_recv,
        has_view_mode_recv,
        has_tactical_resnapshot_recv,
        has_notification_dismissed_recv,
        has_session_ready_send,
        has_session_denied_send,
        has_control_ack_send,
        has_control_reject_send,
        has_notification_send,
    ) in &clients
    {
        let mut patched = Vec::new();
        let mut entity_commands = commands.entity(client_entity);

        if !has_auth_recv {
            entity_commands.insert(MessageReceiver::<ClientAuthMessage>::default());
            patched.push("recv:ClientAuthMessage");
        }
        if !has_disconnect_recv {
            entity_commands.insert(MessageReceiver::<ClientDisconnectNotifyMessage>::default());
            patched.push("recv:ClientDisconnectNotifyMessage");
        }
        if !has_control_recv {
            entity_commands.insert(MessageReceiver::<ClientControlRequestMessage>::default());
            patched.push("recv:ClientControlRequestMessage");
        }
        if !has_input_recv {
            entity_commands.insert(MessageReceiver::<ClientRealtimeInputMessage>::default());
            patched.push("recv:ClientRealtimeInputMessage");
        }
        if !has_view_mode_recv {
            entity_commands.insert(MessageReceiver::<ClientLocalViewModeMessage>::default());
            patched.push("recv:ClientLocalViewModeMessage");
        }
        if !has_tactical_resnapshot_recv {
            entity_commands
                .insert(MessageReceiver::<ClientTacticalResnapshotRequestMessage>::default());
            patched.push("recv:ClientTacticalResnapshotRequestMessage");
        }
        if !has_notification_dismissed_recv {
            entity_commands
                .insert(MessageReceiver::<ClientNotificationDismissedMessage>::default());
            patched.push("recv:ClientNotificationDismissedMessage");
        }
        if !has_session_ready_send {
            entity_commands.insert(MessageSender::<ServerSessionReadyMessage>::default());
            patched.push("send:ServerSessionReadyMessage");
        }
        if !has_session_denied_send {
            entity_commands.insert(MessageSender::<ServerSessionDeniedMessage>::default());
            patched.push("send:ServerSessionDeniedMessage");
        }
        if !has_control_ack_send {
            entity_commands.insert(MessageSender::<ServerControlAckMessage>::default());
            patched.push("send:ServerControlAckMessage");
        }
        if !has_control_reject_send {
            entity_commands.insert(MessageSender::<ServerControlRejectMessage>::default());
            patched.push("send:ServerControlRejectMessage");
        }
        if !has_notification_send {
            entity_commands.insert(MessageSender::<ServerNotificationMessage>::default());
            patched.push("send:ServerNotificationMessage");
        }

        if !patched.is_empty() {
            info!(
                "replication patched missing message components for client link entity={:?}: {}",
                client_entity,
                patched.join(", ")
            );
        }
    }
}

/// Ensure replicated entities use per-entity replication groups by default.
///
/// Lightyear's default `ReplicationGroup(0)` is shared and can cause update starvation
/// patterns when many entities are active. We normalize untouched defaults to
/// `ReplicationGroup::new_from_entity()` so each entity advances independently.
pub fn ensure_entity_scoped_replication_groups(
    mut commands: Commands<'_, '_>,
    groups: Query<'_, '_, (Entity, &'_ ReplicationGroup), With<Replicate>>,
) {
    for (entity, group) in &groups {
        if *group == ReplicationGroup::default() {
            commands
                .entity(entity)
                .insert(ReplicationGroup::new_from_entity());
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
