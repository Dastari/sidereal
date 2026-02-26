use bevy::prelude::*;
use lightyear::prelude::input::native::ActionState;
use lightyear::prelude::server::ClientOf;
use lightyear::prelude::{MessageReceiver, RemoteId};
use sidereal_game::{ActionQueue, ControlledEntityGuid, EntityAction, EntityGuid, PlayerTag};
use sidereal_net::{ClientRealtimeInputMessage, PlayerInput};
use std::collections::HashMap;
use std::sync::OnceLock;

use crate::AuthenticatedClientBindings;
use crate::replication::{PlayerControlledEntityMap, SimulatedControlledEntity};

#[derive(Resource, Default)]
pub struct ClientInputTickTracker {
    pub last_accepted_tick_by_player_entity_id: HashMap<String, u64>,
}

#[derive(Resource, Debug, Default)]
pub struct ClientInputDropMetrics {
    pub accepted_inputs: u64,
    pub future_tick: u64,
    pub duplicate_or_out_of_order_tick: u64,
    pub rate_limited: u64,
    pub oversized_packet: u64,
    pub empty_after_filter: u64,
    pub unbound_client: u64,
    pub spoofed_player_id: u64,
}

#[derive(Resource, Debug, Default)]
pub struct ClientInputDropMetricsLogState {
    pub last_logged_at_s: f64,
    pub last_accepted_inputs: u64,
}

#[derive(Resource, Debug, Default)]
pub struct InputActivityLogState {
    pub last_logged_at_s_by_player_entity_id: HashMap<String, f64>,
    pub last_logged_actions_by_player_entity_id: HashMap<String, Vec<EntityAction>>,
}

#[derive(Debug, Clone)]
pub struct LatestRealtimeInput {
    pub tick: u64,
    pub controlled_entity_id: String,
    pub actions: Vec<EntityAction>,
}

#[derive(Resource, Default)]
pub struct LatestRealtimeInputsByPlayer {
    pub by_player_entity_id: HashMap<String, LatestRealtimeInput>,
}

#[derive(Resource, Debug, Default)]
pub struct InputRateLimitState {
    pub current_window_index_by_player_entity_id: HashMap<String, u64>,
    pub message_count_in_window_by_player_entity_id: HashMap<String, u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputValidationFailure {
    FutureTick,
    DuplicateOrOutOfOrder,
    RateLimited,
    OversizedPacket,
}

const MAX_ACTIONS_PER_PACKET: usize = 32;
const MAX_TICKS_AHEAD: u64 = 6;
const MAX_MESSAGES_PER_SECOND: u32 = 120;

impl ClientInputDropMetrics {
    fn record_accepted(&mut self) {
        self.accepted_inputs = self.accepted_inputs.saturating_add(1);
    }

    fn total_drops(&self) -> u64 {
        self.future_tick
            .saturating_add(self.duplicate_or_out_of_order_tick)
            .saturating_add(self.rate_limited)
            .saturating_add(self.oversized_packet)
            .saturating_add(self.empty_after_filter)
            .saturating_add(self.unbound_client)
            .saturating_add(self.spoofed_player_id)
    }
}

fn validate_input_message(
    message: &ClientRealtimeInputMessage,
    last_accepted_tick: Option<u64>,
    now_s: f64,
    rate_limit_state: &mut InputRateLimitState,
) -> Result<(), InputValidationFailure> {
    if message.actions.len() > MAX_ACTIONS_PER_PACKET {
        return Err(InputValidationFailure::OversizedPacket);
    }
    if let Some(last_tick) = last_accepted_tick {
        if message.tick <= last_tick {
            return Err(InputValidationFailure::DuplicateOrOutOfOrder);
        }
        if message.tick > last_tick.saturating_add(MAX_TICKS_AHEAD) {
            return Err(InputValidationFailure::FutureTick);
        }
    }
    let window_index = now_s.max(0.0).floor() as u64;
    let player_entity_id = message.player_entity_id.as_str();
    let stored_window = rate_limit_state
        .current_window_index_by_player_entity_id
        .entry(player_entity_id.to_string())
        .or_insert(window_index);
    if *stored_window != window_index {
        *stored_window = window_index;
        rate_limit_state
            .message_count_in_window_by_player_entity_id
            .insert(player_entity_id.to_string(), 0);
    }
    let counter = rate_limit_state
        .message_count_in_window_by_player_entity_id
        .entry(player_entity_id.to_string())
        .or_insert(0);
    if *counter >= MAX_MESSAGES_PER_SECOND {
        return Err(InputValidationFailure::RateLimited);
    }
    *counter = counter.saturating_add(1);
    Ok(())
}

fn input_debug_logging_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("SIDEREAL_DEBUG_INPUT_LOGS")
            .is_ok_and(|v| v == "1" || v.eq_ignore_ascii_case("true"))
    })
}

