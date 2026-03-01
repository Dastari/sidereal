//! Control request/response: send control requests, receive acks/rejects, log state.

use bevy::log::{info, warn};
use bevy::prelude::*;
use lightyear::prelude::client::{Client, Connected};
use lightyear::prelude::{MessageReceiver, MessageSender};
use sidereal_net::{
    ClientControlRequestMessage, ControlChannel, ServerControlAckMessage,
    ServerControlRejectMessage,
};
use std::sync::OnceLock;

use super::app_state::{
    ClientAppState, ClientSession, LocalPlayerViewState, is_active_world_state,
};
use super::resources::{ClientControlDebugState, ClientControlRequestState, HeadlessTransportMode};

pub fn client_control_debug_logging_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("SIDEREAL_DEBUG_CONTROL_LOGS")
            .is_ok_and(|v| v == "1" || v.eq_ignore_ascii_case("true"))
    })
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
    if senders.is_empty() {
        return;
    }

    if request_state.pending_request_seq.is_none() {
        let desired = player_view_state
            .desired_controlled_entity_id
            .clone()
            .or_else(|| player_view_state.controlled_entity_id.clone())
            .or_else(|| session.player_entity_id.clone());
        if desired != player_view_state.controlled_entity_id {
            request_state.next_request_seq = request_state.next_request_seq.saturating_add(1);
            request_state.pending_controlled_entity_id = desired;
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
        player_entity_id: player_entity_id.clone(),
        controlled_entity_id: requested_controlled_entity_id,
        request_seq,
    };

    for mut sender in &mut senders {
        sender.send::<ControlChannel>(message.clone());
    }
    request_state.last_sent_request_seq = Some(request_seq);
    request_state.last_sent_at_s = now_s;
}

pub fn receive_lightyear_control_results(
    session: Res<'_, ClientSession>,
    mut player_view_state: ResMut<'_, LocalPlayerViewState>,
    mut request_state: ResMut<'_, ClientControlRequestState>,
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

    for mut receiver in &mut ack_receivers {
        for message in receiver.receive() {
            if message.player_entity_id != *local_player_entity_id {
                continue;
            }
            if request_state.pending_request_seq == Some(message.request_seq) {
                request_state.pending_controlled_entity_id = None;
                request_state.pending_request_seq = None;
                request_state.last_sent_request_seq = None;
            }
            if let Some(controlled_entity_id) = message.controlled_entity_id {
                player_view_state.controlled_entity_id = Some(controlled_entity_id);
            } else {
                player_view_state.controlled_entity_id = session.player_entity_id.clone();
            }
            player_view_state.desired_controlled_entity_id =
                player_view_state.controlled_entity_id.clone();
        }
    }

    for mut receiver in &mut reject_receivers {
        for message in receiver.receive() {
            if message.player_entity_id != *local_player_entity_id {
                continue;
            }
            if request_state.pending_request_seq == Some(message.request_seq) {
                request_state.pending_controlled_entity_id = None;
                request_state.pending_request_seq = None;
                request_state.last_sent_request_seq = None;
            }
            if let Some(authoritative) = message.authoritative_controlled_entity_id {
                player_view_state.controlled_entity_id = Some(authoritative);
            } else if player_view_state.controlled_entity_id.is_none() {
                player_view_state.controlled_entity_id = session.player_entity_id.clone();
            }
            player_view_state.desired_controlled_entity_id =
                player_view_state.controlled_entity_id.clone();
            warn!(
                "client control request rejected player={} seq={} reason={}",
                message.player_entity_id, message.request_seq, message.reason
            );
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
