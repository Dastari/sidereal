//! Client input: realtime input messages, input marker ownership, debug logging.

use bevy::log::info;
use bevy::prelude::*;
use lightyear::prelude::client::{Client, Connected};
use lightyear::prelude::input::native::{ActionState, InputMarker};
use lightyear::prelude::MessageSender;
use sidereal_game::EntityAction;
use sidereal_net::{ClientRealtimeInputMessage, ControlChannel, PlayerInput};
use sidereal_runtime_sync::RuntimeEntityHierarchy;
use std::sync::OnceLock;

use crate::client::input::{neutral_player_input, player_input_from_keyboard};

use super::components::ControlledEntity;
use super::resources::{
    ClientControlRequestState, ClientInputAckTracker, ClientInputLogState, ClientInputSendState,
    ClientNetworkTick, HeadlessTransportMode,
};
use super::state::{ClientAppState, ClientSession, LocalPlayerViewState};

pub fn should_send_realtime_input_message(
    now_s: f64,
    last_sent_at_s: f64,
    input_changed: bool,
    target_changed: bool,
) -> bool {
    const HEARTBEAT_INTERVAL_S: f64 = 0.1;
    input_changed || target_changed || (now_s - last_sent_at_s) >= HEARTBEAT_INTERVAL_S
}

pub fn client_input_debug_logging_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("SIDEREAL_DEBUG_INPUT_LOGS")
            .is_ok_and(|v| v == "1" || v.eq_ignore_ascii_case("true"))
    })
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn send_lightyear_input_messages(
    input: Option<Res<'_, ButtonInput<KeyCode>>>,
    app_state: Option<Res<'_, State<ClientAppState>>>,
    headless_mode: Res<'_, HeadlessTransportMode>,
    time: Res<'_, Time>,
    mut commands: Commands<'_, '_>,
    mut realtime_input_senders: Query<
        '_,
        '_,
        &'_ mut MessageSender<ClientRealtimeInputMessage>,
        (With<Client>, With<Connected>),
    >,
    session: Res<'_, ClientSession>,
    player_view_state: Res<'_, LocalPlayerViewState>,
    entity_registry: Res<'_, RuntimeEntityHierarchy>,
    mut tick: ResMut<'_, ClientNetworkTick>,
    mut ack_tracker: ResMut<'_, ClientInputAckTracker>,
    mut input_log_state: ResMut<'_, ClientInputLogState>,
    mut input_send_state: ResMut<'_, ClientInputSendState>,
    request_state: Res<'_, ClientControlRequestState>,
) {
    let in_world_state = app_state
        .as_ref()
        .is_some_and(|state| **state == ClientAppState::InWorld)
        || headless_mode.0;

    let (player_entity_id, player_input) = if in_world_state {
        let Some(player_entity_id) = session.player_entity_id.clone() else {
            return;
        };
        let (player_input, _axes) = if player_view_state.detached_free_camera {
            neutral_player_input()
        } else {
            player_input_from_keyboard(input.as_deref())
        };
        (player_entity_id, player_input)
    } else {
        return;
    };

    let now_s = time.elapsed_secs_f64();
    let has_active_input = player_input.actions.iter().any(|a| {
        !matches!(
            a,
            EntityAction::ThrustNeutral
                | EntityAction::YawNeutral
                | EntityAction::LongitudinalNeutral
                | EntityAction::LateralNeutral
        )
    });
    let target_entity_id = player_view_state
        .controlled_entity_id
        .as_ref()
        .filter(|id| entity_registry.by_entity_id.contains_key(id.as_str()))
        .cloned()
        .unwrap_or_else(|| player_entity_id.clone());
    let target_entity = entity_registry
        .by_entity_id
        .get(target_entity_id.as_str())
        .copied();

    let input_changed = input_send_state.last_sent_actions != player_input.actions;
    let target_changed =
        input_send_state.last_sent_target_entity_id.as_deref() != Some(target_entity_id.as_str());
    let should_send_network = should_send_realtime_input_message(
        now_s,
        input_send_state.last_sent_at_s,
        input_changed,
        target_changed,
    );

    if client_input_debug_logging_enabled() {
        let actions_changed = input_log_state.last_logged_actions != player_input.actions;
        let control_changed = input_log_state.last_logged_controlled_entity_id
            != player_view_state.controlled_entity_id
            || input_log_state.last_logged_pending_controlled_entity_id
                != request_state.pending_controlled_entity_id;
        let periodic_active_log_due =
            has_active_input && now_s - input_log_state.last_logged_at_s >= 0.15;
        if periodic_active_log_due || actions_changed || control_changed {
            input_log_state.last_logged_at_s = now_s;
            input_log_state.last_logged_actions = player_input.actions.clone();
            input_log_state.last_logged_controlled_entity_id =
                player_view_state.controlled_entity_id.clone();
            input_log_state.last_logged_pending_controlled_entity_id =
                request_state.pending_controlled_entity_id.clone();
            info!(
                player = %player_entity_id,
                actions = ?player_input.actions,
                tick = tick.0.saturating_add(1),
                controlled = ?player_view_state.controlled_entity_id,
                routed_target = %target_entity_id,
                pending = ?request_state.pending_controlled_entity_id,
                detached = player_view_state.detached_free_camera,
                send = should_send_network,
                "client sending input route"
            );
        }
    }
    if let Some(target_entity) = target_entity {
        commands.entity(target_entity).insert((
            ControlledEntity {
                entity_id: target_entity_id.clone(),
                player_entity_id: player_entity_id.clone(),
            },
            InputMarker::<PlayerInput>::default(),
            ActionState(player_input.clone()),
        ));
    }

    if !should_send_network {
        return;
    }
    tick.0 = tick.0.saturating_add(1);
    ack_tracker.pending_ticks.push_back(tick.0);
    while ack_tracker.pending_ticks.len() > 512 {
        ack_tracker.pending_ticks.pop_front();
    }

    let realtime_message = ClientRealtimeInputMessage {
        player_entity_id,
        controlled_entity_id: target_entity_id,
        actions: player_input.actions,
        tick: tick.0,
    };
    for mut sender in &mut realtime_input_senders {
        sender.send::<ControlChannel>(realtime_message.clone());
    }
    input_send_state.last_sent_at_s = now_s;
    input_send_state.last_sent_actions = realtime_message.actions;
    input_send_state.last_sent_target_entity_id = Some(realtime_message.controlled_entity_id);
}

