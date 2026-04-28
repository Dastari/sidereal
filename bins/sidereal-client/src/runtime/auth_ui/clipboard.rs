fn active_text_input_mut(
    input_state: &mut AuthReusableInputState,
    field: FocusField,
) -> Option<&mut TextInputState> {
    match field {
        FocusField::Email => Some(&mut input_state.email),
        FocusField::Password => Some(&mut input_state.password),
        FocusField::TotpCode => None,
    }
}

fn active_text_input(
    input_state: &AuthReusableInputState,
    field: FocusField,
) -> Option<&TextInputState> {
    match field {
        FocusField::Email => Some(&input_state.email),
        FocusField::Password => Some(&input_state.password),
        FocusField::TotpCode => None,
    }
}

fn sync_session_text_inputs(session: &mut ClientSession, input_state: &AuthReusableInputState) {
    session.email.clone_from(&input_state.email.text);
    session.password.clone_from(&input_state.password.text);
}

fn control_modifier(keys: &ButtonInput<KeyCode>) -> bool {
    keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight)
}

fn command_modifier(keys: &ButtonInput<KeyCode>) -> bool {
    keys.pressed(KeyCode::SuperLeft) || keys.pressed(KeyCode::SuperRight)
}

fn primary_modifier(keys: &ButtonInput<KeyCode>) -> bool {
    control_modifier(keys) || command_modifier(keys)
}

fn word_modifier(keys: &ButtonInput<KeyCode>) -> bool {
    control_modifier(keys)
        || (cfg!(target_os = "macos")
            && (keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight)))
}

fn handle_text_input_shortcut(
    key_code: KeyCode,
    keys: &ButtonInput<KeyCode>,
    input_state: &mut AuthReusableInputState,
    session: &mut ClientSession,
) {
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    match key_code {
        KeyCode::KeyA => {
            if let Some(input) = active_text_input_mut(input_state, session.focus) {
                input.select_all();
                session.ui_dirty = true;
            }
        }
        KeyCode::KeyC => {
            copy_active_selection(input_state, session.focus);
        }
        KeyCode::KeyX => {
            cut_active_selection(input_state, session.focus);
            sync_session_text_inputs(session, input_state);
            session.ui_dirty = true;
        }
        KeyCode::KeyV => {
            paste_into_active_input(input_state, session.focus);
            sync_session_text_inputs(session, input_state);
            session.ui_dirty = true;
        }
        KeyCode::KeyZ if shift => {
            if let Some(input) = active_text_input_mut(input_state, session.focus) {
                input.redo();
            }
            sync_session_text_inputs(session, input_state);
            session.ui_dirty = true;
        }
        KeyCode::KeyZ => {
            if let Some(input) = active_text_input_mut(input_state, session.focus) {
                input.undo();
            }
            sync_session_text_inputs(session, input_state);
            session.ui_dirty = true;
        }
        KeyCode::KeyY => {
            if let Some(input) = active_text_input_mut(input_state, session.focus) {
                input.redo();
            }
            sync_session_text_inputs(session, input_state);
            session.ui_dirty = true;
        }
        _ => {}
    }
}

fn handle_totp_shortcut(
    key_code: KeyCode,
    keys: &ButtonInput<KeyCode>,
    input_state: &mut AuthReusableInputState,
    session: &mut ClientSession,
    totp_cursor: &mut TotpInputCursor,
) {
    match key_code {
        KeyCode::KeyV => {
            paste_into_totp(input_state, &mut session.totp_code, totp_cursor);
            session.ui_dirty = true;
        }
        KeyCode::KeyA => {
            totp_cursor.index = 0;
            session.ui_dirty = true;
        }
        KeyCode::KeyC if !session.totp_code.is_empty() => {
            input_state.clipboard = session.totp_code.clone();
            write_system_clipboard(&session.totp_code);
        }
        KeyCode::KeyX if !session.totp_code.is_empty() => {
            input_state.clipboard = session.totp_code.clone();
            write_system_clipboard(&session.totp_code);
            session.totp_code.clear();
            totp_cursor.index = 0;
            session.ui_dirty = true;
        }
        _ if keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight) => {}
        _ => {}
    }
}

fn copy_active_selection(input_state: &mut AuthReusableInputState, field: FocusField) {
    if let Some(selected) =
        active_text_input(input_state, field).and_then(TextInputState::copy_selection)
    {
        write_system_clipboard(&selected);
        input_state.clipboard = selected;
    }
}

fn cut_active_selection(input_state: &mut AuthReusableInputState, field: FocusField) {
    if let Some(input) = active_text_input_mut(input_state, field)
        && let Some(selected) = input.cut_selection()
    {
        write_system_clipboard(&selected);
        input_state.clipboard = selected;
    }
}

fn paste_into_active_input(input_state: &mut AuthReusableInputState, field: FocusField) {
    let clipboard = read_system_clipboard().unwrap_or_else(|| input_state.clipboard.clone());
    if clipboard.is_empty() {
        return;
    }
    if let Some(input) = active_text_input_mut(input_state, field) {
        input.insert_text(&clipboard);
    }
}

fn paste_into_totp(
    input_state: &mut AuthReusableInputState,
    code: &mut String,
    cursor: &mut TotpInputCursor,
) {
    let clipboard = read_system_clipboard().unwrap_or_else(|| input_state.clipboard.clone());
    if clipboard.is_empty() {
        return;
    }
    let normalized = normalize_totp_code(&clipboard);
    if normalized.len() == TOTP_CODE_LENGTH {
        *code = normalized;
        cursor.index = TOTP_CODE_LENGTH - 1;
    } else {
        insert_totp_digits(code, cursor, &clipboard);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn read_system_clipboard() -> Option<String> {
    let mut clipboard = arboard::Clipboard::new().ok()?;
    clipboard.get_text().ok()
}

#[cfg(target_arch = "wasm32")]
fn read_system_clipboard() -> Option<String> {
    None
}

#[cfg(not(target_arch = "wasm32"))]
fn write_system_clipboard(value: &str) {
    if let Ok(mut clipboard) = arboard::Clipboard::new() {
        let _ = clipboard.set_text(value.to_string());
    }
}

#[cfg(target_arch = "wasm32")]
fn write_system_clipboard(_value: &str) {}
