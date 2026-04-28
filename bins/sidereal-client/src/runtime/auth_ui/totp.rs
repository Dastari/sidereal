#[allow(clippy::type_complexity)]
fn update_auth_text(
    session: Res<'_, ClientSession>,
    active_theme: Res<'_, ActiveUiTheme>,
    mut text_sets: ParamSet<
        '_,
        '_,
        (
            Query<'_, '_, (&AuthUiStatusText, &mut Text, &mut TextColor)>,
            Query<'_, '_, &mut Text, With<AuthUiFlowTitle>>,
            Query<'_, '_, &mut Text, With<AuthUiSubmitLabel>>,
            Query<'_, '_, (&AuthUiStatusFrame, &mut Node), With<AuthUiStatusFrame>>,
            Query<'_, '_, (&AuthUiStatusTitle, &mut Text, &mut TextColor)>,
            Query<'_, '_, (&AuthUiStatusIconSlot, &mut Node)>,
        ),
    >,
) {
    let theme = theme_definition(active_theme.0);
    let flow_title = flow_title(&session);

    for mut text in &mut text_sets.p1() {
        text.0 = flow_title.to_string();
    }

    let submit_label = submit_label(&session);
    for mut text in &mut text_sets.p2() {
        text.0 = submit_label.to_ascii_uppercase();
    }

    let status = session.status.trim();
    let status_tone = auth_status_tone(status);

    for (frame, mut node) in &mut text_sets.p3() {
        node.display = if status_tone == Some(frame.tone) {
            Display::Flex
        } else {
            Display::None
        };
    }

    for (icon, mut node) in &mut text_sets.p5() {
        node.display = if status_tone == Some(icon.tone)
            && matches!(icon.tone, UiSemanticTone::Danger | UiSemanticTone::Warning)
        {
            Display::Flex
        } else {
            Display::None
        };
    }

    for (title, mut text, mut color) in &mut text_sets.p4() {
        text.0 = auth_status_title(status, title.tone).to_string();
        *color = TextColor(title.tone.foreground_color(theme));
    }

    for (status_text, mut text, mut color) in &mut text_sets.p0() {
        text.0 = status.to_string();
        *color = TextColor(status_text.tone.foreground_color(theme));
    }
}

fn update_auth_field_layout(
    session: Res<'_, ClientSession>,
    mut field_containers: Query<'_, '_, (&AuthUiFieldContainer, &mut Visibility)>,
) {
    for (container, mut visibility) in &mut field_containers {
        *visibility = if is_field_visible(&session, container.field) {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

#[allow(clippy::type_complexity)]
fn update_auth_field_content(
    session: Res<'_, ClientSession>,
    input_state: Res<'_, AuthReusableInputState>,
    password_display: Res<'_, AuthPasswordDisplayState>,
    blink: Res<'_, CursorBlink>,
    totp_cursor: Res<'_, TotpInputCursor>,
    mut field_queries: ParamSet<
        '_,
        '_,
        (
            Query<'_, '_, (&AuthUiInputText, &mut Text)>,
            Query<'_, '_, (&AuthUiTotpDigitText, &mut Text)>,
            Query<'_, '_, (&AuthUiCursor, &mut Node)>,
            Query<'_, '_, (&AuthUiTotpDigitCursor, &mut Visibility)>,
            Query<'_, '_, (&AuthUiSelectionText, &mut Text)>,
            Query<'_, '_, (&AuthUiSelectionBox, &mut Node)>,
        ),
    >,
) {
    for (input, mut text) in &mut field_queries.p0() {
        let Some(state) = active_text_input(&input_state, input.field) else {
            continue;
        };
        let segments = state.display_segments(display_kind_for_field(
            input.field,
            input.kind,
            &password_display,
        ));
        text.0 = match input.segment {
            AuthInputTextSegment::BeforeSelection => segments.before_selection,
            AuthInputTextSegment::AfterSelection => segments.after_selection,
        };
    }

    for (cursor, mut node) in &mut field_queries.p2() {
        let Some(state) = active_text_input(&input_state, cursor.field) else {
            node.display = Display::None;
            continue;
        };
        let segments = state.display_segments(display_kind_for_field(
            cursor.field,
            input_kind(cursor.field),
            &password_display,
        ));
        let edge_visible = match cursor.edge {
            AuthInputCursorEdge::SelectionStart => segments.caret_at_selection_start,
            AuthInputCursorEdge::SelectionEnd => !segments.caret_at_selection_start,
        };
        let visible = edge_visible
            && blink.visible
            && session.focus == cursor.field
            && is_field_visible(&session, cursor.field);
        node.display = if visible {
            Display::Flex
        } else {
            Display::None
        };
    }

    for (selection, mut text) in &mut field_queries.p4() {
        let Some(state) = active_text_input(&input_state, selection.field) else {
            continue;
        };
        text.0 = state
            .display_segments(display_kind_for_field(
                selection.field,
                input_kind(selection.field),
                &password_display,
            ))
            .selected;
    }

    for (selection, mut node) in &mut field_queries.p5() {
        let visible = active_text_input(&input_state, selection.field).is_some_and(|state| {
            state.has_selection()
                && session.focus == selection.field
                && is_field_visible(&session, selection.field)
        });
        node.display = if visible {
            Display::Flex
        } else {
            Display::None
        };
    }

    let totp_digits = normalize_totp_code(&session.totp_code);
    for (digit, mut text) in &mut field_queries.p1() {
        text.0 = totp_digits
            .chars()
            .nth(digit.index)
            .map(|value| value.to_string())
            .unwrap_or_default();
    }

    for (cursor, mut visibility) in &mut field_queries.p3() {
        let visible = blink.visible
            && session.focus == FocusField::TotpCode
            && is_field_visible(&session, FocusField::TotpCode)
            && cursor.index == totp_cursor.index;
        *visibility = if visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

fn normalize_totp_code(raw: &str) -> String {
    raw.chars()
        .filter(|value| value.is_ascii_digit())
        .take(TOTP_CODE_LENGTH)
        .collect()
}

fn next_totp_cursor_index(code: &str) -> usize {
    normalize_totp_code(code)
        .len()
        .min(TOTP_CODE_LENGTH.saturating_sub(1))
}

fn insert_totp_digits(code: &mut String, cursor: &mut TotpInputCursor, raw: &str) {
    let inserted = normalize_totp_code(raw);
    if inserted.is_empty() {
        return;
    }

    let mut digits: Vec<char> = normalize_totp_code(code).chars().collect();
    digits.resize(TOTP_CODE_LENGTH, '\0');
    let mut index = cursor.index.min(TOTP_CODE_LENGTH - 1);
    for digit in inserted.chars() {
        digits[index] = digit;
        if index >= TOTP_CODE_LENGTH - 1 {
            break;
        }
        index += 1;
    }
    *code = digits.into_iter().filter(|digit| *digit != '\0').collect();
    cursor.index = index.min(TOTP_CODE_LENGTH - 1);
}

fn handle_totp_backspace(code: &mut String, cursor: &mut TotpInputCursor) {
    let mut digits: Vec<char> = normalize_totp_code(code).chars().collect();
    if digits.is_empty() {
        cursor.index = 0;
        return;
    }

    let active_index = cursor.index.min(TOTP_CODE_LENGTH - 1);
    let remove_index = if active_index < digits.len() {
        active_index
    } else {
        digits.len().saturating_sub(1)
    };
    digits.remove(remove_index);
    *code = digits.into_iter().collect();
    cursor.index = remove_index.saturating_sub(usize::from(remove_index > 0));
}

