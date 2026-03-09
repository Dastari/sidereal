//! Control request/response: send control requests, receive acks/rejects, log state.

use bevy::log::{info, warn};
use bevy::prelude::*;
use lightyear::prelude::client::{Client, Connected};
use lightyear::prelude::{MessageReceiver, MessageSender};
use sidereal_net::{
    ClientControlRequestMessage, ClientLocalViewMode, ClientLocalViewModeMessage, ControlChannel,
    PlayerEntityId, ServerControlAckMessage, ServerControlRejectMessage,
};
use sidereal_runtime_sync::parse_guid_from_entity_id;
use std::sync::OnceLock;

use super::app_state::{
    ClientAppState, ClientSession, LocalPlayerViewState, is_active_world_state,
};
use super::components::{ControlledEntity, GameplayCamera, TopDownCamera, WorldEntity};
use super::platform::safe_viewport_size;
use super::resources::ClientViewModeState;
use super::resources::{ClientControlDebugState, ClientControlRequestState, HeadlessTransportMode};

pub fn client_control_debug_logging_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("SIDEREAL_DEBUG_CONTROL_LOGS")
            .is_ok_and(|v| v == "1" || v.eq_ignore_ascii_case("true"))
    })
}

fn ids_refer_to_same_guid(left: &str, right: &str) -> bool {
    if left == right {
        return true;
    }
    parse_guid_from_entity_id(left)
        .zip(parse_guid_from_entity_id(right))
        .is_some_and(|(l, r)| l == r)
}

fn control_target_log_label(value: Option<&str>) -> &str {
    value.unwrap_or("<none>")
}

#[allow(clippy::too_many_arguments)]
pub fn send_lightyear_control_requests(
    app_state: Option<Res<'_, State<ClientAppState>>>,
    headless_mode: Res<'_, HeadlessTransportMode>,
    time: Res<'_, Time>,
    session: Res<'_, ClientSession>,
    mut request_state: ResMut<'_, ClientControlRequestState>,
    mut senders: Query<
        '_,
        '_,
        &mut MessageSender<ClientControlRequestMessage>,
        (With<Client>, With<Connected>),
    >,
    player_view_state: Res<'_, LocalPlayerViewState>,
) {
    let active_world_state = is_active_world_state(&app_state, &headless_mode);
    if !active_world_state {
        return;
    }
    let Some(player_entity_id) = session.player_entity_id.as_ref() else {
        return;
    };
    let Some(canonical_player_entity_id) =
        PlayerEntityId::parse(player_entity_id.as_str()).map(PlayerEntityId::canonical_wire_id)
    else {
        return;
    };
    if senders.is_empty() {
        return;
    }

    if request_state.pending_request_seq.is_none() {
        let desired = player_view_state
            .desired_controlled_entity_id
            .clone()
            .or_else(|| player_view_state.controlled_entity_id.clone());
        // Sidereal persists the last authoritative control target on the player entity. The
        // client must wait for that replicated state instead of speculatively asking the server
        // to switch back to the player anchor during bootstrap. Otherwise a fresh login appears
        // to "forget" the last controlled ship and introduces avoidable control/prediction churn
        // before the authoritative target has even replicated in.
        let Some(desired) = desired else {
            return;
        };
        let control_changed = player_view_state
            .controlled_entity_id
            .as_deref()
            .is_none_or(|current| !ids_refer_to_same_guid(desired.as_str(), current));
        if control_changed {
            request_state.next_request_seq = request_state.next_request_seq.saturating_add(1);
            request_state.pending_controlled_entity_id = Some(desired);
            request_state.pending_request_seq = Some(request_state.next_request_seq);
            request_state.last_sent_request_seq = None;
            request_state.last_sent_at_s = 0.0;
        }
    }

    let Some(request_seq) = request_state.pending_request_seq else {
        return;
    };
    let now_s = time.elapsed_secs_f64();
    let resend_interval_s = 0.5;
    if request_state.last_sent_request_seq == Some(request_seq)
        && now_s - request_state.last_sent_at_s < resend_interval_s
    {
        return;
    }
    let requested_controlled_entity_id = request_state.pending_controlled_entity_id.clone();
    let message = ClientControlRequestMessage {
        player_entity_id: canonical_player_entity_id,
        controlled_entity_id: requested_controlled_entity_id,
        request_seq,
    };

    for mut sender in &mut senders {
        sender.send::<ControlChannel>(message.clone());
    }
    if client_control_debug_logging_enabled() {
        info!(
            "client control handover request player={} seq={} previous_controlled={} desired_controlled={} request_payload={}",
            player_entity_id,
            request_seq,
            control_target_log_label(player_view_state.controlled_entity_id.as_deref()),
            control_target_log_label(player_view_state.desired_controlled_entity_id.as_deref()),
            control_target_log_label(message.controlled_entity_id.as_deref()),
        );
    }
    request_state.last_sent_at_s = now_s;
    request_state.last_sent_request_seq = Some(request_seq);
}

