pub(super) fn update_runtime_stream_icon_system(
    time: Res<'_, Time>,
    fetch_state: Res<'_, RuntimeAssetHttpFetchState>,
    mut indicator_state: ResMut<'_, RuntimeAssetNetIndicatorState>,
    mut text_query: Query<
        '_,
        '_,
        &mut TextColor,
        With<super::components::RuntimeStreamingIconText>,
    >,
) {
    let Ok(mut color) = text_query.single_mut() else {
        return;
    };
    if !fetch_state.has_in_flight_fetch() {
        color.0.set_alpha(0.0);
        indicator_state.blinking_phase_s = 0.0;
        return;
    }
    indicator_state.blinking_phase_s += time.delta_secs();
    let phase = (indicator_state.blinking_phase_s * 6.0).fract();
    let on = phase < 0.5;
    color.0 = if on {
        Color::srgba(0.35, 0.9, 1.0, 1.0)
    } else {
        Color::srgba(0.35, 0.9, 1.0, 0.2)
    };
}

