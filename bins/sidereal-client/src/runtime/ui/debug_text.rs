pub(super) fn update_debug_overlay_text_ui_system(
    time: Res<'_, Time>,
    debug_overlay: Res<'_, DebugOverlayState>,
    snapshot: Res<'_, DebugOverlaySnapshot>,
    diagnostics: Res<'_, DiagnosticsStore>,
    input_send_state: Res<'_, ClientInputSendState>,
    mut display_metrics: Local<'_, DebugOverlayDisplayMetrics>,
    mut ui_queries: DebugOverlayTextUiQueries<'_, '_>,
) {
    let Ok(mut root_visibility) = ui_queries.root_query.single_mut() else {
        return;
    };

    if !debug_overlay.enabled {
        *root_visibility = Visibility::Hidden;
        return;
    }

    *root_visibility = Visibility::Visible;

    let now_s = time.elapsed_secs_f64();
    if !display_metrics.initialized || now_s - display_metrics.last_sample_at_s >= 1.0 {
        display_metrics.sampled_fps = diagnostics
            .get(&FrameTimeDiagnosticsPlugin::FPS)
            .and_then(|diagnostic| diagnostic.average().or_else(|| diagnostic.smoothed()));
        display_metrics.sampled_frame_ms = diagnostics
            .get(&FrameTimeDiagnosticsPlugin::FRAME_TIME)
            .and_then(|diagnostic| diagnostic.average().or_else(|| diagnostic.smoothed()));
        display_metrics.last_sample_at_s = now_s;
        display_metrics.initialized = true;
    }

    let mut header_row_pairs = Vec::with_capacity(2);
    header_row_pairs.push((
        "FPS".to_string(),
        display_metrics
            .sampled_fps
            .map(|value| format!("{value:.0}"))
            .unwrap_or_else(|| "--".to_string()),
    ));
    header_row_pairs.push((
        "Frame Time".to_string(),
        display_metrics
            .sampled_frame_ms
            .map(|value| format!("{value:.2} ms"))
            .unwrap_or_else(|| "--.-- ms".to_string()),
    ));
    let mut row_pairs = Vec::with_capacity(snapshot.text_rows.len() + 1);
    row_pairs.push((
        "Sent Input".to_string(),
        format_sent_input_actions(&input_send_state.last_sent_actions),
    ));
    for row in &snapshot.text_rows {
        row_pairs.push((
            row.label.clone(),
            truncate_debug_overlay_value(&row.value, DEBUG_OVERLAY_VALUE_MAX_CHARS),
        ));
    }
    let columns = split_debug_overlay_text_columns(&row_pairs);
    let header_labels_text = header_row_pairs
        .iter()
        .map(|(label, _)| label.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let header_values_text = header_row_pairs
        .iter()
        .map(|(_, value)| value.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let primary_labels_text = if columns[0].labels.is_empty() {
        header_labels_text.clone()
    } else {
        format!("{header_labels_text}\n{}", columns[0].labels.join("\n"))
    };
    let primary_values_text = if columns[0].values.is_empty() {
        header_values_text.clone()
    } else {
        format!("{header_values_text}\n{}", columns[0].values.join("\n"))
    };
    let secondary_labels_text = columns[1].labels.join("\n");
    let secondary_values_text = columns[1].values.join("\n");
    let tertiary_labels_text = columns[2].labels.join("\n");
    let tertiary_values_text = columns[2].values.join("\n");

    let debug_value_color = Color::srgb(0.85, 0.92, 1.0);
    for (
        mut text,
        color,
        primary_label,
        primary_label_shadow,
        primary_value,
        primary_value_shadow,
        secondary_label,
        secondary_label_shadow,
        secondary_value,
        secondary_value_shadow,
        tertiary_label,
        tertiary_label_shadow,
        tertiary_value,
        tertiary_value_shadow,
    ) in &mut ui_queries.text_query
    {
        if primary_label.is_some() || primary_label_shadow.is_some() {
            text.0 = primary_labels_text.clone();
        } else if primary_value.is_some() {
            text.0 = primary_values_text.clone();
            if let Some(mut color) = color {
                color.0 = debug_value_color;
            }
        } else if primary_value_shadow.is_some() {
            text.0 = primary_values_text.clone();
        } else if secondary_label.is_some() || secondary_label_shadow.is_some() {
            text.0 = secondary_labels_text.clone();
        } else if secondary_value.is_some() {
            text.0 = secondary_values_text.clone();
            if let Some(mut color) = color {
                color.0 = debug_value_color;
            }
        } else if secondary_value_shadow.is_some() {
            text.0 = secondary_values_text.clone();
        } else if tertiary_label.is_some() || tertiary_label_shadow.is_some() {
            text.0 = tertiary_labels_text.clone();
        } else if tertiary_value.is_some() {
            text.0 = tertiary_values_text.clone();
            if let Some(mut color) = color {
                color.0 = debug_value_color;
            }
        } else if tertiary_value_shadow.is_some() {
            text.0 = tertiary_values_text.clone();
        }
    }
}

fn format_sent_input_actions(actions: &[EntityAction]) -> String {
    if actions.is_empty() {
        return "[]".to_string();
    }

    let names: Vec<&'static str> = actions.iter().map(describe_entity_action).collect();
    let value = format!("[{}]", names.join(", "));
    truncate_debug_overlay_value(&value, DEBUG_OVERLAY_VALUE_MAX_CHARS)
}

fn truncate_debug_overlay_value(value: &str, max_chars: usize) -> String {
    let char_count = value.chars().count();
    if char_count <= max_chars {
        return value.to_string();
    }
    let keep = max_chars.saturating_sub(3);
    let truncated = value.chars().take(keep).collect::<String>();
    format!("{truncated}...")
}

fn describe_entity_action(action: &EntityAction) -> &'static str {
    match action {
        EntityAction::Forward => "Forward",
        EntityAction::Backward => "Backward",
        EntityAction::LongitudinalNeutral => "Long Neutral",
        EntityAction::Left => "Left",
        EntityAction::Right => "Right",
        EntityAction::LateralNeutral => "Turn Neutral",
        EntityAction::Brake => "Brake",
        EntityAction::AfterburnerOn => "Afterburner On",
        EntityAction::AfterburnerOff => "Afterburner Off",
        EntityAction::FirePrimary => "Fire Primary",
        EntityAction::FireSecondary => "Fire Secondary",
        EntityAction::ActivateShield => "Shield On",
        EntityAction::DeactivateShield => "Shield Off",
        EntityAction::ActivateTractor => "Tractor On",
        EntityAction::DeactivateTractor => "Tractor Off",
        EntityAction::ActivateScanner => "Scanner On",
        EntityAction::DeployCargo => "Deploy Cargo",
        EntityAction::EngageAutopilot => "Autopilot On",
        EntityAction::DisengageAutopilot => "Autopilot Off",
        EntityAction::InitiateDocking => "Dock",
    }
}

