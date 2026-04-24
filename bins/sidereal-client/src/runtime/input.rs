//! Client input: realtime input messages and input marker ownership.
#![allow(clippy::items_after_test_module)]

use bevy::prelude::*;
use lightyear::prelude::MessageSender;
use lightyear::prelude::client::{Client, Connected};
use lightyear::prelude::input::native::{ActionState, InputMarker};
use sidereal_game::{EntityAction, EntityGuid, SimulationMotionWriter};
use sidereal_net::{ClientRealtimeInputMessage, InputChannel, PlayerEntityId, PlayerInput};
use sidereal_runtime_sync::parse_guid_from_entity_id;

use super::app_state::{
    ClientAppState, ClientSession, LocalPlayerViewState, is_active_world_state,
};
use super::components::ControlledEntity;
use super::dev_console::DevConsoleState;
use super::resources::{
    ClientControlRequestState, ClientInputAckTracker, ClientInputSendState, ClientNetworkTick,
    HeadlessTransportMode, NativePredictionRecoveryState,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct InputAxes {
    pub thrust: f32,
    pub turn: f32,
    pub brake: bool,
    pub afterburner: bool,
    pub fire_primary: bool,
}

pub(crate) fn player_input_from_keyboard(
    input: Option<&ButtonInput<KeyCode>>,
) -> (PlayerInput, InputAxes) {
    let brake = input.is_some_and(|keys| {
        keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight)
    });
    let fire_primary = input.is_some_and(|keys| keys.pressed(KeyCode::Space));
    let thrust = if brake {
        0.0
    } else if input.is_some_and(|keys| keys.pressed(KeyCode::KeyW)) {
        1.0
    } else if input.is_some_and(|keys| keys.pressed(KeyCode::KeyS)) {
        -0.7
    } else {
        0.0
    };
    let turn = if input.is_some_and(|keys| keys.pressed(KeyCode::KeyA)) {
        1.0
    } else if input.is_some_and(|keys| keys.pressed(KeyCode::KeyD)) {
        -1.0
    } else {
        0.0
    };
    let afterburner = input
        .is_some_and(|keys| keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight));
    let axes = InputAxes {
        thrust,
        turn,
        brake,
        afterburner,
        fire_primary,
    };
    (
        PlayerInput::from_axis_inputs(thrust, turn, brake, afterburner, fire_primary),
        axes,
    )
}

pub(crate) fn neutral_player_input() -> (PlayerInput, InputAxes) {
    let axes = InputAxes {
        thrust: 0.0,
        turn: 0.0,
        brake: false,
        afterburner: false,
        fire_primary: false,
    };
    (
        PlayerInput::from_axis_inputs(0.0, 0.0, false, false, false),
        axes,
    )
}

pub fn should_send_realtime_input_message(
    now_s: f64,
    last_sent_at_s: f64,
    input_changed: bool,
    target_changed: bool,
) -> bool {
    const HEARTBEAT_INTERVAL_S: f64 = 0.1;
    input_changed || target_changed || (now_s - last_sent_at_s) >= HEARTBEAT_INTERVAL_S
}

/// Canonical form for player entity id so server/client lookup matches.
fn canonical_player_entity_id(id: &str) -> String {
    PlayerEntityId::parse(id)
        .map(PlayerEntityId::canonical_wire_id)
        .unwrap_or_else(|| id.to_string())
}

#[allow(clippy::type_complexity)]
fn resolve_entity_by_guid_prefer_predicted(
    guid_candidates: &Query<
        '_,
        '_,
        (
            Entity,
            Option<&'_ EntityGuid>,
            Has<lightyear::prelude::Predicted>,
            Has<lightyear::prelude::Interpolated>,
        ),
    >,
    guid_like: &str,
) -> Option<(Entity, bool)> {
    let target_guid =
        parse_guid_from_entity_id(guid_like).or_else(|| uuid::Uuid::parse_str(guid_like).ok())?;
    let mut winner: Option<(Entity, i32)> = None;
    for (entity, guid, is_predicted, is_interpolated) in guid_candidates {
        if guid.is_none_or(|guid| guid.0 != target_guid) {
            continue;
        }
        let score = if is_predicted {
            3
        } else if is_interpolated {
            2
        } else {
            1
        };
        if winner.is_none_or(|(_, best_score)| score > best_score) {
            winner = Some((entity, score));
        }
    }
    winner.map(|(entity, score)| (entity, score >= 3))
}