#[allow(clippy::type_complexity)]
pub fn enforce_single_input_marker_owner(
    mut commands: Commands<'_, '_>,
    session: Res<'_, ClientSession>,
    player_view_state: Res<'_, LocalPlayerViewState>,
    entity_registry: Res<'_, RuntimeEntityHierarchy>,
    input_marked_entities: Query<
        '_,
        '_,
        (Entity, Option<&'_ ControlledEntity>),
        With<InputMarker<PlayerInput>>,
    >,
) {
    let Some(player_entity_id) = session.player_entity_id.as_ref() else {
        return;
    };
    let target_entity_id = player_view_state
        .controlled_entity_id
        .as_ref()
        .filter(|id| entity_registry.by_entity_id.contains_key(id.as_str()))
        .cloned()
        .unwrap_or_else(|| player_entity_id.clone());
    let target_entity = entity_registry
        .by_entity_id
        .get(target_entity_id.as_str())
        .copied();

    for (entity, controlled) in &input_marked_entities {
        let keep = Some(entity) == target_entity
            && controlled
                .is_some_and(|controlled| controlled.player_entity_id == *player_entity_id);
        if keep {
            continue;
        }
        commands
            .entity(entity)
            .remove::<(InputMarker<PlayerInput>, ActionState<PlayerInput>)>();
    }
}
