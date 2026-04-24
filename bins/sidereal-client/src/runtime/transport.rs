//! Lightyear client transport: spawn, connect, ensure channels.

use bevy::log::{info, warn};
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
    ControlChannel, InputChannel, ManifestChannel, NotificationChannel, TacticalDeltaChannel,
    TacticalSnapshotChannel,
};
use std::net::{IpAddr, SocketAddr, ToSocketAddrs};

use super::app_state::{ClientAppState, ClientSession};
use super::dialog_ui::DialogQueue;
use super::ecs_util::queue_despawn_if_exists;
use super::resources::{
    ClientInputTimelineTuning, ClientInterpolationTimelineTuning, ClientTimelineFocusState,
    ControlBootstrapState, LogoutCleanupRequested, NativePredictionRecoveryPhase,
    NativePredictionRecoveryState, NativePredictionRecoveryTuning, PendingDisconnectNotify,
    PredictionCorrectionTuning, PredictionRecoveryReason,
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
    let (addr_text, source) = session
        .replication_transport
        .udp_addr
        .as_deref()
        .map(|addr| (addr.to_string(), "gateway"))
        .unwrap_or_else(|| {
            (
                std::env::var("REPLICATION_UDP_ADDR")
                    .unwrap_or_else(|_| "127.0.0.1:7001".to_string()),
                "REPLICATION_UDP_ADDR",
            )
        });
    let resolved = resolve_socket_addr(&addr_text)
        .map_err(|err| format!("invalid replication UDP addr from {source}: {err}"))?;

    if let Some(remote_gateway_addr) =
        rewrite_loopback_udp_addr_for_remote_gateway(resolved, &session.gateway_url)?
    {
        warn!(
            "replication UDP addr from {} resolved to loopback {}; gateway_url={} is remote, using {} instead",
            source, resolved, session.gateway_url, remote_gateway_addr
        );
        return Ok(remote_gateway_addr);
    }

    Ok(resolved)
}

#[cfg(not(target_arch = "wasm32"))]
fn resolved_local_udp_bind(remote_addr: SocketAddr) -> Result<SocketAddr, String> {
    resolved_local_udp_bind_from_config(remote_addr, std::env::var("CLIENT_UDP_BIND").ok())
}

#[cfg(not(target_arch = "wasm32"))]
fn resolved_local_udp_bind_from_config(
    remote_addr: SocketAddr,
    configured: Option<String>,
) -> Result<SocketAddr, String> {
    if let Some(configured) = configured {
        return configured
            .parse::<SocketAddr>()
            .map_err(|err| format!("invalid CLIENT_UDP_BIND: {err}"));
    }
    if remote_addr.ip().is_loopback() {
        return Ok(SocketAddr::from(([127, 0, 0, 1], 0)));
    }

    let bind_addr = SocketAddr::from(([0, 0, 0, 0], 0));
    warn!(
        "native client UDP target {} is remote; using default local bind {} instead of loopback",
        remote_addr, bind_addr
    );
    Ok(bind_addr)
}

#[cfg(not(target_arch = "wasm32"))]
fn resolve_socket_addr(addr: &str) -> Result<SocketAddr, String> {
    if let Ok(parsed) = addr.parse::<SocketAddr>() {
        return Ok(parsed);
    }
    addr.to_socket_addrs()
        .map_err(|err| err.to_string())?
        .next()
        .ok_or_else(|| "DNS lookup returned no socket addresses".to_string())
}

#[cfg(not(target_arch = "wasm32"))]
fn rewrite_loopback_udp_addr_for_remote_gateway(
    udp_addr: SocketAddr,
    gateway_url: &str,
) -> Result<Option<SocketAddr>, String> {
    if !udp_addr.ip().is_loopback() {
        return Ok(None);
    }

    let gateway = reqwest::Url::parse(gateway_url)
        .map_err(|err| format!("invalid gateway URL for UDP fallback: {err}"))?;
    let Some(host) = gateway.host_str() else {
        return Ok(None);
    };
    if gateway_host_is_loopback(host) {
        return Ok(None);
    }

    let target = format!("{}:{}", bracket_ipv6_host(host), udp_addr.port());
    resolve_socket_addr(&target).map(Some)
}

#[cfg(not(target_arch = "wasm32"))]
fn gateway_host_is_loopback(host: &str) -> bool {
    host.eq_ignore_ascii_case("localhost")
        || host.parse::<IpAddr>().is_ok_and(|addr| addr.is_loopback())
}

