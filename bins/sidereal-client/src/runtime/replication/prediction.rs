pub(crate) fn configure_prediction_manager_tuning(
    tuning: Res<'_, PredictionCorrectionTuning>,
    mut managers: Query<
        '_,
        '_,
        (Entity, &mut PredictionManager, Has<Client>),
        Added<PredictionManager>,
    >,
) {
    for (entity, mut manager, has_client_marker) in &mut managers {
        manager.rollback_policy.max_rollback_ticks = tuning.max_rollback_ticks;
        manager.rollback_policy.state = match tuning.rollback_state {
            PredictionRollbackStateTuning::Always => RollbackMode::Always,
            PredictionRollbackStateTuning::Check => RollbackMode::Check,
            PredictionRollbackStateTuning::Disabled => RollbackMode::Disabled,
        };
        // Sidereal keeps Lightyear client-side input history for prediction replay, but the
        // authoritative server input path is ClientRealtimeInputMessage, not Lightyear's native
        // server input receiver. Leaving Lightyear input rollback enabled lets native-input tracker
        // state trigger Rollback::FromInputs against local prediction history that the server did
        // not authoritatively confirm, which can snap controlled ships back to stale poses. Server
        // state rollback remains enabled above and is the authoritative reconciliation lane. The
        // default state mode is Always so dynamic control handoff cannot miss a correction because
        // local prediction history was stale or reseeded during the role transition.
        manager.rollback_policy.input = RollbackMode::Disabled;
        manager.correction_policy = if tuning.instant_correction {
            CorrectionPolicy::instant_correction()
        } else {
            CorrectionPolicy::default()
        };
        bevy::log::info!(
            "configured prediction manager entity={} has_client_marker={} (rollback_state={:?}, input_rollback_state={:?}, max_rollback_ticks={}, correction_mode={})",
            entity,
            has_client_marker,
            manager.rollback_policy.state,
            manager.rollback_policy.input,
            tuning.max_rollback_ticks,
            if tuning.instant_correction {
                "instant"
            } else {
                "smooth"
            }
        );
    }
}
