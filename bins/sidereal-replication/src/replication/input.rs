use bevy::prelude::*;
use lightyear::prelude::input::native::ActionState;
use sidereal_game::{ActionQueue, EntityAction, is_flight_control_action};
use sidereal_net::PlayerInput;
use std::collections::HashMap;

use crate::replication::SimulatedControlledEntity;

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

impl ClientInputDropMetrics {
    fn record_accepted(&mut self) {
        self.accepted_inputs = self.accepted_inputs.saturating_add(1);
    }

    fn record(&mut self, reason: InputDropReason) {
        match reason {
            InputDropReason::EmptyAfterFilter => {
                self.empty_after_filter = self.empty_after_filter.saturating_add(1);
            }
        }
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

#[derive(Debug, Clone, Copy)]
enum InputDropReason {
    EmptyAfterFilter,
}

fn is_allowed_control_action(action: EntityAction) -> bool {
    is_flight_control_action(action)
}

pub fn drain_native_player_inputs_to_action_queue(
    entities: Query<
        '_,
        '_,
        (
            &'_ SimulatedControlledEntity,
            &'_ ActionState<PlayerInput>,
            &'_ mut ActionQueue,
        ),
    >,
    mut input_tick_tracker: ResMut<'_, ClientInputTickTracker>,
    mut input_drop_metrics: ResMut<'_, ClientInputDropMetrics>,
) {
    for (controlled, state, mut queue) in entities {
        let allowed_actions = state
            .0
            .actions
            .iter()
            .copied()
            .filter(|action| is_allowed_control_action(*action))
            .collect::<Vec<_>>();
        if allowed_actions.is_empty() {
            if !state.0.actions.is_empty() {
                input_drop_metrics.record(InputDropReason::EmptyAfterFilter);
            }
            continue;
        }
        let has_active = allowed_actions
            .iter()
            .any(|a| !matches!(a, EntityAction::ThrustNeutral | EntityAction::YawNeutral));
        if has_active {
            info!(
                player = %controlled.player_entity_id,
                actions = ?allowed_actions,
                "server received active input"
            );
        }
        for action in allowed_actions {
            queue.push(action);
        }
        let last_tick = input_tick_tracker
            .last_accepted_tick_by_player_entity_id
            .entry(controlled.player_entity_id.clone())
            .or_insert(0);
        *last_tick = last_tick.saturating_add(1);
        input_drop_metrics.record_accepted();
    }
}

pub fn report_input_drop_metrics(
    time: Res<'_, Time>,
    metrics: Res<'_, ClientInputDropMetrics>,
    mut state: ResMut<'_, ClientInputDropMetricsLogState>,
) {
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