#[cfg(not(target_arch = "wasm32"))]
fn bracket_ipv6_host(host: &str) -> String {
    if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]")
    } else {
        host.to_string()
    }
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
        let remote_addr = match resolved_udp_addr(session) {
            Ok(v) => v,
            Err(err) => {
                tracing::error!("{err}");
                return;
            }
        };
        let local_addr = match resolved_local_udp_bind(remote_addr) {
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
        if !transport.has_sender::<NotificationChannel>() {
            transport.add_sender_from_registry::<NotificationChannel>(&registry);
        }
        if !transport.has_receiver::<NotificationChannel>() {
            transport.add_receiver_from_registry::<NotificationChannel>(&registry);
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

pub fn update_native_prediction_recovery_for_window_focus(
    tuning: Res<'_, NativePredictionRecoveryTuning>,
    input_tuning: Res<'_, ClientInputTimelineTuning>,
    prediction_tuning: Res<'_, PredictionCorrectionTuning>,
    control_bootstrap_state: Res<'_, ControlBootstrapState>,
    windows: Query<'_, '_, &'_ Window, With<PrimaryWindow>>,
    time: Res<'_, Time>,
    mut recovery_state: ResMut<'_, NativePredictionRecoveryState>,
) {
    let now_s = time.elapsed_secs_f64();
    recovery_state.complete_recovery_if_elapsed(now_s);

    let window_focused = windows
        .single()
        .map(|window| window.focused)
        .unwrap_or(true);
    if recovery_state.last_window_focused == Some(window_focused) {
        return;
    }

    let previous_phase = recovery_state.phase;
    recovery_state.last_window_focused = Some(window_focused);
    recovery_state.transition_count = recovery_state.transition_count.saturating_add(1);
    recovery_state.pending_neutral_send = true;

    if window_focused {
        let unfocused_duration_s = match previous_phase {
            NativePredictionRecoveryPhase::Unfocused { started_at_s } => {
                (now_s - started_at_s).max(0.0)
            }
            _ => 0.0,
        };
        recovery_state.last_unfocused_duration_s = unfocused_duration_s;
        if unfocused_duration_s >= tuning.min_unfocused_s {
            recovery_state.phase = NativePredictionRecoveryPhase::Recovering {
                regain_at_s: now_s,
                suppress_input_until_s: now_s + tuning.suppress_input_s,
                reason: PredictionRecoveryReason::FocusStall,
            };
        } else {
            recovery_state.phase = NativePredictionRecoveryPhase::Focused;
        }
        info!(
            "native prediction focus regained unfocused_s={:.3} recovery_phase={} control_phase={:?} focused_max_predicted_ticks={} unfocused_max_predicted_ticks={} rollback_budget_ticks={} suppress_input_s={:.3} resync_after_s={:.3} max_tick_gap={}",
            unfocused_duration_s,
            recovery_state.phase.label(now_s),
            control_bootstrap_state.phase,
            input_tuning.max_predicted_ticks,
            input_tuning.unfocused_max_predicted_ticks,
            prediction_tuning.max_rollback_ticks,
            tuning.suppress_input_s,
            tuning.resync_after_s,
            tuning.max_tick_gap,
        );
    } else {
        recovery_state.phase = NativePredictionRecoveryPhase::Unfocused {
            started_at_s: now_s,
        };
        info!(
            "native prediction focus lost control_phase={:?} focused_max_predicted_ticks={} unfocused_max_predicted_ticks={} rollback_budget_ticks={} resync_after_s={:.3} max_tick_gap={}",
            control_bootstrap_state.phase,
            input_tuning.max_predicted_ticks,
            input_tuning.unfocused_max_predicted_ticks,
            prediction_tuning.max_rollback_ticks,
            tuning.resync_after_s,
            tuning.max_tick_gap,
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
        default_input_sync_config, input_delay_config_from_tuning,
        interpolation_config_from_tuning, resolved_local_udp_bind_from_config, resolved_udp_addr,
    };
    use crate::runtime::app_state::ClientSession;
    use crate::runtime::resources::{ClientInputTimelineTuning, ClientInterpolationTimelineTuning};
    use sidereal_core::gateway_dtos::ReplicationTransportConfig;
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

    #[test]
    fn native_udp_addr_rewrites_loopback_gateway_advertisement_for_remote_gateway() {
        let session = ClientSession {
            gateway_url: "http://192.168.50.25:8080".to_string(),
            replication_transport: ReplicationTransportConfig {
                udp_addr: Some("127.0.0.1:7001".to_string()),
                ..Default::default()
            },
            ..Default::default()
        };

        let addr = resolved_udp_addr(&session).expect("udp addr");

        assert_eq!(addr.to_string(), "192.168.50.25:7001");
    }

    #[test]
    fn native_udp_addr_keeps_loopback_for_local_gateway() {
        let session = ClientSession {
            gateway_url: "http://127.0.0.1:8080".to_string(),
            replication_transport: ReplicationTransportConfig {
                udp_addr: Some("127.0.0.1:7001".to_string()),
                ..Default::default()
            },
            ..Default::default()
        };

        let addr = resolved_udp_addr(&session).expect("udp addr");

        assert_eq!(addr.to_string(), "127.0.0.1:7001");
    }

    #[test]
    fn native_local_udp_default_uses_wildcard_for_remote_target() {
        let remote = "192.168.50.25:7001".parse().expect("remote addr");

        let bind = resolved_local_udp_bind_from_config(remote, None).expect("bind addr");

        assert_eq!(bind.to_string(), "0.0.0.0:0");
    }

    #[test]
    fn native_local_udp_default_keeps_loopback_for_local_target() {
        let remote = "127.0.0.1:7001".parse().expect("remote addr");

        let bind = resolved_local_udp_bind_from_config(remote, None).expect("bind addr");

        assert_eq!(bind.to_string(), "127.0.0.1:0");
    }
}
