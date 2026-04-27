//! Scene setup systems for UI camera and character select.

use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use bevy::state::state_scoped::DespawnOnExit;
use sidereal_ui::layout;
use sidereal_ui::theme::{ActiveUiTheme, UiVisualSettings, theme_definition};
use sidereal_ui::typography::text_font;
use sidereal_ui::widgets::{
    UiButtonVariant, UiInteractionState, button_surface, panel_surface, spawn_hud_frame_chrome,
};

use super::app_state::{CharacterSelectionState, ClientAppState, ClientSession};
use super::auth_net;
use super::components::{
    CharacterSelectButton, CharacterSelectEnterButton, CharacterSelectPreviewMetaText,
    CharacterSelectPreviewNameText, CharacterSelectRoot, CharacterSelectStatusText,
    UiOverlayCamera,
};
use super::platform::UI_OVERLAY_RENDER_LAYER;
use super::resources::EmbeddedFonts;

pub(super) fn spawn_ui_overlay_camera(mut commands: Commands<'_, '_>) {
    commands.spawn((
        Camera2d,
        Camera {
            // Keep UI rendering independent from auth/world camera lifecycles.
            order: 100,
            clear_color: ClearColorConfig::None,
            ..default()
        },
        // Prevent world sprites/meshes from being rendered twice by the UI overlay camera.
        RenderLayers::layer(UI_OVERLAY_RENDER_LAYER),
        UiOverlayCamera,
    ));
}

pub(super) fn insert_embedded_fonts(app: &mut App) {
    static BODY_BOLD: &[u8] = include_bytes!("../../../../data/fonts/Rajdhani-Bold.ttf");
    static BODY_REGULAR: &[u8] = include_bytes!("../../../../data/fonts/Rajdhani-Regular.ttf");
    static DISPLAY: &[u8] = include_bytes!("../../../../data/fonts/Orbitron-Variable.ttf");
    static MONO: &[u8] = include_bytes!("../../../../data/fonts/GeistMono-Regular.ttf");
    static MONO_BOLD: &[u8] = include_bytes!("../../../../data/fonts/GeistMono-Bold.ttf");

    let mut fonts = app.world_mut().resource_mut::<Assets<Font>>();
    let bold = fonts.add(
        Font::try_from_bytes(BODY_BOLD.to_vec()).expect("embedded Rajdhani-Bold.ttf is valid"),
    );
    let regular = fonts.add(
        Font::try_from_bytes(BODY_REGULAR.to_vec())
            .expect("embedded Rajdhani-Regular.ttf is valid"),
    );
    let display = fonts.add(
        Font::try_from_bytes(DISPLAY.to_vec()).expect("embedded Orbitron-Variable.ttf is valid"),
    );
    let mono = fonts
        .add(Font::try_from_bytes(MONO.to_vec()).expect("embedded GeistMono-Regular.ttf is valid"));
    let mono_bold = fonts.add(
        Font::try_from_bytes(MONO_BOLD.to_vec()).expect("embedded GeistMono-Bold.ttf is valid"),
    );
    app.insert_resource(EmbeddedFonts {
        bold,
        regular,
        display,
        mono,
        mono_bold,
    });
    app.init_resource::<ActiveUiTheme>();
    app.init_resource::<UiVisualSettings>();
}