fn summary_logging_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("SIDEREAL_REPLICATION_SUMMARY_LOGS")
            .is_ok_and(|v| v == "1" || v.eq_ignore_ascii_case("true"))
    })
}

#[allow(clippy::type_complexity)]
#[allow(clippy::too_many_arguments)]
pub fn receive_latest_realtime_input_messages(
    time: Res<'_, Time>,
    mut last_activity: ResMut<'_, crate::ClientLastActivity>,
    bindings: Res<'_, AuthenticatedClientBindings>,
    mut input_tick_tracker: ResMut<'_, ClientInputTickTracker>,
    mut input_drop_metrics: ResMut<'_, ClientInputDropMetrics>,
    mut rate_limit_state: ResMut<'_, InputRateLimitState>,
    mut latest: ResMut<'_, LatestRealtimeInputsByPlayer>,
    mut receivers: Query<
        '_,
        '_,
        (
            Entity,
            &'_ RemoteId,
            &'_ mut MessageReceiver<ClientRealtimeInputMessage>,
        ),
        With<ClientOf>,
    >,
) {
    let now_s = time.elapsed_secs_f64();
    for (client_entity, remote_id, mut receiver) in &mut receivers {
        let Some(bound_player_entity_id) = bindings.by_client_entity.get(&client_entity) else {
            for _ in receiver.receive() {
                input_drop_metrics.unbound_client =
                    input_drop_metrics.unbound_client.saturating_add(1);
            }
            continue;
        };
        for message in receiver.receive() {
            last_activity.0.insert(client_entity, now_s);
            if message.player_entity_id != *bound_player_entity_id {
                input_drop_metrics.spoofed_player_id =
                    input_drop_metrics.spoofed_player_id.saturating_add(1);
                warn!(
                    "dropping realtime input with spoofed player id: remote={:?} claimed={} bound={}",
                    remote_id.0, message.player_entity_id, bound_player_entity_id
                );
                continue;
            }
            let last_accepted_tick = input_tick_tracker
                .last_accepted_tick_by_player_entity_id
                .get(message.player_entity_id.as_str())
                .copied();
            match validate_input_message(&message, last_accepted_tick, now_s, &mut rate_limit_state)
            {
                Ok(()) => {}
                Err(InputValidationFailure::FutureTick) => {
                    input_drop_metrics.future_tick =
                        input_drop_metrics.future_tick.saturating_add(1);
                    continue;
                }
                Err(InputValidationFailure::DuplicateOrOutOfOrder) => {
                    input_drop_metrics.duplicate_or_out_of_order_tick = input_drop_metrics
                        .duplicate_or_out_of_order_tick
                        .saturating_add(1);
                    continue;
                }
                Err(InputValidationFailure::RateLimited) => {
                    input_drop_metrics.rate_limited =
                        input_drop_metrics.rate_limited.saturating_add(1);
                    continue;
                }
                Err(InputValidationFailure::OversizedPacket) => {
                    input_drop_metrics.oversized_packet =
                        input_drop_metrics.oversized_packet.saturating_add(1);
                    continue;
                }
            }
            let entry = latest
                .by_player_entity_id
                .entry(message.player_entity_id.clone())
                .or_insert(LatestRealtimeInput {
                    tick: 0,
                    controlled_entity_id: String::new(),
                    actions: Vec::new(),
                });
            if message.tick < entry.tick {
                continue;
            }
            input_tick_tracker
                .last_accepted_tick_by_player_entity_id
                .insert(message.player_entity_id.clone(), message.tick);
            entry.tick = message.tick;
            entry.controlled_entity_id = message.controlled_entity_id;
            entry.actions = message.actions;
        }
    }
}

