use bevy::prelude::*;
use lightyear::prelude::input::native::ActionState;
use sidereal_game::{ActionQueue, EntityAction, EntityGuid, PlayerTag};
use sidereal_net::PlayerInput;
use std::collections::HashMap;
use std::sync::OnceLock;

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

#[derive(Resource, Debug, Default)]
pub struct InputActivityLogState {
    pub last_logged_at_s_by_player_entity_id: HashMap<String, f64>,
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

fn input_debug_logging_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("SIDEREAL_DEBUG_INPUT_LOGS")
            .is_ok_and(|v| v == "1" || v.eq_ignore_ascii_case("true"))
    })
}

#[allow(clippy::type_complexity)]
pub fn drain_native_player_inputs_to_action_queue(
    entities: Query<
        '_,
        '_,
        (
            &'_ EntityGuid,
            Option<&'_ SimulatedControlledEntity>,
            Option<&'_ PlayerTag>,
            &'_ ActionState<PlayerInput>,
            &'_ mut ActionQueue,
        ),
    >,
    time: Res<'_, Time>,
    mut input_tick_tracker: ResMut<'_, ClientInputTickTracker>,
    mut input_drop_metrics: ResMut<'_, ClientInputDropMetrics>,
    mut input_log_state: ResMut<'_, InputActivityLogState>,
) {
    const ACTIVE_INPUT_LOG_INTERVAL_S: f64 = 0.5;
    for (guid, simulated, player_tag, state, mut queue) in entities {
        if state.0.actions.is_empty() {
            continue;
        }
        let player_entity_id = simulated
            .map(|controlled| controlled.player_entity_id.clone())
            .or_else(|| player_tag.map(|_| format!("player:{}", guid.0)))
            .unwrap_or_else(|| format!("entity:{}", guid.0));
        let has_active = state.0.actions.iter().any(|a| {
            !matches!(
                a,
                EntityAction::ThrustNeutral
                    | EntityAction::YawNeutral
                    | EntityAction::LongitudinalNeutral
                    | EntityAction::LateralNeutral
            )
        });
        if has_active && input_debug_logging_enabled() {
            let now = time.elapsed_secs_f64();
            let last_logged = input_log_state
                .last_logged_at_s_by_player_entity_id
                .entry(player_entity_id.clone())
                .or_insert(f64::NEG_INFINITY);
            if now - *last_logged >= ACTIVE_INPUT_LOG_INTERVAL_S {
                info!(
                    player = %player_entity_id,
                    actions = ?state.0.actions,
                    "server received active input"
                );
                *last_logged = now;
            }
        }
        for action in state.0.actions.iter().copied() {
            queue.push(action);
        }
        let last_tick = input_tick_tracker
            .last_accepted_tick_by_player_entity_id
            .entry(player_entity_id)
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