pub(super) fn setup_character_select_screen(
    mut commands: Commands<'_, '_>,
    mut images: ResMut<'_, Assets<Image>>,
    fonts: Res<'_, EmbeddedFonts>,
    active_theme: Res<'_, ActiveUiTheme>,
    visual_settings: Res<'_, UiVisualSettings>,
    character_selection: Res<'_, CharacterSelectionState>,
) {
    let theme = theme_definition(active_theme.0);
    let glow_intensity = visual_settings.glow_intensity();
    let (panel_bg, panel_border, panel_shadow) = panel_surface(theme, glow_intensity);

    commands
        .spawn((
            layout::fullscreen_centered_root(),
            Transform::default(),
            GlobalTransform::default(),
            CharacterSelectRoot,
            DespawnOnExit(ClientAppState::CharacterSelect),
        ))
        .with_children(|root| {
            root.spawn((
                layout::panel(
                    Val::Px(940.0),
                    theme.metrics.panel_padding_px,
                    16.0,
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
                    Some("Character Registry"),
                    &fonts.mono.clone(),
                    glow_intensity,
                );

                panel.spawn((
                    Text::new("Character Select"),
                    text_font(fonts.display.clone(), 34.0),
                    TextColor(theme.colors.foreground_color()),
                ));
                panel.spawn((
                    Text::new("Choose a character, then enter world."),
                    text_font(fonts.mono.clone(), 12.0),
                    TextColor(theme.colors.muted_foreground_color()),
                ));

                panel
                    .spawn((
                        layout::horizontal_stack(18.0, JustifyContent::SpaceBetween),
                        Transform::default(),
                        GlobalTransform::default(),
                    ))
                    .with_children(|content| {
                        let (preview_bg, preview_border, preview_shadow) = panel_surface(
                            theme,
                            glow_intensity * 0.65,
                        );
                        content
                            .spawn((
                                Node {
                                    width: Val::Percent(58.0),
                                    min_height: Val::Px(330.0),
                                    padding: UiRect::all(Val::Px(18.0)),
                                    border: UiRect::all(Val::Px(theme.metrics.panel_border_px)),
                                    border_radius: BorderRadius::all(Val::Px(
                                        theme.metrics.panel_radius_px,
                                    )),
                                    flex_direction: FlexDirection::Column,
                                    justify_content: JustifyContent::FlexEnd,
                                    row_gap: Val::Px(8.0),
                                    ..default()
                                },
                                Transform::default(),
                                GlobalTransform::default(),
                                preview_bg,
                                preview_border,
                                preview_shadow,
                            ))
                            .with_children(|preview| {
                                spawn_hud_frame_chrome(
                                    preview,
                                    &mut images,
                                    theme,
                                    Some("Selected Character"),
                                    &fonts.mono.clone(),
                                    glow_intensity,
                                );
                                let selected = character_selection.selected_character();
                                preview.spawn((
                                    Text::new(
                                        selected
                                            .map(character_display_name)
                                            .unwrap_or("No Character")
                                            .to_string(),
                                    ),
                                    text_font(fonts.display.clone(), 28.0),
                                    TextColor(theme.colors.primary_color()),
                                    CharacterSelectPreviewNameText,
                                ));
                                preview.spawn((
                                    Text::new(
                                        selected
                                            .map(|character| {
                                                format!(
                                                    "{} / {}",
                                                    character.status.to_ascii_uppercase(),
                                                    short_player_id(&character.player_entity_id)
                                                )
                                            })
                                            .unwrap_or_else(|| {
                                                "Create a character from the dashboard or account site."
                                                    .to_string()
                                            }),
                                    ),
                                    text_font(fonts.mono.clone(), 13.0),
                                    TextColor(theme.colors.muted_foreground_color()),
                                    CharacterSelectPreviewMetaText,
                                ));
                            });

                        content
                            .spawn((
                                Node {
                                    width: Val::Percent(42.0),
                                    flex_direction: FlexDirection::Column,
                                    row_gap: Val::Px(10.0),
                                    ..default()
                                },
                                Transform::default(),
                                GlobalTransform::default(),
                            ))
                            .with_children(|list| {
                                for character in &character_selection.characters {
                                    let (button_bg, button_border, button_shadow) = button_surface(
                                        theme,
                                        UiButtonVariant::Secondary,
                                        UiInteractionState::Idle,
                                        glow_intensity,
                                    );
                                    list.spawn((
                                        Button,
                                        CharacterSelectButton {
                                            player_entity_id: character.player_entity_id.clone(),
                                        },
                                        layout::leading_button(
                                            Val::Percent(100.0),
                                            58.0,
                                            theme.metrics.input_radius_px,
                                            theme.metrics.control_border_px,
                                            12.0,
                                        ),
                                        Transform::default(),
                                        GlobalTransform::default(),
                                        button_bg,
                                        button_border,
                                        button_shadow,
                                    ))
                                    .with_children(|button| {
                                        button
                                            .spawn((
                                                Node {
                                                    width: Val::Percent(100.0),
                                                    flex_direction: FlexDirection::Column,
                                                    row_gap: Val::Px(2.0),
                                                    ..default()
                                                },
                                                Transform::default(),
                                                GlobalTransform::default(),
                                            ))
                                            .with_children(|text_stack| {
                                                text_stack.spawn((
                                                    Text::new(character_display_name(character)),
                                                    text_font(fonts.mono_bold.clone(), 17.0),
                                                    TextColor(
                                                        theme.colors.panel_foreground_color(),
                                                    ),
                                                ));
                                                text_stack.spawn((
                                                    Text::new(format!(
                                                        "{}  {}",
                                                        character.status.to_ascii_uppercase(),
                                                        short_player_id(&character.player_entity_id)
                                                    )),
                                                    text_font(fonts.mono.clone(), 11.0),
                                                    TextColor(
                                                        theme.colors.muted_foreground_color(),
                                                    ),
                                                ));
                                            });
                                    });
                                }
                            });
                    });

                let (enter_bg, enter_border, enter_shadow) = button_surface(
                    theme,
                    UiButtonVariant::Primary,
                    UiInteractionState::Idle,
                    glow_intensity,
                );
                panel
                    .spawn((
                        Button,
                        CharacterSelectEnterButton,
                        layout::button(
                            Val::Percent(100.0),
                            46.0,
                            theme.metrics.input_radius_px,
                            theme.metrics.control_border_px,
                        ),
                        Transform::default(),
                        GlobalTransform::default(),
                        enter_bg,
                        enter_border,
                        enter_shadow,
                    ))
                    .with_children(|button| {
                        button.spawn((
                            Text::new("ENTER WORLD"),
                            text_font(fonts.mono_bold.clone(), 18.0),
                            TextColor(theme.colors.primary_foreground_color()),
                        ));
                    });

                panel.spawn((
                    Text::new(""),
                    text_font(fonts.mono.clone(), 12.0),
                    TextColor(theme.colors.muted_foreground_color()),
                    CharacterSelectStatusText,
                ));
            });
        });
}

