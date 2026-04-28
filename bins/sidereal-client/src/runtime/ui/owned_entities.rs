#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(super) fn update_owned_entities_panel_system(
    mut commands: Commands<'_, '_>,
    mut images: ResMut<'_, Assets<Image>>,
    fonts: Res<'_, EmbeddedFonts>,
    active_theme: Res<'_, ActiveUiTheme>,
    visual_settings: Res<'_, UiVisualSettings>,
    session: Res<'_, ClientSession>,
    player_view_state: Res<'_, LocalPlayerViewState>,
    manifest_cache: Res<'_, OwnedAssetManifestCache>,
    mut panel_state: ResMut<'_, OwnedEntitiesPanelState>,
    existing_panels: Query<'_, '_, Entity, With<OwnedEntitiesPanelRoot>>,
) {
    let Some(local_player_entity_id) = session.player_entity_id.as_ref() else {
        return;
    };
    let mut owned_ship_rows = manifest_cache
        .assets_by_entity_id
        .values()
        .filter(|asset| asset.kind.eq_ignore_ascii_case("ship"))
        .map(|asset| {
            let entity_id = asset.entity_id.clone();
            let label = if asset.display_name.trim().is_empty() {
                entity_id.clone()
            } else {
                asset.display_name.clone()
            };
            (entity_id, label)
        })
        .collect::<Vec<_>>();
    owned_ship_rows.sort_by(|a, b| {
        a.1.to_lowercase()
            .cmp(&b.1.to_lowercase())
            .then_with(|| a.0.cmp(&b.0))
    });
    owned_ship_rows.dedup_by(|a, b| a.0 == b.0);
    let entity_ids = owned_ship_rows
        .iter()
        .map(|(entity_id, _)| entity_id.clone())
        .collect::<Vec<_>>();
    let selected_id = player_view_state
        .desired_controlled_entity_id
        .clone()
        .or_else(|| player_view_state.controlled_entity_id.clone());

    if panel_state.last_entity_ids == entity_ids
        && panel_state.last_selected_id == selected_id
        && panel_state.last_detached_mode == player_view_state.detached_free_camera
        && !existing_panels.is_empty()
    {
        return;
    }
    panel_state.last_entity_ids = entity_ids.clone();
    panel_state.last_selected_id = selected_id.clone();
    panel_state.last_detached_mode = player_view_state.detached_free_camera;
    let theme = theme_definition(active_theme.0);
    let glow_intensity = visual_settings.glow_intensity();
    let (panel_bg, panel_border, panel_shadow) = panel_surface(theme, glow_intensity);

    for panel in &existing_panels {
        queue_despawn_if_exists(&mut commands, panel);
    }

    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: px(12),
                top: px(12),
                ..layout::panel(
                    px(280),
                    10.0,
                    8.0,
                    theme.metrics.panel_radius_px,
                    theme.metrics.panel_border_px,
                )
            },
            panel_bg,
            panel_border,
            panel_shadow,
            OwnedEntitiesPanelRoot,
            GameplayHud,
            UiOverlayLayer,
            RenderLayers::layer(UI_OVERLAY_RENDER_LAYER),
            WorldEntity,
            DespawnOnExit(ClientAppState::InWorld),
        ))
        .with_children(|panel| {
            spawn_hud_frame_chrome(
                panel,
                &mut images,
                theme,
                Some("Owned Fleet"),
                &fonts.mono,
                glow_intensity,
            );
            panel.spawn((
                Text::new("Owned Ships"),
                text_font(fonts.bold.clone(), 18.0),
                TextColor(theme.colors.foreground_color()),
            ));

            let free_roam_selected = selected_id
                .as_deref()
                .is_some_and(|selected| ids_refer_to_same_guid(selected, local_player_entity_id))
                && !player_view_state.detached_free_camera;
            let free_roam_state = if free_roam_selected {
                UiInteractionState::Selected
            } else {
                UiInteractionState::Idle
            };
            let (free_roam_bg, free_roam_border, free_roam_shadow) = button_surface(
                theme,
                UiButtonVariant::Secondary,
                free_roam_state,
                glow_intensity,
            );
            panel
                .spawn((
                    Button,
                    OwnedEntitiesPanelButton {
                        action: OwnedEntitiesPanelAction::FreeRoam,
                    },
                    layout::leading_button(
                        percent(100.0),
                        34.0,
                        theme.metrics.input_radius_px,
                        theme.metrics.control_border_px,
                        10.0,
                    ),
                    free_roam_bg,
                    free_roam_border,
                    free_roam_shadow,
                ))
                .with_children(|button| {
                    button.spawn((
                        Text::new("FREE ROAM"),
                        text_font(fonts.mono_bold.clone(), 17.0),
                        TextColor(theme.colors.panel_foreground_color()),
                    ));
                });
            if owned_ship_rows.is_empty() {
                panel.spawn((
                    Text::new("No owned entities visible"),
                    text_font(fonts.regular.clone(), 13.0),
                    TextColor(theme.colors.muted_foreground_color()),
                ));
            } else {
                for (entity_id, display_label) in owned_ship_rows {
                    let is_selected = selected_id.as_deref().is_some_and(|selected| {
                        ids_refer_to_same_guid(selected, entity_id.as_str())
                    });
                    let button_state = if is_selected {
                        UiInteractionState::Selected
                    } else {
                        UiInteractionState::Idle
                    };
                    let (button_bg, button_border, button_shadow) = button_surface(
                        theme,
                        UiButtonVariant::Secondary,
                        button_state,
                        glow_intensity,
                    );
                    panel
                        .spawn((
                            Button,
                            OwnedEntitiesPanelButton {
                                action: OwnedEntitiesPanelAction::ControlEntity(entity_id),
                            },
                            layout::leading_button(
                                percent(100.0),
                                34.0,
                                theme.metrics.input_radius_px,
                                theme.metrics.control_border_px,
                                10.0,
                            ),
                            button_bg,
                            button_border,
                            button_shadow,
                        ))
                        .with_children(|button| {
                            button.spawn((
                                Text::new(display_label.to_ascii_uppercase()),
                                text_font(fonts.mono_bold.clone(), 17.0),
                                TextColor(theme.colors.panel_foreground_color()),
                            ));
                        });
                }
            }
        });
}

