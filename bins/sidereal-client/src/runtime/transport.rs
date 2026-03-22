//! Lightyear client transport: spawn, connect, ensure channels.

use bevy::log::info;
#[cfg(target_arch = "wasm32")]
use bevy::log::warn;
use bevy::prelude::*;
#[cfg(not(target_arch = "wasm32"))]
use lightyear::prelude::UdpIo;
#[cfg(target_arch = "wasm32")]
use lightyear::prelude::client::WebTransportClientIo;
use lightyear::prelude::client::{
    Client, Connect, Connected, InputDelayConfig, InputTimelineConfig, RawClient,
};
#[cfg(not(target_arch = "wasm32"))]
use lightyear::prelude::{
    ChannelRegistry, MessageManager, PeerAddr, ReplicationReceiver, Transport,
};
#[cfg(target_arch = "wasm32")]
use lightyear::prelude::{
    ChannelRegistry, MessageManager, PeerAddr, ReplicationReceiver, Transport,
};
use lightyear::prelude::{LocalAddr, SyncConfig};
use sidereal_net::{
    ControlChannel, InputChannel, ManifestChannel, TacticalDeltaChannel, TacticalSnapshotChannel,
};
use std::net::SocketAddr;

use super::app_state::{ClientAppState, ClientSession};
use super::dialog_ui::DialogQueue;
use super::ecs_util::queue_despawn_if_exists;
use super::resources::{
    ClientInputTimelineTuning, LogoutCleanupRequested, PendingDisconnectNotify,
};

