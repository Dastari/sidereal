pub(crate) fn toggle_debug_overlay_system(
    input: Res<'_, ButtonInput<KeyCode>>,
    dev_console_state: Option<Res<'_, DevConsoleState>>,
    mut debug_overlay: ResMut<'_, DebugOverlayState>,
) {
    if is_console_open(dev_console_state.as_deref()) {
        return;
    }
    if input.just_pressed(KeyCode::F3) {
        debug_overlay.enabled = !debug_overlay.enabled;
    }
}

pub(crate) fn debug_overlay_enabled(debug_overlay: Res<'_, DebugOverlayState>) -> bool {
    debug_overlay.enabled
}

pub(crate) fn count_fixed_update_runs_for_debug_diagnostics_system(
    mut diagnostics: ResMut<'_, RuntimeStallDiagnostics>,
) {
    diagnostics.fixed_runs_current_frame = diagnostics.fixed_runs_current_frame.saturating_add(1);
}

pub(crate) fn track_runtime_stall_diagnostics_system(
    real_time: Res<'_, Time<Real>>,
    fixed_time: Res<'_, Time<Fixed>>,
    windows: Query<'_, '_, &'_ Window, With<PrimaryWindow>>,
    mut diagnostics: ResMut<'_, RuntimeStallDiagnostics>,
) {
    let now_s = real_time.elapsed_secs_f64();
    let update_delta_ms = real_time.delta_secs_f64() * 1000.0;
    diagnostics.last_update_delta_ms = update_delta_ms;
    diagnostics.max_update_delta_ms = diagnostics.max_update_delta_ms.max(update_delta_ms);
    diagnostics.fixed_runs_last_frame = diagnostics.fixed_runs_current_frame;
    diagnostics.fixed_runs_max_frame = diagnostics
        .fixed_runs_max_frame
        .max(diagnostics.fixed_runs_last_frame);
    diagnostics.fixed_runs_current_frame = 0;
    diagnostics.fixed_overstep_ms = fixed_time.overstep().as_secs_f64() * 1000.0;

    let window_focused = windows
        .single()
        .map(|window| window.focused)
        .unwrap_or(true);
    if !diagnostics.focus_initialized {
        diagnostics.window_focused = window_focused;
        diagnostics.focus_initialized = true;
        diagnostics.last_focus_change_at_s = now_s;
    } else if diagnostics.window_focused != window_focused {
        diagnostics.window_focused = window_focused;
        diagnostics.focus_transitions = diagnostics.focus_transitions.saturating_add(1);
        diagnostics.last_focus_change_at_s = now_s;
    }

    if !window_focused {
        diagnostics.observed_unfocused_duration_s += real_time.delta_secs_f64();
        diagnostics.observed_unfocused_frames =
            diagnostics.observed_unfocused_frames.saturating_add(1);
    }

    if update_delta_ms >= DEBUG_STALL_GAP_THRESHOLD_MS {
        let estimated_ticks = (real_time.delta_secs_f64() * f64::from(SIM_TICK_HZ)).ceil() as u32;
        diagnostics.last_stall_gap_ms = update_delta_ms;
        diagnostics.last_stall_gap_estimated_ticks = estimated_ticks;
        if update_delta_ms > diagnostics.max_stall_gap_ms {
            diagnostics.max_stall_gap_ms = update_delta_ms;
            diagnostics.max_stall_gap_estimated_ticks = estimated_ticks;
        }
    }
}

