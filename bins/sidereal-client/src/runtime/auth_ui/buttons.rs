#[allow(clippy::type_complexity)]
fn handle_auth_button_interactions(
    mut interactions: Query<
        '_,
        '_,
        (
            &Interaction,
            &AuthUiButton,
            Option<&AuthUiInputBox>,
            Option<&AuthUiTotpDigitBox>,
        ),
        Changed<Interaction>,
    >,
    mut session: ResMut<'_, ClientSession>,
    mut totp_cursor: ResMut<'_, TotpInputCursor>,
    mut request_state: ResMut<'_, super::auth_net::GatewayRequestState>,
    gateway_http: Res<'_, GatewayHttpAdapter>,
    mut app_exit: MessageWriter<'_, AppExit>,
    mut password_display: ResMut<'_, AuthPasswordDisplayState>,
) {
    for (interaction, button, input_box, totp_digit) in &mut interactions {
        match *interaction {
            Interaction::Pressed => {
                if let Some(input) = input_box {
                    session.focus = input.field;
                    session.ui_dirty = true;
                    continue;
                }
                if let Some(input) = totp_digit {
                    session.focus = input.field;
                    totp_cursor.index = input.index.min(TOTP_CODE_LENGTH - 1);
                    session.ui_dirty = true;
                    continue;
                }

                match button.0 {
                    AuthButtonKind::Submit => {
                        submit_auth_request(&mut session, request_state.as_mut(), *gateway_http);
                    }
                    AuthButtonKind::Focus(field) => {
                        session.focus = field;
                        session.ui_dirty = true;
                    }
                    AuthButtonKind::FocusTotpDigit(index) => {
                        session.focus = FocusField::TotpCode;
                        totp_cursor.index = index.min(TOTP_CODE_LENGTH - 1);
                        session.ui_dirty = true;
                    }
                    AuthButtonKind::TogglePasswordVisibility => {
                        session.focus = FocusField::Password;
                        password_display.reveal_password = !password_display.reveal_password;
                        session.ui_dirty = true;
                    }
                    AuthButtonKind::ForgotPasswordLink => {
                        let url = forgot_password_url();
                        match open_external_url(&url) {
                            Ok(()) => {
                                session.status =
                                    "Opened password reset in your browser.".to_string();
                            }
                            Err(err) => {
                                warn!("failed to open password reset URL: {err}");
                                session.status =
                                    format!("Open this URL to reset your password: {url}");
                            }
                        }
                        session.ui_dirty = true;
                    }
                    AuthButtonKind::Quit => {
                        app_exit.write(AppExit::Success);
                    }
                }
            }
            Interaction::Hovered | Interaction::None => {}
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn sync_auth_svg_icon_adornments(
    mut commands: Commands<'_, '_>,
    active_theme: Res<'_, ActiveUiTheme>,
    password_display: Res<'_, AuthPasswordDisplayState>,
    mut images: ResMut<'_, Assets<Image>>,
    mut icon_cache: Local<'_, AuthSvgIconHandleCache>,
    anchors: Query<'_, '_, (Entity, &AuthUiSvgIconAnchor, Option<&AuthUiStatusIconSlot>)>,
    mut icons: Query<'_, '_, (Entity, &AuthUiSvgIcon, &mut ImageNode)>,
) {
    let theme = theme_definition(active_theme.0);
    if icon_cache.theme_id != Some(active_theme.0) {
        icon_cache.handles_by_key.clear();
        icon_cache.theme_id = Some(active_theme.0);
    }

    let existing_anchors = icons
        .iter()
        .map(|(_, icon, _)| icon.anchor)
        .collect::<HashSet<_>>();
    for (anchor, icon_anchor, status_icon) in &anchors {
        if existing_anchors.contains(&anchor) {
            continue;
        }
        let kind = auth_svg_icon_kind(icon_anchor.role, password_display.reveal_password);
        let icon_color = auth_svg_icon_color(icon_anchor.role, status_icon, theme);
        let cache_key = AuthUiSvgIconCacheKey {
            kind,
            color: auth_svg_icon_color_key(icon_anchor.role, status_icon),
        };
        let Some(handle) =
            auth_svg_icon_handle(cache_key, icon_color, &mut icon_cache, &mut images)
        else {
            continue;
        };
        commands.entity(anchor).with_children(|slot| {
            slot.spawn((
                Node {
                    width: Val::Px(AUTH_INPUT_ICON_PX),
                    height: Val::Px(AUTH_INPUT_ICON_PX),
                    flex_shrink: 0.0,
                    ..default()
                },
                ImageNode::new(handle),
                FocusPolicy::Pass,
                AuthUiSvgIcon { anchor },
            ));
        });
    }

    for (entity, icon, mut image_node) in &mut icons {
        let Ok((_, anchor, status_icon)) = anchors.get(icon.anchor) else {
            commands.entity(entity).despawn();
            continue;
        };
        let kind = auth_svg_icon_kind(anchor.role, password_display.reveal_password);
        let icon_color = auth_svg_icon_color(anchor.role, status_icon, theme);
        let cache_key = AuthUiSvgIconCacheKey {
            kind,
            color: auth_svg_icon_color_key(anchor.role, status_icon),
        };
        let Some(handle) =
            auth_svg_icon_handle(cache_key, icon_color, &mut icon_cache, &mut images)
        else {
            continue;
        };
        image_node.image = handle;
    }
}

#[allow(clippy::type_complexity)]
fn sync_auth_button_visuals(
    active_theme: Res<'_, ActiveUiTheme>,
    visual_settings: Res<'_, UiVisualSettings>,
    session: Res<'_, ClientSession>,
    totp_cursor: Res<'_, TotpInputCursor>,
    mut query: Query<
        '_,
        '_,
        (
            &Interaction,
            &AuthUiButton,
            Option<&AuthUiInputBox>,
            Option<&AuthUiTotpDigitBox>,
            &mut BackgroundColor,
            Option<&mut BorderColor>,
            Option<&mut BoxShadow>,
        ),
    >,
) {
    let theme = theme_definition(active_theme.0);
    let glow_intensity = visual_settings.glow_intensity();
    for (interaction, button, input_box, totp_digit, mut bg, border, shadow) in &mut query {
        if matches!(
            button.0,
            AuthButtonKind::ForgotPasswordLink | AuthButtonKind::TogglePasswordVisibility
        ) {
            *bg = match *interaction {
                Interaction::Hovered | Interaction::Pressed => {
                    BackgroundColor(theme.colors.primary_color().with_alpha(0.08))
                }
                Interaction::None => BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.0)),
            };
            if let Some(mut border) = border {
                *border = BorderColor::all(Color::srgba(0.0, 0.0, 0.0, 0.0));
            }
            if let Some(mut shadow) = shadow {
                *shadow = BoxShadow::default();
            }
            continue;
        }

        let is_focused_control = match button.0 {
            AuthButtonKind::Focus(field) => {
                field == session.focus && is_field_visible(&session, field)
            }
            AuthButtonKind::TogglePasswordVisibility => {
                session.focus == FocusField::Password
                    && is_field_visible(&session, FocusField::Password)
            }
            AuthButtonKind::FocusTotpDigit(index) => {
                session.focus == FocusField::TotpCode
                    && is_field_visible(&session, FocusField::TotpCode)
                    && index == totp_cursor.index
            }
            AuthButtonKind::Submit | AuthButtonKind::ForgotPasswordLink | AuthButtonKind::Quit => {
                false
            }
        };
        let is_input_surface = input_box.is_some() || totp_digit.is_some();
        let state = match *interaction {
            Interaction::Pressed => UiInteractionState::Pressed,
            Interaction::Hovered if is_input_surface && is_focused_control => {
                UiInteractionState::Focused
            }
            Interaction::Hovered => UiInteractionState::Hovered,
            Interaction::None if is_focused_control => UiInteractionState::Focused,
            Interaction::None => UiInteractionState::Idle,
        };

        let (next_bg, next_border, next_shadow) = if is_input_surface {
            input_surface(
                theme,
                matches!(
                    state,
                    UiInteractionState::Focused | UiInteractionState::Pressed
                ),
                glow_intensity,
            )
        } else {
            let variant = match button.0 {
                AuthButtonKind::Submit => UiButtonVariant::Primary,
                AuthButtonKind::Focus(_) => UiButtonVariant::Outline,
                AuthButtonKind::FocusTotpDigit(_) => UiButtonVariant::Outline,
                AuthButtonKind::TogglePasswordVisibility => UiButtonVariant::Outline,
                AuthButtonKind::ForgotPasswordLink => UiButtonVariant::Outline,
                AuthButtonKind::Quit => UiButtonVariant::Outline,
            };
            button_surface(theme, variant, state, glow_intensity)
        };
        *bg = next_bg;
        if let Some(mut border) = border {
            *border = next_border;
        }
        if let Some(mut shadow) = shadow {
            *shadow = next_shadow;
        }
    }
}