/// Spawns the Lightyear client and triggers Connect if no client entity exists.
/// Used on Enter Auth so we have a connection for sending auth after (re)login.
pub fn ensure_lightyear_client_system(
    mut commands: Commands<'_, '_>,
    session: Res<'_, ClientSession>,
    existing: Query<
        '_,
        '_,
        (
            Entity,
            Has<Connected>,
            Has<lightyear::prelude::client::Connecting>,
        ),
        With<RawClient>,
    >,
) {
    #[cfg(target_arch = "wasm32")]
    {
        if existing.is_empty() {
            start_lightyear_client_transport_inner(&mut commands, &session);
            return;
        }
        for (entity, connected, connecting) in &existing {
            if !connected && !connecting {
                queue_despawn_if_exists(&mut commands, entity);
                start_lightyear_client_transport_inner(&mut commands, &session);
                info!(
                    "wasm client lightyear WebTransport replacing stale client entity={:?}",
                    entity
                );
                return;
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        if existing.is_empty() {
            start_lightyear_client_transport_inner(&mut commands, &session);
            return;
        }
        for (entity, connected, connecting) in &existing {
            if !connected && !connecting {
                // Recreate transport entity instead of reconnecting in-place to avoid
                // stale transport/message state across repeated logout/login cycles.
                queue_despawn_if_exists(&mut commands, entity);
                start_lightyear_client_transport_inner(&mut commands, &session);
                info!(
                    "native client lightyear UDP replacing stale client entity={:?}",
                    entity
                );
                return;
            }
        }
    }
}

pub fn start_lightyear_client_transport(
    mut commands: Commands<'_, '_>,
    session: Res<'_, ClientSession>,
) {
    start_lightyear_client_transport_inner(&mut commands, &session);
}

#[cfg(not(target_arch = "wasm32"))]
fn resolved_udp_addr(session: &ClientSession) -> Result<SocketAddr, String> {
    if let Some(addr) = session.replication_transport.udp_addr.as_deref() {
        return addr
            .parse::<SocketAddr>()
            .map_err(|err| format!("invalid replication UDP addr from gateway: {err}"));
    }
    std::env::var("REPLICATION_UDP_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:7001".to_string())
        .parse::<SocketAddr>()
        .map_err(|err| format!("invalid REPLICATION_UDP_ADDR: {err}"))
}

#[cfg(target_arch = "wasm32")]
fn resolved_webtransport_config(session: &ClientSession) -> Result<(SocketAddr, String), String> {
    let remote_addr_text = session
        .replication_transport
        .webtransport_addr
        .clone()
        .or_else(|| std::env::var("REPLICATION_WEBTRANSPORT_ADDR").ok())
        .ok_or_else(|| "missing replication WebTransport address".to_string())?;
    let remote_addr = remote_addr_text
        .parse::<SocketAddr>()
        .map_err(|err| format!("invalid replication WebTransport addr: {err}"))?;
    let certificate_digest = session
        .replication_transport
        .webtransport_certificate_sha256
        .clone()
        .or_else(|| std::env::var("REPLICATION_WEBTRANSPORT_CERT_SHA256").ok())
        .ok_or_else(|| "missing replication WebTransport certificate digest".to_string())?
        .to_ascii_lowercase();
    Ok((remote_addr, certificate_digest))
}

pub fn start_lightyear_client_transport_inner(
    commands: &mut Commands<'_, '_>,
    session: &ClientSession,
) {
    #[cfg(target_arch = "wasm32")]
    {
        let (remote_addr, certificate_digest) = match resolved_webtransport_config(session) {
            Ok(value) => value,
            Err(err) => {
                warn!("wasm client WebTransport bootstrap unavailable: {}", err);
                return;
            }
        };
        let client = commands
            .spawn((
                Name::new("wasm-client-lightyear"),
                RawClient,
                WebTransportClientIo { certificate_digest },
                MessageManager::default(),
                ReplicationReceiver::default(),
                PeerAddr(remote_addr),
            ))
            .id();
        commands.trigger(Connect { entity: client });
        info!(
            "wasm client lightyear WebTransport connecting to {}",
            remote_addr
        );
        return;
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let local_addr = std::env::var("CLIENT_UDP_BIND")
            .unwrap_or_else(|_| "127.0.0.1:0".to_string())
            .parse::<SocketAddr>();
        let local_addr = match local_addr {
            Ok(v) => v,
            Err(err) => {
                tracing::error!("invalid CLIENT_UDP_BIND: {err}");
                return;
            }
        };
        let remote_addr = match resolved_udp_addr(session) {
            Ok(v) => v,
            Err(err) => {
                tracing::error!("{err}");
                return;
            }
        };

        let client = commands
            .spawn((
                Name::new("native-client-lightyear"),
                RawClient,
                UdpIo::default(),
                MessageManager::default(),
                ReplicationReceiver::default(),
                LocalAddr(local_addr),
                PeerAddr(remote_addr),
            ))
            .id();
        commands.trigger(Connect { entity: client });
        info!(
            "native client lightyear UDP connecting {} -> {}",
            local_addr, remote_addr
        );
    }
}

pub fn ensure_client_transport_channels(
    mut transports: Query<'_, '_, &mut Transport, With<Client>>,
    registry: Res<'_, ChannelRegistry>,
) {
    for mut transport in &mut transports {
        if !transport.has_sender::<ControlChannel>() {
            transport.add_sender_from_registry::<ControlChannel>(&registry);
        }
        if !transport.has_receiver::<ControlChannel>() {
            transport.add_receiver_from_registry::<ControlChannel>(&registry);
        }
        if !transport.has_sender::<InputChannel>() {
            transport.add_sender_from_registry::<InputChannel>(&registry);
        }
        if !transport.has_receiver::<InputChannel>() {
            transport.add_receiver_from_registry::<InputChannel>(&registry);
        }
        if !transport.has_sender::<TacticalSnapshotChannel>() {
            transport.add_sender_from_registry::<TacticalSnapshotChannel>(&registry);
        }
        if !transport.has_receiver::<TacticalSnapshotChannel>() {
            transport.add_receiver_from_registry::<TacticalSnapshotChannel>(&registry);
        }
        if !transport.has_sender::<TacticalDeltaChannel>() {
            transport.add_sender_from_registry::<TacticalDeltaChannel>(&registry);
        }
        if !transport.has_receiver::<TacticalDeltaChannel>() {
            transport.add_receiver_from_registry::<TacticalDeltaChannel>(&registry);
        }
        if !transport.has_sender::<ManifestChannel>() {
            transport.add_sender_from_registry::<ManifestChannel>(&registry);
        }
        if !transport.has_receiver::<ManifestChannel>() {
            transport.add_receiver_from_registry::<ManifestChannel>(&registry);
        }
    }
}

pub fn configure_client_input_timeline_on_add(
    trigger: On<Add, Client>,
    tuning: Res<'_, ClientInputTimelineTuning>,
    query: Query<'_, '_, Option<&'_ InputTimelineConfig>, With<Client>>,
    mut commands: Commands<'_, '_>,
) {
    let Ok(existing_config) = query.get(trigger.entity) else {
        return;
    };
    if existing_config.is_some() {
        return;
    }

    commands.entity(trigger.entity).insert(
        InputTimelineConfig::default()
            .with_sync_config(SyncConfig::default())
            .with_input_delay(InputDelayConfig::fixed_input_delay(
                tuning.fixed_input_delay_ticks,
            )),
    );
    info!(
        "configured client input timeline entity={} fixed_input_delay_ticks={}",
        trigger.entity, tuning.fixed_input_delay_ticks
    );
}

pub fn handle_unexpected_server_disconnect_system(
    mut removed_connected: RemovedComponents<'_, '_, Connected>,
    raw_clients: Query<'_, '_, Entity, With<RawClient>>,
    app_state: Option<Res<'_, State<ClientAppState>>>,
    pending_disconnect: Res<'_, PendingDisconnectNotify>,
    mut cleanup_requested: ResMut<'_, LogoutCleanupRequested>,
    mut dialog_queue: ResMut<'_, DialogQueue>,
) {
    // Ignore expected disconnects initiated by local logout flow.
    if pending_disconnect.0.is_some() || cleanup_requested.0 {
        let _: Vec<_> = removed_connected.read().collect();
        return;
    }

    // Only show server-disconnected UX when we were in active world flow.
    if !app_state.as_ref().is_some_and(|state| {
        matches!(
            state.get(),
            ClientAppState::InWorld
                | ClientAppState::WorldLoading
                | ClientAppState::AssetLoading
                | ClientAppState::CharacterSelect
        )
    }) {
        let _: Vec<_> = removed_connected.read().collect();
        return;
    }

    let live_raw_clients = raw_clients.iter().collect::<std::collections::HashSet<_>>();
    let disconnected = removed_connected
        .read()
        .any(|entity| live_raw_clients.contains(&entity));
    if !disconnected {
        return;
    }

    dialog_queue.push_error(
        "Server Disconnected",
        "The replication server connection was lost.\n\nYou have been returned to the login screen.",
    );
    cleanup_requested.0 = true;
}