#[allow(clippy::type_complexity)]
pub fn drain_native_player_inputs_to_action_queue(
    entities: Query<
        '_,
        '_,
        (
            Entity,
            &'_ EntityGuid,
            Option<&'_ SimulatedControlledEntity>,
            Option<&'_ PlayerTag>,
            Option<&'_ ControlledEntityGuid>,
            &'_ ActionState<PlayerInput>,
            &'_ mut ActionQueue,
        ),
        Without<lightyear::prelude::Confirmed<ActionState<PlayerInput>>>,
    >,
    time: Res<'_, Time>,
    controlled_entity_map: Res<'_, PlayerControlledEntityMap>,
    latest_realtime_inputs: Res<'_, LatestRealtimeInputsByPlayer>,
    mut input_drop_metrics: ResMut<'_, ClientInputDropMetrics>,
    mut input_log_state: ResMut<'_, InputActivityLogState>,
) {
    const ACTIVE_INPUT_LOG_INTERVAL_S: f64 = 0.15;
    for (entity, guid, simulated, player_tag, controlled_entity_guid, state, mut queue) in entities
    {
        if state.0.actions.is_empty() {
            continue;
        }
        if simulated.is_none() && player_tag.is_some() {
            let own_guid = guid.0.to_string();
            let controls_other_entity = controlled_entity_guid
                .and_then(|value| value.0.as_ref())
                .is_some_and(|target_guid| target_guid != &own_guid);
            if controls_other_entity {
                // Player anchors that currently control another entity should not consume
                // network input into their own ActionQueue; that queue is for local observer movement only.
                continue;
            }
        }
        let player_entity_id = simulated
            .map(|controlled| controlled.player_entity_id.clone())
            .or_else(|| player_tag.map(|_| format!("player:{}", guid.0)))
            .unwrap_or_else(|| format!("entity:{}", guid.0));
        if simulated.is_some() {
            let is_authoritative_target = controlled_entity_map
                .by_player_entity_id
                .get(player_entity_id.as_str())
                .is_some_and(|mapped| *mapped == entity);
            if !is_authoritative_target {
                continue;
            }
        }
        let controlled_entity_id = simulated
            .map(|controlled| controlled.entity_id.clone())
            .unwrap_or_else(|| player_entity_id.clone());
        let latest_realtime_actions = latest_realtime_inputs
            .by_player_entity_id
            .get(player_entity_id.as_str())
            .filter(|latest| latest.controlled_entity_id == controlled_entity_id)
            .map(|latest| latest.actions.as_slice());
        let actions = latest_realtime_actions.unwrap_or(state.0.actions.as_slice());
        if actions.is_empty() {
            continue;
        }
        // Server input should reflect the latest client intent snapshot for this tick.
        // Replacing (instead of appending) prevents stale-intent backlog under jitter/redundancy.
        queue.clear();
        for action in actions.iter().copied() {
            queue.push(action);
        }
        let accepted_tick = latest_realtime_inputs
            .by_player_entity_id
            .get(player_entity_id.as_str())
            .map(|latest| latest.tick)
            .unwrap_or(0);
        if input_debug_logging_enabled() {
            let now = time.elapsed_secs_f64();
            let last_logged_at_s = *input_log_state
                .last_logged_at_s_by_player_entity_id
                .get(player_entity_id.as_str())
                .unwrap_or(&f64::NEG_INFINITY);
            let time_due = now - last_logged_at_s >= ACTIVE_INPUT_LOG_INTERVAL_S;
            let actions_changed = input_log_state
                .last_logged_actions_by_player_entity_id
                .get(player_entity_id.as_str())
                .is_none_or(|last| last.as_slice() != actions);
            let should_log = time_due || actions_changed;
            if should_log {
                info!(
                    actions = ?actions,
                    accepted_tick = accepted_tick,
                    player = %player_entity_id,
                    controlled = %controlled_entity_id,
                    "server received input route"
                );
                input_log_state
                    .last_logged_at_s_by_player_entity_id
                    .insert(player_entity_id.clone(), now);
                input_log_state
                    .last_logged_actions_by_player_entity_id
                    .insert(player_entity_id.clone(), actions.to_vec());
            }
        }
        input_drop_metrics.record_accepted();
    }
}