fn ids_refer_to_same_guid(left: &str, right: &str) -> bool {
    if left == right {
        return true;
    }
    parse_guid_from_entity_id(left)
        .zip(parse_guid_from_entity_id(right))
        .is_some_and(|(l, r)| l == r)
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn send_lightyear_input_messages(
    input: Option<Res<'_, ButtonInput<KeyCode>>>,
    dev_console_state: Option<Res<'_, DevConsoleState>>,
    windows: Query<'_, '_, &'_ Window, With<bevy::window::PrimaryWindow>>,
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
    guid_candidates: Query<
        '_,
        '_,
        (
            Entity,
            Option<&'_ EntityGuid>,
            Has<lightyear::prelude::Predicted>,
            Has<lightyear::prelude::Interpolated>,
        ),
    >,
    mut tick: ResMut<'_, ClientNetworkTick>,
    mut ack_tracker: ResMut<'_, ClientInputAckTracker>,
    mut input_send_state: ResMut<'_, ClientInputSendState>,
    mut recovery_state: ResMut<'_, NativePredictionRecoveryState>,
    _request_state: Res<'_, ClientControlRequestState>,
) {
    let suppress_for_console = super::dev_console::is_console_open(dev_console_state.as_deref());
    let in_world_state = is_active_world_state(&app_state, &headless_mode);
    let window_focused = windows.single().map(|w| w.focused).unwrap_or(true);
    let now_s = time.elapsed_secs_f64();
    recovery_state.complete_recovery_if_elapsed(now_s);
    let suppress_for_recovery = recovery_state.is_suppressing_input(now_s);
    let force_neutral_send = recovery_state.pending_neutral_send;

    let (player_entity_id, target_entity_id, player_input) = if in_world_state {
        let Some(player_entity_id) = session.player_entity_id.clone() else {
            return;
        };
        let target_entity_id = player_view_state
            .controlled_entity_id
            .as_ref()
            .cloned()
            .unwrap_or_else(|| player_entity_id.clone());
        let controlling_player_anchor =
            ids_refer_to_same_guid(target_entity_id.as_str(), player_entity_id.as_str());
        let suppress_input_for_camera_only =
            player_view_state.detached_free_camera && !controlling_player_anchor;
        let suppress_active_input = suppress_input_for_camera_only
            || !window_focused
            || suppress_for_console
            || suppress_for_recovery
            || force_neutral_send;
        let (player_input, _axes) = if suppress_active_input {
            neutral_player_input()
        } else {
            player_input_from_keyboard(input.as_deref())
        };
        (player_entity_id, target_entity_id, player_input)
    } else {
        return;
    };

    let has_active_input = player_input.actions.iter().any(|a| {
        !matches!(
            a,
            EntityAction::LongitudinalNeutral | EntityAction::LateralNeutral
        )
    });
    let target_entity =
        resolve_entity_by_guid_prefer_predicted(&guid_candidates, &target_entity_id)
            .map(|(entity, _)| entity);

    // Canonical ids for network message so server lookup matches (same form used for target_changed).
    let message_player_id = canonical_player_entity_id(&player_entity_id);
    let message_controlled_id =
        if canonical_player_entity_id(&target_entity_id) == message_player_id {
            message_player_id.clone()
        } else {
            target_entity_id.clone()
        };

    let input_changed = input_send_state.last_sent_actions != player_input.actions;
    let target_changed = input_send_state.last_sent_target_entity_id.as_deref()
        != Some(message_controlled_id.as_str());
    let should_send_network = should_send_realtime_input_message(
        now_s,
        input_send_state.last_sent_at_s,
        input_changed || force_neutral_send,
        target_changed,
    );
    // While movement/fire input is active, send every fixed tick so authoritative
    // server routing cannot stall on sparse heartbeats.
    let should_send_network = should_send_network || has_active_input;

    if let Some(target_entity) = target_entity {
        commands.entity(target_entity).insert((
            SimulationMotionWriter,
            InputMarker::<PlayerInput>::default(),
            ActionState(player_input.clone()),
        ));
    }

    if !should_send_network {
        return;
    }
    if force_neutral_send {
        recovery_state.pending_neutral_send = false;
        recovery_state.neutral_send_count = recovery_state.neutral_send_count.saturating_add(1);
    }
    tick.0 = tick.0.saturating_add(1);
    ack_tracker.pending_ticks.push_back(tick.0);
    while ack_tracker.pending_ticks.len() > 512 {
        ack_tracker.pending_ticks.pop_front();
    }

    let realtime_message = ClientRealtimeInputMessage {
        player_entity_id: message_player_id,
        controlled_entity_id: message_controlled_id,
        control_generation: player_view_state.controlled_entity_generation,
        actions: player_input.actions,
        tick: tick.0,
    };
    for mut sender in &mut realtime_input_senders {
        sender.send::<InputChannel>(realtime_message.clone());
    }
    input_send_state.last_sent_at_s = now_s;
    input_send_state.last_sent_actions = realtime_message.actions.clone();
    input_send_state.last_sent_target_entity_id = Some(realtime_message.controlled_entity_id);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neutral_input_has_no_actions() {
        let (input, axes) = neutral_player_input();
        assert_eq!(axes.thrust, 0.0);
        assert_eq!(axes.turn, 0.0);
        assert!(!axes.brake);
        assert!(!axes.afterburner);
        assert!(!axes.fire_primary);
        assert!(input.actions.iter().all(|action| {
            matches!(
                action,
                sidereal_game::EntityAction::LongitudinalNeutral
                    | sidereal_game::EntityAction::LateralNeutral
                    | sidereal_game::EntityAction::AfterburnerOff
            )
        }));
    }
}

#[allow(clippy::type_complexity)]
pub fn enforce_single_input_marker_owner(
    mut commands: Commands<'_, '_>,
    session: Res<'_, ClientSession>,
    player_view_state: Res<'_, LocalPlayerViewState>,
    guid_candidates: Query<
        '_,
        '_,
        (
            Entity,
            Option<&'_ EntityGuid>,
            Has<lightyear::prelude::Predicted>,
            Has<lightyear::prelude::Interpolated>,
        ),
    >,
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
        .cloned()
        .unwrap_or_else(|| player_entity_id.clone());
    let target_entity =
        resolve_entity_by_guid_prefer_predicted(&guid_candidates, &target_entity_id)
            .map(|(entity, _)| entity);

    for (entity, controlled) in &input_marked_entities {
        let keep = Some(entity) == target_entity
            && controlled.is_none_or(|controlled| {
                ids_refer_to_same_guid(&controlled.player_entity_id, player_entity_id)
                    || controlled.player_entity_id == *player_entity_id
            });
        if keep {
            continue;
        }
        commands
            .entity(entity)
            .remove::<(InputMarker<PlayerInput>, ActionState<PlayerInput>)>();
    }
}