#[allow(clippy::type_complexity)]
#[allow(clippy::too_many_arguments)]
pub fn send_local_view_mode_updates(
    app_state: Option<Res<'_, State<ClientAppState>>>,
    headless_mode: Res<'_, HeadlessTransportMode>,
    time: Res<'_, Time>,
    session: Res<'_, ClientSession>,
    mut state: ResMut<'_, ClientViewModeState>,
    camera: Query<'_, '_, &'_ TopDownCamera>,
    gameplay_camera_projection: Query<'_, '_, &'_ Projection, With<GameplayCamera>>,
    windows: Query<'_, '_, &'_ Window, With<bevy::window::PrimaryWindow>>,
    mut senders: Query<
        '_,
        '_,
        &mut MessageSender<ClientLocalViewModeMessage>,
        (With<Client>, With<Connected>),
    >,
) {
    if !is_active_world_state(&app_state, &headless_mode) || senders.is_empty() {
        return;
    }
    let Some(player_entity_id) = session.player_entity_id.as_ref() else {
        return;
    };
    let Some(canonical_player_entity_id) =
        PlayerEntityId::parse(player_entity_id.as_str()).map(PlayerEntityId::canonical_wire_id)
    else {
        return;
    };

    // Current camera ranges are tactical-only; map mode engages once strategic zoom range
    // is implemented (distance threshold intentionally above current max_distance).
    const MAP_MODE_DISTANCE_THRESHOLD_M: f32 = 120.0;
    let current_mode = camera
        .single()
        .ok()
        .map(|camera| {
            if camera.distance >= MAP_MODE_DISTANCE_THRESHOLD_M {
                ClientLocalViewMode::Map
            } else {
                ClientLocalViewMode::Tactical
            }
        })
        .unwrap_or(ClientLocalViewMode::Tactical);
    // Derive delivery radius from visible world half-diagonal + buffer.
    // This keeps culling aligned with what the player can actually see.
    const DELIVERY_RADIUS_BUFFER_M: f32 = 120.0;
    const DELIVERY_RADIUS_MIN_M: f32 = 300.0;
    const DELIVERY_RADIUS_MAX_M: f32 = 5000.0;
    let dynamic_delivery_range_m = gameplay_camera_projection
        .single()
        .ok()
        .and_then(|projection| match projection {
            Projection::Orthographic(ortho) => Some(ortho.scale),
            _ => None,
        })
        .zip(windows.single().ok().and_then(safe_viewport_size))
        .map(|(ortho_scale, viewport_size)| {
            let half_extents = viewport_size * 0.5 * ortho_scale.max(0.0001);
            (half_extents.length() + DELIVERY_RADIUS_BUFFER_M)
                .clamp(DELIVERY_RADIUS_MIN_M, DELIVERY_RADIUS_MAX_M)
        })
        .unwrap_or(DELIVERY_RADIUS_MIN_M);
    let now_s = time.elapsed_secs_f64();
    let mode_changed = state.last_sent_mode != Some(current_mode);
    let range_changed = state
        .last_sent_delivery_range_m
        .is_none_or(|last| (last - dynamic_delivery_range_m).abs() >= 5.0);
    let heartbeat_due = now_s - state.last_sent_at_s >= 1.0;
    if !mode_changed && !range_changed && !heartbeat_due {
        return;
    }

    let message = ClientLocalViewModeMessage {
        player_entity_id: canonical_player_entity_id,
        view_mode: current_mode,
        delivery_range_m: dynamic_delivery_range_m,
    };
    for mut sender in &mut senders {
        sender.send::<ControlChannel>(message.clone());
    }
    state.last_sent_mode = Some(current_mode);
    state.last_sent_delivery_range_m = Some(dynamic_delivery_range_m);
    state.last_sent_at_s = now_s;
}

