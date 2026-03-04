//! Lightyear client transport: spawn, connect, ensure channels.

use bevy::log::info;
use bevy::prelude::*;
use lightyear::prelude::client::{Client, Connect, Connected, RawClient};
use lightyear::prelude::{
    ChannelRegistry, LocalAddr, MessageManager, PeerAddr, ReplicationReceiver, Transport, UdpIo,
};
use sidereal_net::{AssetChannel, ControlChannel, InputChannel};
use std::net::SocketAddr;

use super::app_state::ClientAppState;
use super::dialog_ui::DialogQueue;
use super::resources::{LogoutCleanupRequested, PendingDisconnectNotify};

/// Spawns the Lightyear client and triggers Connect if no client entity exists.
/// Used on Enter Auth so we have a connection for sending auth after (re)login.
pub fn ensure_lightyear_client_system(
    mut commands: Commands<'_, '_>,
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
    if existing.is_empty() {
        start_lightyear_client_transport_inner(&mut commands);
        return;
    }
    for (entity, connected, connecting) in &existing {
        if !connected && !connecting {
            // Recreate transport entity instead of reconnecting in-place to avoid
            // stale transport/message state across repeated logout/login cycles.
            commands.entity(entity).try_despawn();
            start_lightyear_client_transport_inner(&mut commands);
            info!(
                "native client lightyear UDP replacing stale client entity={:?}",
                entity
            );
            return;
        }
    }
}

pub fn start_lightyear_client_transport(mut commands: Commands<'_, '_>) {
    start_lightyear_client_transport_inner(&mut commands);
}

pub fn start_lightyear_client_transport_inner(commands: &mut Commands<'_, '_>) {
    let local_addr = std::env::var("CLIENT_UDP_BIND")
        .unwrap_or_else(|_| "127.0.0.1:0".to_string())
        .parse::<SocketAddr>();
    let local_addr = match local_addr {
        Ok(v) => v,
        Err(err) => {
            eprintln!("invalid CLIENT_UDP_BIND: {err}");
            return;
        }
    };
    let remote_addr = std::env::var("REPLICATION_UDP_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:7001".to_string())
        .parse::<SocketAddr>();
    let remote_addr = match remote_addr {
        Ok(v) => v,
        Err(err) => {
            eprintln!("invalid REPLICATION_UDP_ADDR: {err}");
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
        if !transport.has_sender::<AssetChannel>() {
            transport.add_sender_from_registry::<AssetChannel>(&registry);
        }
        if !transport.has_receiver::<AssetChannel>() {
            transport.add_receiver_from_registry::<AssetChannel>(&registry);
        }
    }
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
