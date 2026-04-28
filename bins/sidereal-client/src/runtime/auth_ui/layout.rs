fn setup_auth_screen(
    mut commands: Commands<'_, '_>,
    mut images: ResMut<'_, Assets<Image>>,
    fonts: Res<'_, EmbeddedFonts>,
    active_theme: Res<'_, ActiveUiTheme>,
    visual_settings: Res<'_, UiVisualSettings>,
    session: Res<'_, ClientSession>,
    mut input_state: ResMut<'_, AuthReusableInputState>,
) {
    info!("client auth UI setup: spawning auth screen");
    input_state.email.set_text(session.email.clone());
    input_state.password.set_text(session.password.clone());
    let theme = theme_definition(active_theme.0);
    let glow_intensity = visual_settings.glow_intensity();
    let (panel_bg, panel_border, panel_shadow) = panel_surface(theme, glow_intensity);
    let (submit_bg, submit_border, submit_shadow) = button_surface(
        theme,
        UiButtonVariant::Primary,
        UiInteractionState::Idle,
        glow_intensity,
    );
    let (quit_bg, quit_border, quit_shadow) = button_surface(
        theme,
        UiButtonVariant::Outline,
        UiInteractionState::Idle,
        glow_intensity,
    );

    commands
        .spawn((
            layout::fullscreen_centered_root(),
            Transform::default(),
            GlobalTransform::default(),
            AuthUiRoot,
            DespawnOnExit(ClientAppState::Auth),
        ))
        .with_children(|root| {
            root.spawn((
                layout::fullscreen_backdrop(),
                Transform::default(),
                GlobalTransform::default(),
                BackgroundColor(theme.colors.background_color()),
                AuthUiBackdrop,
            ));

            root.spawn((
                layout::panel(
                    Val::Px(420.0),
                    16.0,
                    10.0,
                    theme.metrics.panel_radius_px,
                    theme.metrics.panel_border_px,
                ),
                Transform::default(),
                GlobalTransform::default(),
                panel_bg,
                panel_border,
                panel_shadow,
            ))
            .with_children(|panel| {
                spawn_hud_frame_chrome(
                    panel,
                    &mut images,
                    theme,
                    Some("Auth Terminal"),
                    &fonts.mono.clone(),
                    glow_intensity,
                );

                panel.spawn((
                    Text::new("SIDEREAL"),
                    text_font(fonts.display.clone(), 30.0),
                    TextColor(theme.colors.foreground_color()),
                ));

                panel.spawn((
                    Text::new("Login"),
                    text_font(fonts.mono.clone(), 12.0),
                    TextColor(theme.colors.muted_foreground_color()),
                    AuthUiFlowTitle,
                ));

                spawn_status_frame(panel, &mut images, &fonts, theme, glow_intensity);

                spawn_input_field(
                    panel,
                    &fonts,
                    theme,
                    glow_intensity,
                    "Email",
                    FocusField::Email,
                    TextInputKind::Text,
                    AuthUiSvgIconRole::Email,
                    None,
                );
                spawn_input_field(
                    panel,
                    &fonts,
                    theme,
                    glow_intensity,
                    "Password",
                    FocusField::Password,
                    TextInputKind::password(),
                    AuthUiSvgIconRole::Password,
                    Some(AuthUiSvgIconRole::PasswordVisibilityToggle),
                );
                spawn_totp_code_input(
                    panel,
                    &fonts,
                    theme,
                    glow_intensity,
                    "Authenticator Code",
                    FocusField::TotpCode,
                );
                panel
                    .spawn((
                        Button,
                        AuthUiButton(AuthButtonKind::Submit),
                        layout::button(
                            Val::Percent(100.0),
                            42.0,
                            theme.metrics.input_radius_px,
                            theme.metrics.control_border_px,
                        ),
                        submit_bg,
                        submit_border,
                        submit_shadow,
                    ))
                    .with_children(|button| {
                        button.spawn((
                            Text::new("LOGIN"),
                            text_font(fonts.mono_bold.clone(), 18.0),
                            TextColor(theme.colors.primary_foreground_color()),
                            AuthUiSubmitLabel,
                        ));
                    });

                panel
                    .spawn((
                        Button,
                        AuthUiButton(AuthButtonKind::ForgotPasswordLink),
                        Node {
                            align_self: AlignSelf::FlexEnd,
                            width: Val::Px(170.0),
                            height: Val::Px(24.0),
                            justify_content: JustifyContent::FlexEnd,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        Transform::default(),
                        GlobalTransform::default(),
                        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.0)),
                    ))
                    .with_children(|link| {
                        link.spawn((
                            Text::new("Forgot Password?"),
                            text_font(fonts.mono_bold.clone(), 13.0),
                            TextColor(theme.colors.primary_color()),
                        ));
                    });
                panel
                    .spawn((
                        Button,
                        AuthUiButton(AuthButtonKind::Quit),
                        Node {
                            align_self: AlignSelf::FlexEnd,
                            ..layout::button(
                                Val::Px(140.0),
                                38.0,
                                theme.metrics.input_radius_px,
                                theme.metrics.control_border_px,
                            )
                        },
                        Transform::default(),
                        GlobalTransform::default(),
                        quit_bg,
                        quit_border,
                        quit_shadow,
                    ))
                    .with_children(|button| {
                        button.spawn((
                            Text::new("QUIT"),
                            text_font(fonts.mono_bold.clone(), 16.0),
                            TextColor(theme.colors.panel_foreground_color()),
                        ));
                    });
            });
        });
}

