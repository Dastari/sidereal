use bevy::prelude::*;
use lightyear::prelude::input::native::ActionState;
use lightyear::prelude::server::ClientOf;
use lightyear::prelude::{MessageReceiver, RemoteId};
use sidereal_game::{ActionQueue, ControlledEntityGuid, EntityAction, EntityGuid, PlayerTag};
use sidereal_net::{ClientRealtimeInputMessage, PlayerInput};
use sidereal_net::{PlayerEntityId, RuntimeEntityId};
use std::collections::HashMap;

use crate::replication::auth::AuthenticatedClientBindings;
use crate::replication::lifecycle::ClientLastActivity;
use crate::replication::{PlayerControlledEntityMap, SimulatedControlledEntity, debug_env};

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
    pub controlled_target_mismatch: u64,
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
    pub controlled_entity_id: RuntimeEntityId,
    pub actions: Vec<EntityAction>,
}

#[derive(Resource, Default)]
pub struct LatestRealtimeInputsByPlayer {
    pub by_player_entity_id: HashMap<PlayerEntityId, LatestRealtimeInput>,
}

#[derive(Resource, Debug, Default)]
pub struct InputRateLimitState {
    pub current_window_index_by_player_entity_id: HashMap<String, u64>,
    pub message_count_in_window_by_player_entity_id: HashMap<String, u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InputValidationFailure {
    FutureTick,
    DuplicateOrOutOfOrder,
    RateLimited,
    OversizedPacket,
}

pub(crate) const MAX_ACTIONS_PER_PACKET: usize = 32;
pub fn init_resources(app: &mut App) {
    app.insert_resource(ClientInputTickTracker::default());
    app.insert_resource(ClientInputDropMetrics::default());
    app.insert_resource(ClientInputDropMetricsLogState::default());
    app.insert_resource(InputActivityLogState::default());
    app.insert_resource(InputRateLimitState::default());
    app.insert_resource(LatestRealtimeInputsByPlayer::default());
}

const MAX_TICKS_AHEAD: u64 = 6;
/// When true, accept the latest message even if its tick is far ahead of last accepted (skip-ahead).
/// Prevents input backlog when client sends faster than server drains (e.g. free-roam at high FPS).
const SKIP_AHEAD_ON_FUTURE_TICK: bool = true;
pub(crate) const MAX_MESSAGES_PER_SECOND: u32 = 120;

/// Canonical form for player entity id (bare UUID wire format).
pub(crate) fn canonical_player_entity_id(id: &str) -> String {
    PlayerEntityId::parse(id)
        .map(PlayerEntityId::canonical_wire_id)
        .unwrap_or_else(|| id.to_string())
}

fn runtime_ids_refer_to_same_guid(left: &str, right: &str) -> bool {
    if left == right {
        return true;
    }
    parse_runtime_entity_id(left)
        .zip(parse_runtime_entity_id(right))
        .is_some_and(|(l, r)| l == r)
}

fn parse_player_entity_id(id: &str) -> Option<PlayerEntityId> {
    PlayerEntityId::parse(id)
}

fn parse_runtime_entity_id(id: &str) -> Option<RuntimeEntityId> {
    RuntimeEntityId::parse(id)
}

pub(crate) fn canonical_controlled_entity_id(
    id: &str,
    player_entity_id: PlayerEntityId,
) -> Option<RuntimeEntityId> {
    if canonical_player_entity_id(id) == player_entity_id.canonical_wire_id() {
        return Some(RuntimeEntityId(player_entity_id.0));
    }
    parse_runtime_entity_id(id)
}

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

pub(crate) fn validate_input_message(
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
    let player_entity_id = canonical_player_entity_id(message.player_entity_id.as_str());
    let stored_window = rate_limit_state
        .current_window_index_by_player_entity_id
        .entry(player_entity_id.clone())
        .or_insert(window_index);
    if *stored_window != window_index {
        *stored_window = window_index;
        rate_limit_state
            .message_count_in_window_by_player_entity_id
            .insert(player_entity_id.clone(), 0);
    }
    let counter = rate_limit_state
        .message_count_in_window_by_player_entity_id
        .entry(player_entity_id)
        .or_insert(0);
    if *counter >= MAX_MESSAGES_PER_SECOND {
        return Err(InputValidationFailure::RateLimited);
    }
    *counter = counter.saturating_add(1);
    Ok(())
}

fn input_debug_logging_enabled() -> bool {
    debug_env("SIDEREAL_DEBUG_INPUT_LOGS")
}

fn summary_logging_enabled() -> bool {
    debug_env("SIDEREAL_REPLICATION_SUMMARY_LOGS")
}

/// Drains the receiver and applies only the latest input per player (by tick).
/// When the client sends many messages per server tick (e.g. free-roam at high FPS),
/// we discard older messages and apply only the newest, avoiding backlog and redundant work.
#[allow(clippy::type_complexity)]
#[allow(clippy::too_many_arguments)]
pub fn receive_latest_realtime_input_messages(
    time: Res<'_, Time<Real>>,
    mut last_activity: ResMut<'_, ClientLastActivity>,
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
        let Some(bound_player_id) = parse_player_entity_id(bound_player_entity_id.as_str()) else {
            warn!(
                "dropping realtime input: invalid bound player id for client {:?}: {}",
                client_entity, bound_player_entity_id
            );
            for _ in receiver.receive() {
                input_drop_metrics.unbound_client =
                    input_drop_metrics.unbound_client.saturating_add(1);
            }
            continue;
        };
        let bound_player_wire = bound_player_id.canonical_wire_id();
        let messages: Vec<ClientRealtimeInputMessage> = receiver.receive().collect();
        if messages.is_empty() {
            continue;
        }
        last_activity.0.insert(client_entity, now_s);

        // Keep only messages claiming this client's bound player; count spoofed.
        let mut valid_claims: Vec<ClientRealtimeInputMessage> = Vec::with_capacity(messages.len());
        for message in messages {
            let Some(claimed_player_id) = parse_player_entity_id(message.player_entity_id.as_str())
            else {
                input_drop_metrics.spoofed_player_id =
                    input_drop_metrics.spoofed_player_id.saturating_add(1);
                warn!(
                    "dropping realtime input with invalid claimed player id: remote={:?} claimed={} bound={}",
                    remote_id.0, message.player_entity_id, bound_player_wire
                );
                continue;
            };
            if claimed_player_id != bound_player_id {
                input_drop_metrics.spoofed_player_id =
                    input_drop_metrics.spoofed_player_id.saturating_add(1);
                warn!(
                    "dropping realtime input with spoofed player id: remote={:?} claimed={} bound={}",
                    remote_id.0, message.player_entity_id, bound_player_wire
                );
            } else {
                if message.player_entity_id != bound_player_wire {
                    warn!(
                        "realtime input invariant: canonical player id match but encoding differs claimed={} canonical={}",
                        message.player_entity_id, bound_player_wire
                    );
                }
                valid_claims.push(message);
            }
        }

        // Discard old inputs in favor of new: apply only the message with the highest tick.
        let Some(best) = valid_claims.into_iter().max_by_key(|m| m.tick) else {
            continue;
        };

        let last_accepted_tick = input_tick_tracker
            .last_accepted_tick_by_player_entity_id
            .get(bound_player_wire.as_str())
            .copied();
        match validate_input_message(&best, last_accepted_tick, now_s, &mut rate_limit_state) {
            Ok(()) => {}
            Err(InputValidationFailure::FutureTick) if !SKIP_AHEAD_ON_FUTURE_TICK => {
                input_drop_metrics.future_tick = input_drop_metrics.future_tick.saturating_add(1);
                continue;
            }
            Err(InputValidationFailure::FutureTick) => {
                // Skip-ahead: accept latest input anyway so we don't backlog when client sends fast.
            }
            Err(InputValidationFailure::DuplicateOrOutOfOrder) => {
                input_drop_metrics.duplicate_or_out_of_order_tick = input_drop_metrics
                    .duplicate_or_out_of_order_tick
                    .saturating_add(1);
                continue;
            }
            Err(InputValidationFailure::RateLimited) => {
                input_drop_metrics.rate_limited = input_drop_metrics.rate_limited.saturating_add(1);
                continue;
            }
            Err(InputValidationFailure::OversizedPacket) => {
                input_drop_metrics.oversized_packet =
                    input_drop_metrics.oversized_packet.saturating_add(1);
                continue;
            }
        }

        let entry =
            latest
                .by_player_entity_id
                .entry(bound_player_id)
                .or_insert(LatestRealtimeInput {
                    tick: 0,
                    controlled_entity_id: RuntimeEntityId(bound_player_id.0),
                    actions: Vec::new(),
                });
        if best.tick >= entry.tick {
            input_tick_tracker
                .last_accepted_tick_by_player_entity_id
                .insert(bound_player_wire.clone(), best.tick);
            let Some(controlled_id) =
                canonical_controlled_entity_id(&best.controlled_entity_id, bound_player_id)
            else {
                input_drop_metrics.empty_after_filter =
                    input_drop_metrics.empty_after_filter.saturating_add(1);
                warn!(
                    "dropping realtime input with invalid controlled entity id: player={} controlled_raw={}",
                    bound_player_wire, best.controlled_entity_id
                );
                continue;
            };
            if best.controlled_entity_id != controlled_id.to_string()
                && best.controlled_entity_id != bound_player_wire
            {
                warn!(
                    "realtime input invariant: controlled entity id encoding normalized raw={} canonical={}",
                    best.controlled_entity_id, controlled_id
                );
            }
            entry.tick = best.tick;
            entry.controlled_entity_id = controlled_id;
            entry.actions = best.actions;
            if input_debug_logging_enabled() {
                info!(
                    "replication received client input: player_entity_id={} controlled_entity_id={} tick={} actions={:?}",
                    bound_player_wire, entry.controlled_entity_id, entry.tick, entry.actions
                );
            }
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
    for (entity, guid, simulated, player_tag, controlled_entity_guid, mut queue) in entities {
        if simulated.is_none() && player_tag.is_some() {
            let own_guid = guid.0.to_string();
            let controls_other_entity = controlled_entity_guid
                .and_then(|value| value.0.as_ref())
                .is_some_and(|target_guid| !runtime_ids_refer_to_same_guid(target_guid, &own_guid));
            if controls_other_entity {
                // Player anchors that currently control another entity should not consume
                // network input into their own ActionQueue; that queue is for local observer movement only.
                continue;
            }
        }
        let player_entity_id_raw = simulated
            .map(|controlled| controlled.player_entity_id.canonical_wire_id())
            .or_else(|| player_tag.map(|_| guid.0.to_string()))
            .unwrap_or_else(|| format!("entity:{}", guid.0));
        let player_entity_id = canonical_player_entity_id(player_entity_id_raw.as_str());
        let Some(player_id) = parse_player_entity_id(player_entity_id.as_str()) else {
            continue;
        };
        let is_authoritative_target = controlled_entity_map
            .by_player_entity_id
            .get(&player_id)
            .is_some_and(|mapped| *mapped == entity);
        // Only the current authoritative control target for this player should consume
        // realtime input, whether it's a simulated ship or the free-roam player anchor.
        if (simulated.is_some() || player_tag.is_some()) && !is_authoritative_target {
            continue;
        }
        let controlled_entity_id = RuntimeEntityId(guid.0);
        let latest_for_player = latest_realtime_inputs.by_player_entity_id.get(&player_id);
        let (actions, action_source) = match latest_for_player {
            Some(latest) if latest.controlled_entity_id == controlled_entity_id => {
                (latest.actions.as_slice(), "realtime")
            }
            // Accept realtime input for the authoritative target even when the client's
            // controlled id encoding/routing is stale or mismatched. The server-side
            // authoritative control map already scoped this entity to the player.
            Some(latest) if is_authoritative_target => {
                input_drop_metrics.controlled_target_mismatch = input_drop_metrics
                    .controlled_target_mismatch
                    .saturating_add(1);
                (latest.actions.as_slice(), "realtime_mismatch_accepted")
            }
            // For non-simulated entities, keep strict target matching to avoid
            // accidentally applying player intent outside authoritative control flow.
            Some(_) => {
                input_drop_metrics.controlled_target_mismatch = input_drop_metrics
                    .controlled_target_mismatch
                    .saturating_add(1);
                (&[][..], "mismatch")
            }
            None => (&[][..], "no_realtime"),
        };
        if actions.is_empty() {
            // Latest snapshot has no actions; clear stale queue state.
            if !queue.pending.is_empty() {
                queue.clear();
            }
            if input_debug_logging_enabled()
                && latest_realtime_inputs
                    .by_player_entity_id
                    .contains_key(&player_id)
            {
                info!(
                    player = %player_entity_id,
                    controlled = %controlled_entity_id,
                    "server input route has no actions after realtime selection"
                );
            }
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
            .get(&player_id)
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
                    entity = ?entity,
                    guid = %guid.0,
                    actions = ?actions,
                    accepted_tick = accepted_tick,
                    source = action_source,
                    player = %player_entity_id,
                    controlled = %controlled_entity_id,
                    "server applied input to action queue"
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
        "replication input summary accepted={} accepted_per_s={:.1} drops_total={} future={} duplicate_or_out_of_order={} rate_limited={} oversized={} empty_after_filter={} unbound={} spoofed={} controlled_target_mismatch={}",
        accepted_delta,
        accepted_per_s,
        metrics.total_drops(),
        metrics.future_tick,
        metrics.duplicate_or_out_of_order_tick,
        metrics.rate_limited,
        metrics.oversized_packet,
        metrics.empty_after_filter,
        metrics.unbound_client,
        metrics.spoofed_player_id,
        metrics.controlled_target_mismatch
    );
}