pub fn receive_lightyear_control_results(
    time: Res<'_, Time>,
    session: Res<'_, ClientSession>,
    mut player_view_state: ResMut<'_, LocalPlayerViewState>,
    mut request_state: ResMut<'_, ClientControlRequestState>,
    mut debug_state: ResMut<'_, ClientControlDebugState>,
    mut ack_receivers: Query<
        '_,
        '_,
        &mut MessageReceiver<ServerControlAckMessage>,
        (With<Client>, With<Connected>),
    >,
    mut reject_receivers: Query<
        '_,
        '_,
        &mut MessageReceiver<ServerControlRejectMessage>,
        (With<Client>, With<Connected>),
    >,
) {
    let Some(local_player_entity_id) = session.player_entity_id.as_ref() else {
        return;
    };
    let Some(local_player_id) = PlayerEntityId::parse(local_player_entity_id.as_str()) else {
        return;
    };

    for mut receiver in &mut ack_receivers {
        for message in receiver.receive() {
            let Some(message_player_id) = PlayerEntityId::parse(message.player_entity_id.as_str())
            else {
                continue;
            };
            if message_player_id != local_player_id {
                continue;
            }
            let pending_requested = request_state.pending_controlled_entity_id.clone();
            if request_state.pending_request_seq == Some(message.request_seq) {
                request_state.pending_controlled_entity_id = None;
                request_state.pending_request_seq = None;
                request_state.last_sent_request_seq = None;
            }
            let previous_controlled = player_view_state.controlled_entity_id.clone();
            if let Some(controlled_entity_id) = message.controlled_entity_id {
                player_view_state.controlled_entity_id = Some(controlled_entity_id);
            } else {
                player_view_state.controlled_entity_id = session.player_entity_id.clone();
            }
            player_view_state.desired_controlled_entity_id =
                player_view_state.controlled_entity_id.clone();
            if client_control_debug_logging_enabled() {
                info!(
                    "client control handover ack player={} seq={} previous_controlled={} authoritative_controlled={} pending_requested={} result=ack",
                    message.player_entity_id,
                    message.request_seq,
                    control_target_log_label(previous_controlled.as_deref()),
                    control_target_log_label(player_view_state.controlled_entity_id.as_deref()),
                    control_target_log_label(pending_requested.as_deref()),
                );
            }
            debug_state.handover_audit_entity_id = player_view_state.controlled_entity_id.clone();
            debug_state.handover_audit_started_at_s = Some(time.elapsed_secs_f64());
            debug_state.last_handover_audit_log_at_s = 0.0;
        }
    }

    for mut receiver in &mut reject_receivers {
        for message in receiver.receive() {
            let Some(message_player_id) = PlayerEntityId::parse(message.player_entity_id.as_str())
            else {
                continue;
            };
            if message_player_id != local_player_id {
                continue;
            }
            let pending_requested = request_state.pending_controlled_entity_id.clone();
            let pending_request_seq = request_state.pending_request_seq;
            if request_state.pending_request_seq == Some(message.request_seq) {
                request_state.pending_controlled_entity_id = None;
                request_state.pending_request_seq = None;
                request_state.last_sent_request_seq = None;
            }
            let previous_controlled = player_view_state.controlled_entity_id.clone();
            if let Some(authoritative) = message.authoritative_controlled_entity_id {
                player_view_state.controlled_entity_id = Some(authoritative);
            } else if player_view_state.controlled_entity_id.is_none() {
                player_view_state.controlled_entity_id = session.player_entity_id.clone();
            }
            player_view_state.desired_controlled_entity_id =
                player_view_state.controlled_entity_id.clone();
            let duplicate_stale_seq =
                message.reason == "stale_seq" && pending_request_seq != Some(message.request_seq);
            if duplicate_stale_seq {
                if client_control_debug_logging_enabled() {
                    info!(
                        "client ignored duplicate stale control rejection player={} seq={} previous_controlled={} authoritative_controlled={} reason={}",
                        message.player_entity_id,
                        message.request_seq,
                        control_target_log_label(previous_controlled.as_deref()),
                        control_target_log_label(player_view_state.controlled_entity_id.as_deref()),
                        message.reason
                    );
                }
            } else {
                if client_control_debug_logging_enabled() {
                    warn!(
                        "client control handover reject player={} seq={} previous_controlled={} authoritative_controlled={} pending_requested={} reason={} result=reject",
                        message.player_entity_id,
                        message.request_seq,
                        control_target_log_label(previous_controlled.as_deref()),
                        control_target_log_label(player_view_state.controlled_entity_id.as_deref()),
                        control_target_log_label(pending_requested.as_deref()),
                        message.reason
                    );
                }
                warn!(
                    "client control request rejected player={} seq={} reason={}",
                    message.player_entity_id, message.request_seq, message.reason
                );
            }
            debug_state.handover_audit_entity_id = player_view_state.controlled_entity_id.clone();
            debug_state.handover_audit_started_at_s = Some(time.elapsed_secs_f64());
            debug_state.last_handover_audit_log_at_s = 0.0;
        }
    }
}

