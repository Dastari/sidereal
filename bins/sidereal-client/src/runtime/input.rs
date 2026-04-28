//! Client input: realtime input messages and input marker ownership.
#![allow(clippy::items_after_test_module)]

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use lightyear::prelude::client::{Client, Connected};
use lightyear::prelude::input::native::{ActionState, InputMarker};
use lightyear::prelude::{ConfirmedTick, LocalTimeline, MessageSender};
use sidereal_game::{EntityAction, SimulationMotionWriter};
use sidereal_net::{ClientRealtimeInputMessage, InputChannel, PlayerEntityId, PlayerInput};
use sidereal_runtime_sync::parse_guid_from_entity_id;
use std::sync::OnceLock;

use super::app_state::{
    ClientAppState, ClientSession, LocalPlayerViewState, SessionReadyState, is_active_world_state,
};
use super::components::ControlledEntity;
use super::dev_console::DevConsoleState;
use super::resources::{
    ClientControlRequestState, ClientInputAckTracker, ClientInputSendState,
    ClientInputTimelineTuning, ClientNetworkTick, ControlBootstrapPhase, ControlBootstrapState,
    HeadlessTransportMode, NativePredictionRecoveryPhase, NativePredictionRecoveryState,
    NativePredictionRecoveryTuning, PredictionRecoveryReason,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct InputAxes {
    pub thrust: f32,
    pub turn: f32,
    pub brake: bool,
    pub afterburner: bool,
    pub fire_primary: bool,
}

#[derive(SystemParam)]
pub(crate) struct PredictionGuardParams<'w, 's> {
    input_tuning: Res<'w, ClientInputTimelineTuning>,
    recovery_tuning: Res<'w, NativePredictionRecoveryTuning>,
    timeline: Res<'w, LocalTimeline>,
    confirmed_ticks: Query<'w, 's, &'static ConfirmedTick>,
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

fn player_input_has_active_intent(input: &PlayerInput) -> bool {
    input.actions.iter().any(|action| {
        !matches!(
            action,
            EntityAction::LongitudinalNeutral
                | EntityAction::LateralNeutral
                | EntityAction::AfterburnerOff
        )
    })
}

#[derive(SystemParam)]
pub(crate) struct ClientInputSessionState<'w> {
    session: Res<'w, ClientSession>,
    session_ready: Res<'w, SessionReadyState>,
    player_view_state: Res<'w, LocalPlayerViewState>,
    request_state: Res<'w, ClientControlRequestState>,
}

#[derive(Debug, Clone, Copy)]
struct HeadlessInputScript {
    thrust: f32,
    turn: f32,
    brake: bool,
    afterburner: bool,
    fire_primary: bool,
    duration_s: f64,
}

fn parse_headless_input_script(raw: &str) -> Option<HeadlessInputScript> {
    let (mode, duration) = raw.split_once(':').unwrap_or((raw, "1.0"));
    let duration_s = duration
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite() && *value > 0.0)?;
    let normalized = mode.trim().to_ascii_lowercase();
    let script = match normalized.as_str() {
        "forward" => HeadlessInputScript {
            thrust: 1.0,
            turn: 0.0,
            brake: false,
            afterburner: false,
            fire_primary: false,
            duration_s,
        },
        "forward_afterburner" => HeadlessInputScript {
            thrust: 1.0,
            turn: 0.0,
            brake: false,
            afterburner: true,
            fire_primary: false,
            duration_s,
        },
        "turn_left" => HeadlessInputScript {
            thrust: 0.0,
            turn: 1.0,
            brake: false,
            afterburner: false,
            fire_primary: false,
            duration_s,
        },
        _ => return None,
    };
    Some(script)
}

fn headless_input_script() -> Option<HeadlessInputScript> {
    static SCRIPT: OnceLock<Option<HeadlessInputScript>> = OnceLock::new();
    *SCRIPT.get_or_init(|| {
        std::env::var("SIDEREAL_CLIENT_HEADLESS_INPUT_SCRIPT")
            .ok()
            .and_then(|raw| parse_headless_input_script(&raw))
    })
}

