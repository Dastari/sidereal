//! Lightyear client transport: spawn, connect, ensure channels.

use bevy::log::info;
#[cfg(target_arch = "wasm32")]
use bevy::log::warn;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use lightyear::interpolation::timeline::InterpolationConfig;
#[cfg(not(target_arch = "wasm32"))]
use lightyear::prelude::LocalAddr;
use lightyear::prelude::SyncConfig;
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
use sidereal_net::{
    ControlChannel, InputChannel, ManifestChannel, TacticalDeltaChannel, TacticalSnapshotChannel,
};
use std::net::SocketAddr;

use super::app_state::{ClientAppState, ClientSession};
use super::dialog_ui::DialogQueue;
use super::ecs_util::queue_despawn_if_exists;
use super::resources::{
    ClientInputTimelineTuning, ClientInterpolationTimelineTuning, ClientTimelineFocusState,
    LogoutCleanupRequested, PendingDisconnectNotify,
};
use std::time::Duration;

fn default_input_sync_config() -> SyncConfig {
    SyncConfig {
        jitter_multiple: 3,
        jitter_margin: Duration::from_millis(3),
        handshake_pings: 3,
        error_margin: 0.5,
        max_error_margin: 4.0,
        consecutive_errors: 0,
        previous_error_sign: true,
        consecutive_errors_threshold: 2,
        speedup_factor: 1.02,
    }
}

fn input_delay_config_from_tuning(
    tuning: ClientInputTimelineTuning,
    max_predicted_ticks: u16,
) -> InputDelayConfig {
    InputDelayConfig {
        minimum_input_delay_ticks: tuning.fixed_input_delay_ticks,
        maximum_input_delay_before_prediction: tuning.fixed_input_delay_ticks,
        maximum_predicted_ticks: max_predicted_ticks,
    }
}

fn input_timeline_config_from_tuning(
    tuning: ClientInputTimelineTuning,
    window_focused: bool,
) -> InputTimelineConfig {
    let max_predicted_ticks = if window_focused {
        tuning.max_predicted_ticks
    } else {
        tuning.unfocused_max_predicted_ticks
    };
    InputTimelineConfig::default()
        .with_sync_config(default_input_sync_config())
        .with_input_delay(input_delay_config_from_tuning(tuning, max_predicted_ticks))
}

fn interpolation_sync_config() -> SyncConfig {
    SyncConfig {
        jitter_multiple: 3,
        jitter_margin: Duration::from_millis(3),
        handshake_pings: 3,
        error_margin: 0.75,
        max_error_margin: 6.0,
        consecutive_errors: 0,
        previous_error_sign: true,
        consecutive_errors_threshold: 2,
        speedup_factor: 1.02,
    }
}

fn interpolation_config_from_tuning(
    tuning: ClientInterpolationTimelineTuning,
) -> InterpolationConfig {
    InterpolationConfig::default()
        .with_min_delay(Duration::from_millis(tuning.min_delay_ms))
        .with_send_interval_ratio(tuning.send_interval_ratio)
}

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

    commands
        .entity(trigger.entity)
        .insert(input_timeline_config_from_tuning(*tuning, true));
    info!(
        "configured client input timeline entity={} fixed_input_delay_ticks={} max_predicted_ticks={} unfocused_max_predicted_ticks={}",
        trigger.entity,
        tuning.fixed_input_delay_ticks,
        tuning.max_predicted_ticks,
        tuning.unfocused_max_predicted_ticks
    );
}

pub fn configure_client_interpolation_timeline_on_add(
    trigger: On<Add, Client>,
    tuning: Res<'_, ClientInterpolationTimelineTuning>,
    query: Query<'_, '_, Option<&'_ InterpolationConfig>, With<Client>>,
    mut commands: Commands<'_, '_>,
) {
    let Ok(existing_config) = query.get(trigger.entity) else {
        return;
    };
    if existing_config.is_some() {
        return;
    }

    let mut config = interpolation_config_from_tuning(*tuning);
    config.sync = interpolation_sync_config();
    commands.entity(trigger.entity).insert(config);
    info!(
        "configured client interpolation timeline entity={} min_delay_ms={} send_interval_ratio={}",
        trigger.entity, tuning.min_delay_ms, tuning.send_interval_ratio
    );
}

pub fn adapt_client_timeline_tuning_for_window_focus(
    tuning: Res<'_, ClientInputTimelineTuning>,
    mut focus_state: ResMut<'_, ClientTimelineFocusState>,
    windows: Query<'_, '_, &'_ Window, With<PrimaryWindow>>,
    clients: Query<'_, '_, Entity, With<Client>>,
    mut commands: Commands<'_, '_>,
) {
    let window_focused = windows
        .single()
        .map(|window| window.focused)
        .unwrap_or(true);
    if focus_state.last_window_focused == Some(window_focused) {
        return;
    }
    focus_state.last_window_focused = Some(window_focused);

    let max_predicted_ticks = if window_focused {
        tuning.max_predicted_ticks
    } else {
        tuning.unfocused_max_predicted_ticks
    };
    for entity in &clients {
        commands
            .entity(entity)
            .insert(input_timeline_config_from_tuning(*tuning, window_focused));
        info!(
            "reconfigured client input timeline entity={} window_focused={} fixed_input_delay_ticks={} max_predicted_ticks={}",
            entity, window_focused, tuning.fixed_input_delay_ticks, max_predicted_ticks
        );
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

#[cfg(test)]
mod tests {
    use super::{
        default_input_sync_config, input_delay_config_from_tuning, interpolation_config_from_tuning,
    };
    use crate::runtime::resources::{ClientInputTimelineTuning, ClientInterpolationTimelineTuning};
    use std::time::Duration;

    #[test]
    fn default_input_sync_config_has_tighter_resync_budget() {
        let config = default_input_sync_config();
        assert_eq!(config.error_margin, 0.5);
        assert_eq!(config.max_error_margin, 4.0);
        assert!(config.speedup_factor < 1.05);
    }

    #[test]
    fn unfocused_input_delay_config_disables_prediction_lead() {
        let tuning = ClientInputTimelineTuning {
            fixed_input_delay_ticks: 3,
            max_predicted_ticks: 24,
            unfocused_max_predicted_ticks: 0,
        };
        let config = input_delay_config_from_tuning(tuning, tuning.unfocused_max_predicted_ticks);
        assert_eq!(config.minimum_input_delay_ticks, 3);
        assert_eq!(config.maximum_predicted_ticks, 0);
    }

    #[test]
    fn interpolation_timeline_prefers_extra_delay_for_remote_smoothing() {
        let tuning = ClientInterpolationTimelineTuning {
            min_delay_ms: 50,
            send_interval_ratio: 2.0,
        };
        let config = interpolation_config_from_tuning(tuning);
        assert_eq!(config.min_delay, Duration::from_millis(50));
        assert_eq!(config.send_interval_ratio, 2.0);
    }

    #[test]
    fn input_delay_helper_keeps_fixed_floor() {
        let tuning = ClientInputTimelineTuning {
            fixed_input_delay_ticks: 4,
            max_predicted_ticks: 20,
            unfocused_max_predicted_ticks: 0,
        };
        let config = input_delay_config_from_tuning(tuning, 20);
        assert_eq!(config.minimum_input_delay_ticks, 4);
        assert_eq!(config.maximum_input_delay_before_prediction, 4);
        assert_eq!(config.maximum_predicted_ticks, 20);
    }
}