pub fn log_client_control_state_changes(
    session: Res<'_, ClientSession>,
    player_view_state: Res<'_, LocalPlayerViewState>,
    request_state: Res<'_, ClientControlRequestState>,
    mut debug_state: ResMut<'_, ClientControlDebugState>,
) {
    if !client_control_debug_logging_enabled() {
        return;
    }
    let Some(player_entity_id) = session.player_entity_id.as_ref() else {
        return;
    };
    let controlled_changed =
        debug_state.last_controlled_entity_id != player_view_state.controlled_entity_id;
    let pending_changed =
        debug_state.last_pending_controlled_entity_id != request_state.pending_controlled_entity_id;
    let detached_changed =
        debug_state.last_detached_free_camera != player_view_state.detached_free_camera;
    if controlled_changed || pending_changed || detached_changed {
        info!(
            "client control state player={} controlled={:?} pending={:?} pending_seq={:?} detached={}",
            player_entity_id,
            player_view_state.controlled_entity_id,
            request_state.pending_controlled_entity_id,
            request_state.pending_request_seq,
            player_view_state.detached_free_camera
        );
        debug_state.last_controlled_entity_id = player_view_state.controlled_entity_id.clone();
        debug_state.last_pending_controlled_entity_id =
            request_state.pending_controlled_entity_id.clone();
        debug_state.last_detached_free_camera = player_view_state.detached_free_camera;
    }
}

#[allow(clippy::type_complexity)]
pub fn audit_client_control_handover_resolution(
    time: Res<'_, Time>,
    session: Res<'_, ClientSession>,
    mut debug_state: ResMut<'_, ClientControlDebugState>,
    candidates: Query<
        '_,
        '_,
        (
            Entity,
            &'_ sidereal_game::EntityGuid,
            Has<lightyear::prelude::Replicated>,
            Has<lightyear::prelude::Predicted>,
            Has<lightyear::prelude::Interpolated>,
            Has<ControlledEntity>,
            Has<sidereal_game::SimulationMotionWriter>,
            Has<WorldEntity>,
        ),
    >,
) {
    if !client_control_debug_logging_enabled() {
        return;
    }
    let Some(target_entity_id) = debug_state.handover_audit_entity_id.clone() else {
        return;
    };
    let Some(target_guid) = parse_guid_from_entity_id(target_entity_id.as_str())
        .or_else(|| uuid::Uuid::parse_str(target_entity_id.as_str()).ok())
    else {
        return;
    };
    let now_s = time.elapsed_secs_f64();
    if now_s - debug_state.last_handover_audit_log_at_s < 1.0 {
        return;
    }
    debug_state.last_handover_audit_log_at_s = now_s;

    let mut lines = Vec::new();
    let mut saw_predicted = false;
    for (
        entity,
        guid,
        is_replicated,
        is_predicted,
        is_interpolated,
        is_controlled,
        is_motion_writer,
        is_world_entity,
    ) in &candidates
    {
        if guid.0 != target_guid {
            continue;
        }
        saw_predicted |= is_predicted;
        lines.push(format!(
            "entity={entity:?} replicated={is_replicated} predicted={is_predicted} interpolated={is_interpolated} controlled={is_controlled} motion_writer={is_motion_writer} world_entity={is_world_entity}"
        ));
    }
    let player = session.player_entity_id.as_deref().unwrap_or("<none>");
    if lines.is_empty() {
        warn!(
            "client control handover audit player={} target={} age_s={:.2} candidates=<none>",
            player,
            target_entity_id,
            debug_state
                .handover_audit_started_at_s
                .map(|started| (now_s - started).max(0.0))
                .unwrap_or_default()
        );
        return;
    }
    info!(
        "client control handover audit player={} target={} age_s={:.2} candidates={}",
        player,
        target_entity_id,
        debug_state
            .handover_audit_started_at_s
            .map(|started| (now_s - started).max(0.0))
            .unwrap_or_default(),
        lines.join(" | ")
    );
    if saw_predicted {
        debug_state.handover_audit_entity_id = None;
        debug_state.handover_audit_started_at_s = None;
    }
}
