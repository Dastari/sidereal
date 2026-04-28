fn animate_auth_background(
    time: Res<'_, Time>,
    active_theme: Res<'_, ActiveUiTheme>,
    mut bg_query: Query<'_, '_, &mut BackgroundColor, With<AuthUiBackdrop>>,
) {
    let theme = theme_definition(active_theme.0);
    let t = time.elapsed_secs();
    let pulse = 0.75 + 0.25 * (t * 0.5).sin().abs();
    for mut color in &mut bg_query {
        let base = theme.colors.background;
        *color = BackgroundColor(Color::from(base.with_lightness(base.lightness * pulse)));
    }
}

fn tick_cursor_blink(time: Res<'_, Time>, mut blink: ResMut<'_, CursorBlink>) {
    blink.timer.tick(time.delta());
    if blink.timer.just_finished() {
        blink.visible = !blink.visible;
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_auth_keyboard_input(
    mut keyboard_input_reader: MessageReader<'_, '_, KeyboardInput>,
    keys: Res<'_, ButtonInput<KeyCode>>,
    dev_console_state: Option<Res<'_, DevConsoleState>>,
    mut session: ResMut<'_, ClientSession>,
    mut input_state: ResMut<'_, AuthReusableInputState>,
    mut totp_cursor: ResMut<'_, TotpInputCursor>,
    mut request_state: ResMut<'_, super::auth_net::GatewayRequestState>,
    gateway_http: Res<'_, GatewayHttpAdapter>,
) {
    if super::dev_console::is_console_open(dev_console_state.as_deref()) {
        return;
    }
    let mut submit = false;
    for event in keyboard_input_reader.read() {
        if event.state != ButtonState::Pressed {
            continue;
        }

        match &event.logical_key {
            Key::F1 => {
                session.selected_action = AuthAction::Login;
                session.focus = FocusField::Email;
                session.totp_challenge_id = None;
                session.totp_code.clear();
                let email_end = input_state.email.text.len();
                input_state.email.set_cursor(email_end);
                totp_cursor.index = 0;
                session.ui_dirty = true;
            }
            Key::Tab => {
                if keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight) {
                    session.focus = previous_focus_field(&session, session.focus);
                } else {
                    session.focus = next_focus_field(&session, session.focus);
                }
                if session.focus == FocusField::TotpCode {
                    totp_cursor.index = next_totp_cursor_index(&session.totp_code);
                }
                session.ui_dirty = true;
            }
            Key::Enter => {
                submit = true;
            }
            Key::Backspace => {
                if session.focus == FocusField::TotpCode {
                    handle_totp_backspace(&mut session.totp_code, &mut totp_cursor);
                } else {
                    let delete = if command_modifier(&keys) && cfg!(target_os = "macos") {
                        TextInputDelete::ToStart
                    } else if word_modifier(&keys) {
                        TextInputDelete::PreviousWord
                    } else {
                        TextInputDelete::PreviousGrapheme
                    };
                    if let Some(input) = active_text_input_mut(&mut input_state, session.focus) {
                        input.delete(delete);
                    }
                }
                session.ui_dirty = true;
            }
            Key::Delete => {
                if let Some(input) = active_text_input_mut(&mut input_state, session.focus) {
                    let delete = if word_modifier(&keys) {
                        TextInputDelete::NextWord
                    } else {
                        TextInputDelete::NextGrapheme
                    };
                    input.delete(delete);
                    sync_session_text_inputs(&mut session, &input_state);
                    session.ui_dirty = true;
                }
            }
            Key::ArrowLeft if session.focus == FocusField::TotpCode => {
                totp_cursor.index = totp_cursor.index.saturating_sub(1);
                session.ui_dirty = true;
            }
            Key::ArrowRight if session.focus == FocusField::TotpCode => {
                totp_cursor.index = (totp_cursor.index.saturating_add(1)).min(TOTP_CODE_LENGTH - 1);
                session.ui_dirty = true;
            }
            Key::ArrowLeft
            | Key::ArrowRight
            | Key::Home
            | Key::End
            | Key::ArrowUp
            | Key::ArrowDown
                if session.focus != FocusField::TotpCode =>
            {
                if let Some(input) = active_text_input_mut(&mut input_state, session.focus) {
                    let extend =
                        keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
                    let movement = match &event.logical_key {
                        Key::ArrowLeft if word_modifier(&keys) => TextInputMovement::PreviousWord,
                        Key::ArrowLeft => TextInputMovement::PreviousGrapheme,
                        Key::ArrowRight if word_modifier(&keys) => TextInputMovement::NextWord,
                        Key::ArrowRight => TextInputMovement::NextGrapheme,
                        Key::Home | Key::ArrowUp => TextInputMovement::Start,
                        Key::End | Key::ArrowDown => TextInputMovement::End,
                        _ => TextInputMovement::End,
                    };
                    input.move_cursor(movement, extend);
                    session.ui_dirty = true;
                }
            }
            Key::Character(_) if primary_modifier(&keys) => {
                if session.focus == FocusField::TotpCode {
                    handle_totp_shortcut(
                        event.key_code,
                        &keys,
                        &mut input_state,
                        &mut session,
                        &mut totp_cursor,
                    );
                } else {
                    handle_text_input_shortcut(
                        event.key_code,
                        &keys,
                        &mut input_state,
                        &mut session,
                    );
                }
            }
            Key::Copy if session.focus != FocusField::TotpCode => {
                copy_active_selection(&mut input_state, session.focus);
            }
            Key::Cut if session.focus != FocusField::TotpCode => {
                cut_active_selection(&mut input_state, session.focus);
                sync_session_text_inputs(&mut session, &input_state);
                session.ui_dirty = true;
            }
            Key::Paste => {
                if session.focus == FocusField::TotpCode {
                    paste_into_totp(
                        &mut input_state,
                        &mut session.totp_code,
                        &mut totp_cursor,
                    );
                } else {
                    paste_into_active_input(&mut input_state, session.focus);
                    sync_session_text_inputs(&mut session, &input_state);
                }
                session.ui_dirty = true;
            }
            Key::Undo if session.focus != FocusField::TotpCode => {
                if let Some(input) = active_text_input_mut(&mut input_state, session.focus) {
                    input.undo();
                }
                sync_session_text_inputs(&mut session, &input_state);
                session.ui_dirty = true;
            }
            Key::Redo if session.focus != FocusField::TotpCode => {
                if let Some(input) = active_text_input_mut(&mut input_state, session.focus) {
                    input.redo();
                }
                sync_session_text_inputs(&mut session, &input_state);
                session.ui_dirty = true;
            }
            Key::Insert => {
                if keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight) {
                    if session.focus == FocusField::TotpCode {
                        paste_into_totp(
                            &mut input_state,
                            &mut session.totp_code,
                            &mut totp_cursor,
                        );
                    } else {
                        paste_into_active_input(&mut input_state, session.focus);
                        sync_session_text_inputs(&mut session, &input_state);
                    }
                    session.ui_dirty = true;
                } else if control_modifier(&keys) && session.focus != FocusField::TotpCode {
                    copy_active_selection(&mut input_state, session.focus);
                }
            }
            _ => {
                if let Some(inserted_text) = &event.text
                    && inserted_text.chars().all(is_printable_char)
                    && !control_modifier(&keys)
                    && !command_modifier(&keys)
                {
                    if session.focus == FocusField::TotpCode {
                        insert_totp_digits(&mut session.totp_code, &mut totp_cursor, inserted_text);
                    } else {
                        if let Some(input) = active_text_input_mut(&mut input_state, session.focus)
                        {
                            input.insert_text(inserted_text);
                        }
                        sync_session_text_inputs(&mut session, &input_state);
                    }
                    session.ui_dirty = true;
                }
            }
        }
        if session.focus != FocusField::TotpCode {
            sync_session_text_inputs(&mut session, &input_state);
        }
    }

    if keys.just_pressed(KeyCode::Enter) {
        submit = true;
    }

    if submit {
        submit_auth_request(&mut session, request_state.as_mut(), *gateway_http);
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_auth_input_pointer(
    time: Res<'_, Time>,
    mouse: Res<'_, ButtonInput<MouseButton>>,
    keys: Res<'_, ButtonInput<KeyCode>>,
    mut pointer_state: ResMut<'_, AuthInputPointerState>,
    mut session: ResMut<'_, ClientSession>,
    mut input_state: ResMut<'_, AuthReusableInputState>,
    text_slots: Query<'_, '_, (&AuthUiInputTextSlot, &RelativeCursorPosition, &ComputedNode)>,
    input_text_nodes: Query<'_, '_, (&AuthUiInputText, &ComputedNode)>,
    selection_text_nodes: Query<'_, '_, (&AuthUiSelectionText, &ComputedNode)>,
) {
    if mouse.just_released(MouseButton::Left) {
        pointer_state.dragging = None;
    }

    if mouse.just_pressed(MouseButton::Left) {
        for (text_slot, cursor_position, slot_node) in &text_slots {
            if !cursor_position.cursor_over {
                continue;
            }

            let fraction = pointer_text_fraction(
                text_slot.field,
                cursor_position,
                slot_node,
                &input_text_nodes,
                &selection_text_nodes,
            );
            session.focus = text_slot.field;
            let extend = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
            if let Some(input) = active_text_input_mut(&mut input_state, text_slot.field) {
                input.set_cursor_from_fraction(fraction, extend);
                let now = time.elapsed_secs_f64();
                let same_field = pointer_state.last_click_field == Some(text_slot.field);
                if same_field && now - pointer_state.last_click_time_s <= 0.35 {
                    pointer_state.click_count = pointer_state.click_count.saturating_add(1);
                } else {
                    pointer_state.click_count = 1;
                }
                pointer_state.last_click_field = Some(text_slot.field);
                pointer_state.last_click_time_s = now;
                if pointer_state.click_count == 2 {
                    input.select_word_at_cursor();
                } else if pointer_state.click_count >= 3 {
                    input.select_all();
                    pointer_state.click_count = 0;
                }
            }
            pointer_state.dragging = Some(text_slot.field);
            sync_session_text_inputs(&mut session, &input_state);
            session.ui_dirty = true;
            return;
        }
    }

    if mouse.pressed(MouseButton::Left)
        && let Some(dragging_field) = pointer_state.dragging
    {
        for (text_slot, cursor_position, slot_node) in &text_slots {
            if text_slot.field != dragging_field || !cursor_position.cursor_over {
                continue;
            }
            let fraction = pointer_text_fraction(
                text_slot.field,
                cursor_position,
                slot_node,
                &input_text_nodes,
                &selection_text_nodes,
            );
            if let Some(input) = active_text_input_mut(&mut input_state, dragging_field) {
                input.set_cursor_from_fraction(fraction, true);
            }
            sync_session_text_inputs(&mut session, &input_state);
            session.ui_dirty = true;
            return;
        }
    }
}

fn pointer_text_fraction(
    field: FocusField,
    cursor_position: &RelativeCursorPosition,
    slot_node: &ComputedNode,
    input_text_nodes: &Query<'_, '_, (&AuthUiInputText, &ComputedNode)>,
    selection_text_nodes: &Query<'_, '_, (&AuthUiSelectionText, &ComputedNode)>,
) -> f32 {
    let pointer_fraction = cursor_position
        .normalized
        .map(|position| position.x + 0.5)
        .unwrap_or(1.0)
        .clamp(0.0, 1.0);
    let pointer_x = pointer_fraction * slot_node.size().x.max(0.0);
    let text_width = input_text_nodes
        .iter()
        .filter(|(input, _)| input.field == field)
        .map(|(_, node)| node.size().x)
        .chain(
            selection_text_nodes
                .iter()
                .filter(|(selection, _)| selection.field == field)
                .map(|(_, node)| node.size().x),
        )
        .sum::<f32>();

    if text_width <= f32::EPSILON {
        return 0.0;
    }
    (pointer_x / text_width).clamp(0.0, 1.0)
}
