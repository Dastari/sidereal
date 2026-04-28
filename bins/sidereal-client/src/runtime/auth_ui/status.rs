fn auth_status_visible(status: &str) -> bool {
    !status.is_empty() && status != "Ready. Enter your gateway account credentials."
}

fn auth_status_tone(status: &str) -> Option<UiSemanticTone> {
    if !auth_status_visible(status) {
        return None;
    }
    let normalized = status.to_ascii_lowercase();
    if normalized.contains("failed")
        || normalized.contains("invalid")
        || normalized.contains("rejected")
        || normalized.contains("missing")
        || normalized.contains("no access token")
    {
        Some(UiSemanticTone::Danger)
    } else if normalized.contains("authenticator") || normalized.contains("code required") {
        Some(UiSemanticTone::Warning)
    } else {
        Some(UiSemanticTone::Info)
    }
}

fn auth_status_title(status: &str, tone: UiSemanticTone) -> &'static str {
    match tone {
        UiSemanticTone::Danger => "Authentication failed",
        UiSemanticTone::Warning if status.to_ascii_lowercase().contains("authenticator") => {
            "Authenticator required"
        }
        UiSemanticTone::Warning => "Action required",
        UiSemanticTone::Success => "Success",
        UiSemanticTone::Info => "Gateway status",
    }
}

fn flow_title(session: &ClientSession) -> &'static str {
    if session.totp_challenge_id.is_some() && session.selected_action == AuthAction::Login {
        return "Authenticator Required";
    }
    match session.selected_action {
        AuthAction::Login => "Login",
    }
}

fn submit_label(session: &ClientSession) -> &'static str {
    if session.totp_challenge_id.is_some() && session.selected_action == AuthAction::Login {
        return "Verify Code";
    }
    match session.selected_action {
        AuthAction::Login => "Login",
    }
}

fn is_field_visible(session: &ClientSession, field: FocusField) -> bool {
    let totp_required =
        session.selected_action == AuthAction::Login && session.totp_challenge_id.is_some();
    match session.selected_action {
        AuthAction::Login if totp_required => matches!(field, FocusField::TotpCode),
        AuthAction::Login => matches!(field, FocusField::Email | FocusField::Password),
    }
}

fn next_focus_field(session: &ClientSession, current: FocusField) -> FocusField {
    if session.selected_action == AuthAction::Login && session.totp_challenge_id.is_some() {
        return FocusField::TotpCode;
    }
    match session.selected_action {
        AuthAction::Login => match current {
            FocusField::Email => FocusField::Password,
            _ => FocusField::Email,
        },
    }
}

fn previous_focus_field(session: &ClientSession, current: FocusField) -> FocusField {
    if session.selected_action == AuthAction::Login && session.totp_challenge_id.is_some() {
        return FocusField::TotpCode;
    }
    match session.selected_action {
        AuthAction::Login => match current {
            FocusField::Password => FocusField::Email,
            _ => FocusField::Password,
        },
    }
}

fn input_kind(field: FocusField) -> TextInputKind {
    match field {
        FocusField::Password => TextInputKind::password(),
        FocusField::Email | FocusField::TotpCode => TextInputKind::Text,
    }
}

fn display_kind_for_field(
    field: FocusField,
    base_kind: TextInputKind,
    password_display: &AuthPasswordDisplayState,
) -> TextInputKind {
    if field == FocusField::Password && password_display.reveal_password {
        TextInputKind::Text
    } else {
        base_kind
    }
}