fn character_display_name(character: &super::app_state::CharacterSelectionEntry) -> &str {
    let trimmed = character.display_name.trim();
    if trimmed.is_empty() {
        "Unnamed Character"
    } else {
        trimmed
    }
}

fn short_player_id(player_entity_id: &str) -> String {
    let trimmed = player_entity_id.trim();
    if trimmed.len() <= 12 {
        return trimmed.to_ascii_uppercase();
    }
    format!(
        "{}...{}",
        &trimmed[..8].to_ascii_uppercase(),
        &trimmed[trimmed.len() - 4..].to_ascii_uppercase()
    )
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(super) fn handle_character_select_buttons(
    app_state: Option<Res<'_, State<ClientAppState>>>,
    mut interactions: Query<
        '_,
        '_,
        (
            &Interaction,
            Option<&CharacterSelectButton>,
            Option<&CharacterSelectEnterButton>,
        ),
        Changed<Interaction>,
    >,
    mut session: ResMut<'_, ClientSession>,
    mut character_selection: ResMut<'_, CharacterSelectionState>,
    mut request_state: ResMut<'_, auth_net::GatewayRequestState>,
    gateway_http: Res<'_, super::resources::GatewayHttpAdapter>,
    mut status_texts: Query<
        '_,
        '_,
        &mut Text,
        (
            With<CharacterSelectStatusText>,
            Without<CharacterSelectPreviewNameText>,
            Without<CharacterSelectPreviewMetaText>,
        ),
    >,
    mut preview_name_texts: Query<
        '_,
        '_,
        &mut Text,
        (
            With<CharacterSelectPreviewNameText>,
            Without<CharacterSelectStatusText>,
            Without<CharacterSelectPreviewMetaText>,
        ),
    >,
    mut preview_meta_texts: Query<
        '_,
        '_,
        &mut Text,
        (
            With<CharacterSelectPreviewMetaText>,
            Without<CharacterSelectStatusText>,
            Without<CharacterSelectPreviewNameText>,
        ),
    >,
) {
    if !app_state
        .as_ref()
        .is_some_and(|state| **state == ClientAppState::CharacterSelect)
    {
        return;
    }
    for (interaction, select_button, enter_button) in &mut interactions {
        match *interaction {
            Interaction::Pressed => {
                if let Some(select_button) = select_button {
                    character_selection.selected_player_entity_id =
                        Some(select_button.player_entity_id.clone());
                } else if enter_button.is_some() {
                    let Some(selected_player_entity_id) =
                        character_selection.selected_player_entity_id.clone()
                    else {
                        session.status = "No character selected.".to_string();
                        continue;
                    };
                    auth_net::submit_enter_world_request(
                        &mut session,
                        request_state.as_mut(),
                        *gateway_http,
                        selected_player_entity_id,
                    );
                }
            }
            Interaction::Hovered | Interaction::None => {}
        }
    }
    for mut text in &mut status_texts {
        text.0 = session.status.clone();
    }
    let selected = character_selection.selected_character();
    for mut text in &mut preview_name_texts {
        text.0 = selected
            .map(character_display_name)
            .unwrap_or("No Character")
            .to_string();
    }
    for mut text in &mut preview_meta_texts {
        text.0 = selected
            .map(|character| {
                format!(
                    "{} / {}",
                    character.status.to_ascii_uppercase(),
                    short_player_id(&character.player_entity_id)
                )
            })
            .unwrap_or_else(|| {
                "Create a character from the dashboard or account site.".to_string()
            });
    }
}

#[allow(clippy::type_complexity)]
pub(super) fn sync_character_select_button_visuals(
    active_theme: Res<'_, ActiveUiTheme>,
    visual_settings: Res<'_, UiVisualSettings>,
    character_selection: Res<'_, CharacterSelectionState>,
    mut interactions: Query<
        '_,
        '_,
        (
            &Interaction,
            Option<&CharacterSelectButton>,
            Option<&CharacterSelectEnterButton>,
            &mut BackgroundColor,
            &mut BorderColor,
            &mut BoxShadow,
        ),
    >,
) {
    let theme = theme_definition(active_theme.0);
    let glow_intensity = visual_settings.glow_intensity();
    for (interaction, select_button, enter_button, mut bg, mut border, mut shadow) in
        &mut interactions
    {
        let variant = if enter_button.is_some() {
            UiButtonVariant::Primary
        } else {
            UiButtonVariant::Secondary
        };
        let state = match *interaction {
            Interaction::Pressed => UiInteractionState::Pressed,
            Interaction::Hovered => UiInteractionState::Hovered,
            Interaction::None => {
                if select_button.is_some_and(|button| {
                    character_selection
                        .selected_player_entity_id
                        .as_ref()
                        .is_some_and(|selected| selected == &button.player_entity_id)
                }) {
                    UiInteractionState::Selected
                } else {
                    UiInteractionState::Idle
                }
            }
        };
        let (next_bg, next_border, next_shadow) =
            button_surface(theme, variant, state, glow_intensity);
        *bg = next_bg;
        *border = next_border;
        *shadow = next_shadow;
    }
}