#[allow(clippy::items_after_test_module)]
#[cfg(test)]
mod tests {
    use super::*;
    use sidereal_game::EntityAction;

    fn message_with(tick: u64, actions: usize) -> ClientRealtimeInputMessage {
        ClientRealtimeInputMessage {
            player_entity_id: "player:test".to_string(),
            controlled_entity_id: "ship:test".to_string(),
            actions: vec![EntityAction::ThrustNeutral; actions],
            tick,
        }
    }

    #[test]
    fn validation_rejects_duplicate_and_future_ticks() {
        let mut rate_limit = InputRateLimitState::default();
        let duplicate = message_with(10, 1);
        let future = message_with(20, 1);
        assert_eq!(
            validate_input_message(&duplicate, Some(10), 1.0, &mut rate_limit),
            Err(InputValidationFailure::DuplicateOrOutOfOrder)
        );
        assert_eq!(
            validate_input_message(&future, Some(10), 1.0, &mut rate_limit),
            Err(InputValidationFailure::FutureTick)
        );
    }

    #[test]
    fn validation_rejects_oversized_and_rate_limited() {
        let mut rate_limit = InputRateLimitState::default();
        let oversized = message_with(11, MAX_ACTIONS_PER_PACKET + 1);
        assert_eq!(
            validate_input_message(&oversized, Some(10), 1.0, &mut rate_limit),
            Err(InputValidationFailure::OversizedPacket)
        );

        let normal = message_with(11, 1);
        for _ in 0..MAX_MESSAGES_PER_SECOND {
            let result = validate_input_message(&normal, Some(10), 2.0, &mut rate_limit);
            assert_eq!(result, Ok(()));
        }
        assert_eq!(
            validate_input_message(&normal, Some(10), 2.0, &mut rate_limit),
            Err(InputValidationFailure::RateLimited)
        );
    }
}

pub fn report_input_drop_metrics(
    time: Res<'_, Time>,
    metrics: Res<'_, ClientInputDropMetrics>,
    mut state: ResMut<'_, ClientInputDropMetricsLogState>,
) {
    if !summary_logging_enabled() {
        return;
    }
    const LOG_INTERVAL_S: f64 = 5.0;
    let now = time.elapsed_secs_f64();
    let interval_s = now - state.last_logged_at_s;
    if interval_s < LOG_INTERVAL_S {
        return;
    }
    let accepted_delta = metrics
        .accepted_inputs
        .saturating_sub(state.last_accepted_inputs);
    let accepted_per_s = if interval_s > 0.0 {
        accepted_delta as f64 / interval_s
    } else {
        0.0
    };
    if accepted_delta == 0 && metrics.total_drops() == 0 {
        state.last_logged_at_s = now;
        return;
    }
    state.last_logged_at_s = now;
    state.last_accepted_inputs = metrics.accepted_inputs;
    info!(
        "replication input summary accepted={} accepted_per_s={:.1} drops_total={} future={} duplicate_or_out_of_order={} rate_limited={} oversized={} empty_after_filter={} unbound={} spoofed={}",
        accepted_delta,
        accepted_per_s,
        metrics.total_drops(),
        metrics.future_tick,
        metrics.duplicate_or_out_of_order_tick,
        metrics.rate_limited,
        metrics.oversized_packet,
        metrics.empty_after_filter,
        metrics.unbound_client,
        metrics.spoofed_player_id
    );
}