fn scripted_headless_player_input(
    now_s: f64,
    script_started_at_s: &mut Option<f64>,
) -> Option<(PlayerInput, InputAxes)> {
    let script = headless_input_script()?;
    let started_at_s = *script_started_at_s.get_or_insert(now_s);
    if now_s - started_at_s > script.duration_s {
        return None;
    }
    let axes = InputAxes {
        thrust: script.thrust,
        turn: script.turn,
        brake: script.brake,
        afterburner: script.afterburner,
        fire_primary: script.fire_primary,
    };
    Some((
        PlayerInput::from_axis_inputs(
            script.thrust,
            script.turn,
            script.brake,
            script.afterburner,
            script.fire_primary,
        ),
        axes,
    ))
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

fn ids_refer_to_same_guid(left: &str, right: &str) -> bool {
    if left == right {
        return true;
    }
    parse_guid_from_entity_id(left)
        .zip(parse_guid_from_entity_id(right))
        .is_some_and(|(l, r)| l == r)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ActiveControlInputTarget {
    entity: Entity,
    target_entity_id: String,
    generation: u64,
}

fn active_control_input_target(
    control_bootstrap_state: &ControlBootstrapState,
    request_state: &ClientControlRequestState,
) -> Option<ActiveControlInputTarget> {
    if request_state.pending_request_seq.is_some() {
        return None;
    }
    let ControlBootstrapPhase::ActivePredicted {
        target_entity_id,
        generation,
        entity,
    } = &control_bootstrap_state.phase
    else {
        return None;
    };
    if *generation != control_bootstrap_state.generation {
        return None;
    }
    if control_bootstrap_state
        .authoritative_target_entity_id
        .as_deref()
        .is_none_or(|authoritative| !ids_refer_to_same_guid(authoritative, target_entity_id))
    {
        return None;
    }
    Some(ActiveControlInputTarget {
        entity: *entity,
        target_entity_id: target_entity_id.clone(),
        generation: *generation,
    })
}

fn reset_input_send_state_for_inactive_lease(
    ack_tracker: &mut ClientInputAckTracker,
    input_send_state: &mut ClientInputSendState,
    recovery_state: &mut NativePredictionRecoveryState,
) {
    ack_tracker.pending_ticks.clear();
    input_send_state.last_sent_at_s = f64::NEG_INFINITY;
    input_send_state.last_sent_actions.clear();
    input_send_state.last_sent_target_entity_id = None;
    recovery_state.pending_neutral_send = false;
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
    session_state: ClientInputSessionState<'_>,
    control_bootstrap_state: Res<'_, ControlBootstrapState>,
    mut tick: ResMut<'_, ClientNetworkTick>,
    mut ack_tracker: ResMut<'_, ClientInputAckTracker>,
    mut input_send_state: ResMut<'_, ClientInputSendState>,
    mut recovery_state: ResMut<'_, NativePredictionRecoveryState>,
    prediction_guard: PredictionGuardParams<'_, '_>,
) {
    let suppress_for_console = super::dev_console::is_console_open(dev_console_state.as_deref());
    let in_world_state = is_active_world_state(&app_state, &headless_mode);
    let window_focused = windows.single().map(|w| w.focused).unwrap_or(true);
    let now_s = time.elapsed_secs_f64();
    recovery_state.complete_recovery_if_elapsed(now_s);
    let suppress_for_recovery = recovery_state.is_suppressing_input(now_s);
    let force_neutral_send = recovery_state.pending_neutral_send;

    let (player_entity_id, active_target, mut player_input) = if in_world_state {
        let Some(player_entity_id) = session_state.session.player_entity_id.clone() else {
            return;
        };
        let Some(canonical_player_entity_id) =
            PlayerEntityId::parse(player_entity_id.as_str()).map(PlayerEntityId::canonical_wire_id)
        else {
            return;
        };
        let session_ready_for_player = session_state
            .session_ready
            .ready_player_entity_id
            .as_deref()
            .and_then(PlayerEntityId::parse)
            .is_some_and(|ready_id| ready_id.canonical_wire_id() == canonical_player_entity_id);
        if !session_ready_for_player {
            return;
        }
        let Some(active_target) =
            active_control_input_target(&control_bootstrap_state, &session_state.request_state)
        else {
            reset_input_send_state_for_inactive_lease(
                &mut ack_tracker,
                &mut input_send_state,
                &mut recovery_state,
            );
            return;
        };
        let controlling_player_anchor = ids_refer_to_same_guid(
            active_target.target_entity_id.as_str(),
            player_entity_id.as_str(),
        );
        let suppress_input_for_camera_only =
            session_state.player_view_state.detached_free_camera && !controlling_player_anchor;
        let suppress_active_input = suppress_input_for_camera_only
            || !window_focused
            || suppress_for_console
            || suppress_for_recovery
            || force_neutral_send;
        let (player_input, _axes) = if suppress_active_input {
            neutral_player_input()
        } else if headless_mode.0 {
            scripted_headless_player_input(
                now_s,
                &mut input_send_state.headless_script_started_at_s,
            )
            .unwrap_or_else(neutral_player_input)
        } else {
            player_input_from_keyboard(input.as_deref())
        };
        (player_entity_id, active_target, player_input)
    } else {
        return;
    };

    let mut has_active_input = player_input_has_active_intent(&player_input);
    if has_active_input
        && prediction_confirmed_tick_gap_exceeded(
            active_target.entity,
            &prediction_guard.confirmed_ticks,
            &prediction_guard.timeline,
            &prediction_guard.input_tuning,
            &prediction_guard.recovery_tuning,
            window_focused,
        )
    {
        let (neutral, _) = neutral_player_input();
        player_input = neutral;
        has_active_input = false;
        recovery_state.phase = NativePredictionRecoveryPhase::Recovering {
            regain_at_s: now_s,
            suppress_input_until_s: now_s + prediction_guard.recovery_tuning.suppress_input_s,
            reason: PredictionRecoveryReason::ConfirmedTickGapExceeded,
        };
        recovery_state.transition_count = recovery_state.transition_count.saturating_add(1);
        recovery_state.pending_neutral_send = true;
        warn!(
            entity = ?active_target.entity,
            current_tick = prediction_guard.timeline.tick().0,
            max_predicted_ticks = prediction_guard.input_tuning.max_predicted_ticks,
            unfocused_max_predicted_ticks = prediction_guard.input_tuning.unfocused_max_predicted_ticks,
            recovery_max_tick_gap = prediction_guard.recovery_tuning.max_tick_gap,
            "client prediction confirmed tick gap exceeded; suppressing active local input until confirmation catches up"
        );
    }

    // Canonical ids for network message so server lookup matches (same form used for target_changed).
    let message_player_id = canonical_player_entity_id(&player_entity_id);
    let message_controlled_id =
        if canonical_player_entity_id(&active_target.target_entity_id) == message_player_id {
            message_player_id.clone()
        } else {
            active_target.target_entity_id.clone()
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

    commands.entity(active_target.entity).insert((
        SimulationMotionWriter,
        InputMarker::<PlayerInput>::default(),
        ActionState(player_input.clone()),
    ));

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
        control_generation: active_target.generation,
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

fn prediction_confirmed_tick_gap_exceeded(
    target_entity: Entity,
    confirmed_ticks: &Query<'_, '_, &'_ ConfirmedTick>,
    timeline: &LocalTimeline,
    input_tuning: &ClientInputTimelineTuning,
    recovery_tuning: &NativePredictionRecoveryTuning,
    window_focused: bool,
) -> bool {
    let Ok(confirmed_tick) = confirmed_ticks.get(target_entity) else {
        return false;
    };
    let current_tick = u32::from(timeline.tick().0);
    let confirmed_tick = u32::from(confirmed_tick.tick.0);
    let gap = current_tick.saturating_sub(confirmed_tick);
    let max_predicted_ticks = if window_focused {
        input_tuning.max_predicted_ticks
    } else {
        input_tuning.unfocused_max_predicted_ticks
    };
    let budget = u32::from(max_predicted_ticks)
        .saturating_add(u32::from(input_tuning.fixed_input_delay_ticks))
        .saturating_add(4);
    let recovery_gap = recovery_tuning.max_tick_gap.max(1);
    gap > budget.min(recovery_gap)
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

    #[test]
    fn neutral_input_is_not_active_intent() {
        let (input, _) = neutral_player_input();
        assert!(!player_input_has_active_intent(&input));
    }

    #[test]
    fn active_input_detects_movement_afterburner_and_fire() {
        let forward = PlayerInput::from_axis_inputs(1.0, 0.0, false, false, false);
        let afterburner = PlayerInput::from_axis_inputs(0.0, 0.0, false, true, false);
        let fire = PlayerInput::from_axis_inputs(0.0, 0.0, false, false, true);

        assert!(player_input_has_active_intent(&forward));
        assert!(player_input_has_active_intent(&afterburner));
        assert!(player_input_has_active_intent(&fire));
    }

    #[test]
    fn headless_input_script_parses_forward_duration() {
        let script = parse_headless_input_script("forward:2.5").unwrap();
        assert_eq!(script.thrust, 1.0);
        assert_eq!(script.turn, 0.0);
        assert!(!script.fire_primary);
        assert_eq!(script.duration_s, 2.5);
    }

    #[test]
    fn active_input_target_requires_active_predicted_bootstrap() {
        let target_entity_id = "11111111-1111-1111-1111-111111111111".to_string();
        let entity = Entity::from_bits(42);
        let request_state = ClientControlRequestState::default();
        let active = ControlBootstrapState {
            authoritative_target_entity_id: Some(target_entity_id.clone()),
            generation: 7,
            phase: ControlBootstrapPhase::ActivePredicted {
                target_entity_id: target_entity_id.clone(),
                generation: 7,
                entity,
            },
            last_transition_at_s: 0.0,
        };

        assert_eq!(
            active_control_input_target(&active, &request_state),
            Some(ActiveControlInputTarget {
                entity,
                target_entity_id,
                generation: 7,
            })
        );
    }

    #[test]
    fn active_input_target_rejects_pending_request_and_generation_mismatch() {
        let target_entity_id = "11111111-1111-1111-1111-111111111111".to_string();
        let entity = Entity::from_bits(42);
        let active = ControlBootstrapState {
            authoritative_target_entity_id: Some(target_entity_id.clone()),
            generation: 7,
            phase: ControlBootstrapPhase::ActivePredicted {
                target_entity_id,
                generation: 6,
                entity,
            },
            last_transition_at_s: 0.0,
        };
        assert!(
            active_control_input_target(&active, &ClientControlRequestState::default()).is_none()
        );

        let active = ControlBootstrapState {
            generation: 7,
            phase: ControlBootstrapPhase::ActivePredicted {
                target_entity_id: "11111111-1111-1111-1111-111111111111".to_string(),
                generation: 7,
                entity,
            },
            authoritative_target_entity_id: Some(
                "11111111-1111-1111-1111-111111111111".to_string(),
            ),
            last_transition_at_s: 0.0,
        };
        let request_state = ClientControlRequestState {
            pending_request_seq: Some(3),
            ..Default::default()
        };
        assert!(active_control_input_target(&active, &request_state).is_none());
    }

    #[test]
    fn marker_owner_keeps_only_exact_active_predicted_entity() {
        let player_entity_id = "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa".to_string();
        let target_entity_id = "11111111-1111-1111-1111-111111111111".to_string();
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.insert_resource(ClientSession {
            player_entity_id: Some(player_entity_id.clone()),
            ..Default::default()
        });
        app.insert_resource(ClientControlRequestState::default());
        let active_entity = app
            .world_mut()
            .spawn((
                ControlledEntity {
                    entity_id: target_entity_id.clone(),
                    player_entity_id: player_entity_id.clone(),
                },
                SimulationMotionWriter,
                InputMarker::<PlayerInput>::default(),
                ActionState(PlayerInput::default()),
            ))
            .id();
        let stale_entity = app
            .world_mut()
            .spawn((
                ControlledEntity {
                    entity_id: "22222222-2222-2222-2222-222222222222".to_string(),
                    player_entity_id: player_entity_id.clone(),
                },
                SimulationMotionWriter,
                InputMarker::<PlayerInput>::default(),
                ActionState(PlayerInput::default()),
            ))
            .id();
        app.insert_resource(ControlBootstrapState {
            authoritative_target_entity_id: Some(target_entity_id.clone()),
            generation: 3,
            phase: ControlBootstrapPhase::ActivePredicted {
                target_entity_id,
                generation: 3,
                entity: active_entity,
            },
            last_transition_at_s: 0.0,
        });
        app.add_systems(Update, enforce_single_input_marker_owner);

        app.update();

        assert!(
            app.world()
                .get::<InputMarker<PlayerInput>>(active_entity)
                .is_some()
        );
        assert!(
            app.world()
                .get::<SimulationMotionWriter>(active_entity)
                .is_some()
        );
        assert!(
            app.world()
                .get::<InputMarker<PlayerInput>>(stale_entity)
                .is_none()
        );
        assert!(
            app.world()
                .get::<SimulationMotionWriter>(stale_entity)
                .is_none()
        );
    }

    #[test]
    fn marker_owner_removes_all_markers_while_control_request_is_pending() {
        let player_entity_id = "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa".to_string();
        let target_entity_id = "11111111-1111-1111-1111-111111111111".to_string();
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.insert_resource(ClientSession {
            player_entity_id: Some(player_entity_id.clone()),
            ..Default::default()
        });
        app.insert_resource(ClientControlRequestState {
            pending_request_seq: Some(10),
            ..Default::default()
        });
        let active_entity = app
            .world_mut()
            .spawn((
                ControlledEntity {
                    entity_id: target_entity_id.clone(),
                    player_entity_id,
                },
                SimulationMotionWriter,
                InputMarker::<PlayerInput>::default(),
                ActionState(PlayerInput::default()),
            ))
            .id();
        app.insert_resource(ControlBootstrapState {
            authoritative_target_entity_id: Some(target_entity_id.clone()),
            generation: 3,
            phase: ControlBootstrapPhase::ActivePredicted {
                target_entity_id,
                generation: 3,
                entity: active_entity,
            },
            last_transition_at_s: 0.0,
        });
        app.add_systems(Update, enforce_single_input_marker_owner);

        app.update();

        assert!(
            app.world()
                .get::<InputMarker<PlayerInput>>(active_entity)
                .is_none()
        );
        assert!(
            app.world()
                .get::<SimulationMotionWriter>(active_entity)
                .is_none()
        );
    }
}

#[allow(clippy::type_complexity)]
pub fn enforce_single_input_marker_owner(
    mut commands: Commands<'_, '_>,
    session: Res<'_, ClientSession>,
    request_state: Res<'_, ClientControlRequestState>,
    control_bootstrap_state: Res<'_, ControlBootstrapState>,
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
    let active_target = active_control_input_target(&control_bootstrap_state, &request_state);

    for (entity, controlled) in &input_marked_entities {
        let keep = active_target
            .as_ref()
            .is_some_and(|target| entity == target.entity)
            && controlled.is_none_or(|controlled| {
                ids_refer_to_same_guid(&controlled.player_entity_id, player_entity_id)
                    || controlled.player_entity_id == *player_entity_id
            });
        if keep {
            continue;
        }
        commands.entity(entity).remove::<(
            InputMarker<PlayerInput>,
            ActionState<PlayerInput>,
            SimulationMotionWriter,
        )>();
    }
}
