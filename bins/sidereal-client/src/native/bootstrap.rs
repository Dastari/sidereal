//! Bootstrap watchdog: arm on enter in-world, optional failure watch.

use bevy::log::{info, warn};
use bevy::prelude::*;

use super::app_state::ClientSession;
use super::assets::LocalAssetManager;
use super::dialog_ui;
use super::resources::{
    BootstrapWatchdogState, ClientAuthSyncState, DeferredPredictedAdoptionState,
    PredictionBootstrapTuning,
};

pub fn reset_bootstrap_watchdog_on_enter_in_world(
    time: Res<'_, Time>,
    mut watchdog: ResMut<'_, BootstrapWatchdogState>,
) {
    info!("client entered in-world state; bootstrap watchdog armed");
    *watchdog = BootstrapWatchdogState {
        in_world_entered_at_s: Some(time.elapsed_secs_f64()),
        last_bootstrap_progress_at_s: time.elapsed_secs_f64(),
        ..Default::default()
    };
}

#[allow(clippy::too_many_arguments)]
pub(super) fn watch_in_world_bootstrap_failures(
    time: Res<'_, Time>,
    tuning: Res<'_, PredictionBootstrapTuning>,
    auth_state: Res<'_, ClientAuthSyncState>,
    mut session: ResMut<'_, ClientSession>,
    mut asset_manager: ResMut<'_, LocalAssetManager>,
    mut watchdog: ResMut<'_, BootstrapWatchdogState>,
    mut adoption_state: ResMut<'_, DeferredPredictedAdoptionState>,
    mut dialog_queue: ResMut<'_, dialog_ui::DialogQueue>,
    replicated_entities: Query<'_, '_, Entity, With<lightyear::prelude::Replicated>>,
) {
    let now = time.elapsed_secs_f64();
    if watchdog.in_world_entered_at_s.is_none() {
        watchdog.in_world_entered_at_s = Some(now);
        watchdog.last_bootstrap_progress_at_s = now;
    }

    if asset_manager.bootstrap_ready_bytes != watchdog.last_bootstrap_ready_bytes {
        watchdog.last_bootstrap_ready_bytes = asset_manager.bootstrap_ready_bytes;
        watchdog.last_bootstrap_progress_at_s = now;
    }

    let entered_at = watchdog.in_world_entered_at_s.unwrap_or(now);
    if !watchdog.replication_state_seen && !replicated_entities.is_empty() {
        watchdog.replication_state_seen = true;
    }
    let auth_bind_sent = !auth_state.sent_for_client_entities.is_empty();
    if !watchdog.timeout_dialog_shown
        && now - entered_at > 3.0
        && !asset_manager.bootstrap_manifest_seen
        && !watchdog.replication_state_seen
    {
        warn!(
            "client bootstrap timeout waiting for manifest/auth bind (auth_bind_sent={} replication_seen={} manifest_seen={})",
            auth_bind_sent, watchdog.replication_state_seen, watchdog.asset_manifest_seen
        );
        session.status = "World bootstrap timed out. Check error dialog.".to_string();
        session.ui_dirty = true;
        dialog_queue.push_error(
            "World Bootstrap Timeout",
            format!(
                "Connected to transport, but world bootstrap did not begin within 3 seconds.\n\n\
                 Diagnostics:\n\
                 - Auth bind sent: {}\n\
                 - Replication state received: {}\n\
                 - Asset manifest received: {}\n\n\
                 Likely causes:\n\
                 - Replication rejected client auth bind (JWT mismatch/missing secret)\n\
                 - Replication auth/visibility flow not bound for this player\n\n\
                 Check replication logs for: 'replication client authenticated and bound'.",
                if auth_bind_sent { "yes" } else { "no" },
                if watchdog.replication_state_seen {
                    "yes"
                } else {
                    "no"
                },
                if watchdog.asset_manifest_seen {
                    "yes"
                } else {
                    "no"
                },
            ),
        );
        watchdog.timeout_dialog_shown = true;
        if watchdog.replication_state_seen && !asset_manager.bootstrap_phase_complete {
            warn!(
                "forcing bootstrap completion in degraded mode after timeout (replication active, no manifest)"
            );
            asset_manager.bootstrap_phase_complete = true;
            session.status =
                "Replication active without manifest; continuing in degraded bootstrap mode."
                    .to_string();
            session.ui_dirty = true;
        }
    }

    if !watchdog.no_world_state_dialog_shown
        && asset_manager.bootstrap_complete()
        && !watchdog.replication_state_seen
        && now - entered_at > 10.0
    {
        warn!(
            "client bootstrap completed but no replication world state received (auth_bind_sent={} manifest_seen={})",
            auth_bind_sent, watchdog.asset_manifest_seen
        );
        session.status = "No world state received. Check error dialog.".to_string();
        session.ui_dirty = true;
        dialog_queue.push_error(
            "No World State Received",
            "Asset bootstrap completed, but no replication world state updates arrived.\n\n\
             Most likely cause: gateway bootstrap dispatch is not notifying live replication simulation.\n\
             Ensure gateway uses UDP bootstrap handoff (`GATEWAY_BOOTSTRAP_MODE=udp`) and restart gateway + replication."
                .to_string(),
        );
        watchdog.no_world_state_dialog_shown = true;
    }

    if !adoption_state.dialog_shown
        && watchdog.replication_state_seen
        && adoption_state.waiting_entity_id.is_some()
        && adoption_state
            .wait_started_at_s
            .is_some_and(|started_at_s| now - started_at_s > tuning.defer_dialog_after_s)
    {
        let wait_s = adoption_state
            .wait_started_at_s
            .map(|started_at_s| (now - started_at_s).max(0.0))
            .unwrap_or_default();
        let waiting_entity = adoption_state
            .waiting_entity_id
            .as_deref()
            .unwrap_or("<unknown>");
        warn!(
            "controlled predicted adoption stalled for {} (wait {:.2}s, missing: {})",
            waiting_entity, wait_s, adoption_state.last_missing_components
        );
        session.status = "Controlled entity adoption delayed. Check warning dialog.".to_string();
        session.ui_dirty = true;
        dialog_queue.push_warning(
            "Controlled Entity Adoption Delayed",
            format!(
                "Replication is active, but the controlled predicted entity is still waiting for required replicated Avian components.\n\n\
                 Entity: {}\n\
                 Wait time: {:.1}s\n\
                 Missing: {}\n\n\
                 This usually means component replication for the controlled entity is arriving out-of-order under load.",
                waiting_entity,
                wait_s,
                adoption_state.last_missing_components
            ),
        );
        adoption_state.dialog_shown = true;
    }

    if asset_manager.bootstrap_complete() {
        return;
    }

    if !watchdog.stream_stall_dialog_shown
        && asset_manager.bootstrap_manifest_seen
        && !asset_manager.pending_assets.is_empty()
        && now - watchdog.last_bootstrap_progress_at_s > 6.0
    {
        warn!(
            "client bootstrap stream stalled (ready_bytes={} total_bytes={} pending_assets={})",
            asset_manager.bootstrap_ready_bytes,
            asset_manager.bootstrap_total_bytes,
            asset_manager.pending_assets.len()
        );
        session.status = "Asset streaming stalled. Check error dialog.".to_string();
        session.ui_dirty = true;
        dialog_queue.push_error(
            "Asset Streaming Stalled",
            format!(
                "Received asset manifest, but bootstrap download progress has not changed for 6 seconds.\n\n\
                 Diagnostics:\n\
                 - Bootstrap ready bytes: {}\n\
                 - Bootstrap total bytes: {}\n\
                 - Pending assets: {}\n\n\
                 Check replication asset stream logs for chunk send/request/ack activity.",
                asset_manager.bootstrap_ready_bytes,
                asset_manager.bootstrap_total_bytes,
                asset_manager.pending_assets.len(),
            ),
        );
        watchdog.stream_stall_dialog_shown = true;
        if !asset_manager.bootstrap_phase_complete {
            warn!("forcing bootstrap completion in degraded mode after asset stream stall");
            asset_manager.bootstrap_phase_complete = true;
            session.status =
                "Asset bootstrap stalled; continuing in degraded mode while streaming retries."
                    .to_string();
            session.ui_dirty = true;
        }
    }
}