fn spawn_status_frame(
    parent: &mut ChildSpawnerCommands,
    images: &mut Assets<Image>,
    fonts: &EmbeddedFonts,
    theme: sidereal_ui::theme::UiTheme,
    glow_intensity: f32,
) {
    for tone in [
        UiSemanticTone::Danger,
        UiSemanticTone::Warning,
        UiSemanticTone::Info,
    ] {
        let (status_bg, status_border, status_shadow) =
            panel_surface_with_tone(theme, glow_intensity, tone);
        let status_text_color = tone.foreground_color(theme);
        let mut frame = parent.spawn((
            Node {
                display: Display::None,
                width: Val::Percent(100.0),
                min_height: Val::Px(58.0),
                padding: UiRect::all(Val::Px(12.0)),
                align_items: AlignItems::Center,
                column_gap: Val::Px(10.0),
                border: UiRect::all(Val::Px(theme.metrics.control_border_px)),
                border_radius: BorderRadius::all(Val::Px(theme.metrics.control_radius_px.max(4.0))),
                ..default()
            },
            Transform::default(),
            GlobalTransform::default(),
            status_bg,
            status_border,
            status_shadow,
            AuthUiStatusFrame { tone },
        ));

        frame.with_children(|status| {
            spawn_hud_frame_chrome_with_tone(
                status,
                images,
                theme,
                None,
                &fonts.mono,
                glow_intensity,
                tone,
            );

            status.spawn((
                Node {
                    width: Val::Px(28.0),
                    ..layout::input_adornment()
                },
                Transform::default(),
                GlobalTransform::default(),
                AuthUiSvgIconAnchor {
                    role: AuthUiSvgIconRole::Alert,
                },
                AuthUiStatusIconSlot { tone },
            ));

            status
                .spawn((
                    Node {
                        flex_grow: 1.0,
                        min_width: Val::Px(0.0),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(3.0),
                        ..default()
                    },
                    Transform::default(),
                    GlobalTransform::default(),
                ))
                .with_children(|copy| {
                    copy.spawn((
                        Text::new(""),
                        text_font(fonts.mono_bold.clone(), 11.0),
                        TextColor(status_text_color),
                        AuthUiStatusTitle { tone },
                    ));
                    copy.spawn((
                        Text::new(""),
                        text_font(fonts.mono.clone(), 12.0),
                        TextColor(status_text_color),
                        AuthUiStatusText { tone },
                    ));
                });
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_input_field(
    parent: &mut ChildSpawnerCommands,
    fonts: &EmbeddedFonts,
    theme: sidereal_ui::theme::UiTheme,
    glow_intensity: f32,
    label: &str,
    field: FocusField,
    kind: TextInputKind,
    start_icon: AuthUiSvgIconRole,
    end_icon: Option<AuthUiSvgIconRole>,
) {
    let (input_bg, input_border, input_shadow) = input_surface(theme, false, glow_intensity);
    parent
        .spawn((
            layout::vertical_stack(6.0),
            Transform::default(),
            GlobalTransform::default(),
            AuthUiFieldContainer { field },
        ))
        .with_children(|container| {
            container.spawn((
                Text::new(label.to_ascii_uppercase()),
                text_font(fonts.bold.clone(), 11.0),
                TextColor(theme.colors.muted_foreground_color()),
            ));

            container
                .spawn((
                    Button,
                    AuthUiInputBox { field },
                    AuthUiButton(AuthButtonKind::Focus(field)),
                    layout::input_box_with_adornments(
                        44.0,
                        theme.metrics.input_radius_px,
                        theme.metrics.control_border_px,
                        true,
                        end_icon.is_some(),
                    ),
                    Transform::default(),
                    GlobalTransform::default(),
                    RelativeCursorPosition::default(),
                    input_bg,
                    input_border,
                    input_shadow,
                ))
                .with_children(|input_box| {
                    spawn_input_svg_adornment(input_box, start_icon, false);

                    input_box
                        .spawn((
                            layout::input_text_slot(),
                            Transform::default(),
                            GlobalTransform::default(),
                            RelativeCursorPosition::default(),
                            AuthUiInputTextSlot { field },
                        ))
                        .with_children(|slot| {
                            slot.spawn((
                                Text::new(""),
                                text_font(fonts.bold.clone(), 16.0),
                                TextColor(theme.colors.panel_foreground_color()),
                                AuthUiInputText {
                                    field,
                                    segment: AuthInputTextSegment::BeforeSelection,
                                    kind,
                                },
                            ));

                            slot.spawn((
                                Node {
                                    width: Val::Px(AUTH_INPUT_CARET_WIDTH_PX),
                                    height: Val::Px(AUTH_INPUT_CARET_HEIGHT_PX),
                                    flex_shrink: 0.0,
                                    display: Display::None,
                                    ..default()
                                },
                                Transform::default(),
                                GlobalTransform::default(),
                                BackgroundColor(theme.colors.glow_color()),
                                AuthUiCursor {
                                    field,
                                    edge: AuthInputCursorEdge::SelectionStart,
                                },
                            ));

                            slot.spawn((
                                Node {
                                    display: Display::None,
                                    min_width: Val::Px(0.0),
                                    padding: UiRect::axes(Val::Px(2.0), Val::Px(1.0)),
                                    border_radius: BorderRadius::all(Val::Px(3.0)),
                                    ..default()
                                },
                                Transform::default(),
                                GlobalTransform::default(),
                                BackgroundColor(theme.colors.primary_color().with_alpha(0.28)),
                                AuthUiSelectionBox { field },
                            ))
                            .with_children(|selection| {
                                selection.spawn((
                                    Text::new(""),
                                    text_font(fonts.bold.clone(), 16.0),
                                    TextColor(theme.colors.panel_foreground_color()),
                                    AuthUiSelectionText { field },
                                ));
                            });

                            slot.spawn((
                                Node {
                                    width: Val::Px(AUTH_INPUT_CARET_WIDTH_PX),
                                    height: Val::Px(AUTH_INPUT_CARET_HEIGHT_PX),
                                    flex_shrink: 0.0,
                                    display: Display::None,
                                    ..default()
                                },
                                Transform::default(),
                                GlobalTransform::default(),
                                BackgroundColor(theme.colors.glow_color()),
                                AuthUiCursor {
                                    field,
                                    edge: AuthInputCursorEdge::SelectionEnd,
                                },
                            ));

                            slot.spawn((
                                Text::new(""),
                                text_font(fonts.bold.clone(), 16.0),
                                TextColor(theme.colors.panel_foreground_color()),
                                AuthUiInputText {
                                    field,
                                    segment: AuthInputTextSegment::AfterSelection,
                                    kind,
                                },
                            ));
                        });

                    if let Some(end_icon) = end_icon {
                        spawn_input_svg_adornment(input_box, end_icon, true);
                    }
                });
        });
}

fn spawn_input_svg_adornment(
    parent: &mut ChildSpawnerCommands,
    role: AuthUiSvgIconRole,
    interactive: bool,
) {
    let mut entity = parent.spawn((
        Node {
            width: Val::Px(24.0),
            ..layout::input_adornment()
        },
        Transform::default(),
        GlobalTransform::default(),
        AuthUiSvgIconAnchor { role },
    ));
    if interactive {
        entity.insert((
            Button,
            AuthUiButton(AuthButtonKind::TogglePasswordVisibility),
            RelativeCursorPosition::default(),
        ));
    }
}

fn spawn_totp_code_input(
    parent: &mut ChildSpawnerCommands,
    fonts: &EmbeddedFonts,
    theme: sidereal_ui::theme::UiTheme,
    glow_intensity: f32,
    label: &str,
    field: FocusField,
) {
    parent
        .spawn((
            layout::vertical_stack(6.0),
            Transform::default(),
            GlobalTransform::default(),
            AuthUiFieldContainer { field },
            AuthUiTotpCodeInput,
        ))
        .with_children(|container| {
            container.spawn((
                Text::new(label.to_ascii_uppercase()),
                text_font(fonts.bold.clone(), 11.0),
                TextColor(theme.colors.muted_foreground_color()),
            ));

            container
                .spawn((
                    Node {
                        display: Display::Grid,
                        width: Val::Percent(100.0),
                        height: Val::Px(48.0),
                        grid_template_columns: RepeatedGridTrack::flex(
                            TOTP_CODE_LENGTH as u16,
                            1.0,
                        ),
                        column_gap: Val::Px(8.0),
                        ..default()
                    },
                    Transform::default(),
                    GlobalTransform::default(),
                ))
                .with_children(|digits| {
                    for index in 0..TOTP_CODE_LENGTH {
                        let (input_bg, input_border, input_shadow) =
                            input_surface(theme, false, glow_intensity);
                        let mut digit_entity = digits.spawn_empty();
                        digit_entity.insert(Button);
                        digit_entity.insert(AuthUiButton(AuthButtonKind::FocusTotpDigit(index)));
                        digit_entity.insert(AuthUiTotpDigitBox { field, index });
                        digit_entity.insert(Node {
                            width: Val::Percent(100.0),
                            height: Val::Px(48.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            border: UiRect::all(Val::Px(theme.metrics.control_border_px)),
                            border_radius: BorderRadius::all(Val::Px(
                                theme.metrics.input_radius_px,
                            )),
                            ..default()
                        });
                        digit_entity.insert(Transform::default());
                        digit_entity.insert(GlobalTransform::default());
                        digit_entity.insert(input_bg);
                        digit_entity.insert(input_border);
                        digit_entity.insert(input_shadow);
                        digit_entity.with_children(|digit| {
                            digit.spawn((
                                Text::new(""),
                                text_font(fonts.mono_bold.clone(), 22.0),
                                TextColor(theme.colors.panel_foreground_color()),
                                AuthUiTotpDigitText { index },
                            ));

                            digit.spawn((
                                Text::new("|"),
                                text_font(fonts.mono.clone(), 18.0),
                                TextColor(theme.colors.glow_color()),
                                AuthUiTotpDigitCursor { index },
                                Visibility::Hidden,
                            ));
                        });
                    }
                });
        });
}