#[allow(clippy::type_complexity)]
pub(super) fn handle_owned_entities_panel_buttons(
    active_theme: Res<'_, ActiveUiTheme>,
    visual_settings: Res<'_, UiVisualSettings>,
    mut interactions: Query<
        '_,
        '_,
        (
            &Interaction,
            &OwnedEntitiesPanelButton,
            &mut BackgroundColor,
            &mut BorderColor,
            &mut BoxShadow,
        ),
        Changed<Interaction>,
    >,
    session: Res<'_, ClientSession>,
    mut player_view_state: ResMut<'_, LocalPlayerViewState>,
    mut control_request_state: ResMut<'_, ClientControlRequestState>,
    mut panel_state: ResMut<'_, OwnedEntitiesPanelState>,
) {
    let theme = theme_definition(active_theme.0);
    let glow_intensity = visual_settings.glow_intensity();
    for (interaction, button, mut color, mut border, mut shadow) in &mut interactions {
        match *interaction {
            Interaction::Pressed => {
                match &button.action {
                    OwnedEntitiesPanelAction::FreeRoam => {
                        let target = session.player_entity_id.clone();
                        let next_request_seq =
                            control_request_state.next_request_seq.saturating_add(1);
                        info!(
                            "client control selection requested via owned panel player={} target={} seq={}",
                            session.player_entity_id.as_deref().unwrap_or("<none>"),
                            target.as_deref().unwrap_or("<player-anchor>"),
                            next_request_seq
                        );
                        player_view_state.desired_controlled_entity_id = target.clone();
                        control_request_state.next_request_seq = next_request_seq;
                        control_request_state.pending_controlled_entity_id = target;
                        control_request_state.pending_request_seq =
                            Some(control_request_state.next_request_seq);
                        control_request_state.last_sent_request_seq = None;
                        control_request_state.last_sent_at_s = 0.0;
                        // Free roam means the player entity is the controlled entity.
                        // Keep attached camera/input flow active so player movement works.
                        player_view_state.detached_free_camera = false;
                        player_view_state.selected_entity_id = session.player_entity_id.clone();
                    }
                    OwnedEntitiesPanelAction::ControlEntity(entity_id) => {
                        let next_request_seq =
                            control_request_state.next_request_seq.saturating_add(1);
                        info!(
                            "client control selection requested via owned panel player={} target={} seq={}",
                            session.player_entity_id.as_deref().unwrap_or("<none>"),
                            entity_id,
                            next_request_seq
                        );
                        player_view_state.desired_controlled_entity_id = Some(entity_id.clone());
                        control_request_state.next_request_seq = next_request_seq;
                        control_request_state.pending_controlled_entity_id =
                            Some(entity_id.clone());
                        control_request_state.pending_request_seq =
                            Some(control_request_state.next_request_seq);
                        control_request_state.last_sent_request_seq = None;
                        control_request_state.last_sent_at_s = 0.0;
                        player_view_state.detached_free_camera = false;
                        player_view_state.selected_entity_id = Some(entity_id.clone());
                    }
                }
                panel_state.last_selected_id = None;
            }
            Interaction::Hovered => {}
            Interaction::None => {}
        }
        let state = match *interaction {
            Interaction::Pressed => UiInteractionState::Pressed,
            Interaction::Hovered => UiInteractionState::Hovered,
            Interaction::None => {
                let is_selected = match &button.action {
                    OwnedEntitiesPanelAction::FreeRoam => {
                        player_view_state
                            .desired_controlled_entity_id
                            .as_deref()
                            .zip(session.player_entity_id.as_deref())
                            .is_some_and(|(desired, session_player)| {
                                ids_refer_to_same_guid(desired, session_player)
                            })
                            && !player_view_state.detached_free_camera
                    }
                    OwnedEntitiesPanelAction::ControlEntity(entity_id) => {
                        player_view_state.desired_controlled_entity_id.as_ref() == Some(entity_id)
                    }
                };
                if is_selected {
                    UiInteractionState::Selected
                } else {
                    UiInteractionState::Idle
                }
            }
        };
        let (next_bg, next_border, next_shadow) =
            button_surface(theme, UiButtonVariant::Secondary, state, glow_intensity);
        *color = next_bg;
        *border = next_border;
        *shadow = next_shadow;
    }
}

