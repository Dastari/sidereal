//! Scene setup systems for UI camera and character select.

use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use bevy::state::state_scoped::DespawnOnExit;

use super::app_state::{CharacterSelectionState, ClientAppState, ClientSession};
use super::auth_net;
use super::components::{
    CharacterSelectButton, CharacterSelectEnterButton, CharacterSelectRoot,
    CharacterSelectStatusText, UiOverlayCamera,
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
    static BOLD: &[u8] = include_bytes!("../../../../data/fonts/FiraSans-Bold.ttf");
    static REGULAR: &[u8] = include_bytes!("../../../../data/fonts/FiraSans-Regular.ttf");

    let mut fonts = app.world_mut().resource_mut::<Assets<Font>>();
    let bold = fonts
        .add(Font::try_from_bytes(BOLD.to_vec()).expect("embedded FiraSans-Bold.ttf is valid"));
    let regular = fonts.add(
        Font::try_from_bytes(REGULAR.to_vec()).expect("embedded FiraSans-Regular.ttf is valid"),
    );
    app.insert_resource(EmbeddedFonts { bold, regular });
}

pub(super) fn setup_character_select_screen(
    mut commands: Commands<'_, '_>,
    fonts: Res<'_, EmbeddedFonts>,
    character_selection: Res<'_, CharacterSelectionState>,
) {
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            Transform::default(),
            GlobalTransform::default(),
            CharacterSelectRoot,
            DespawnOnExit(ClientAppState::CharacterSelect),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: Val::Px(560.0),
                    padding: UiRect::all(Val::Px(24.0)),
                    border: UiRect::all(Val::Px(2.0)),
                    border_radius: BorderRadius::all(Val::Px(12.0)),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(12.0),
                    ..default()
                },
                Transform::default(),
                GlobalTransform::default(),
                BackgroundColor(Color::srgba(0.06, 0.08, 0.12, 0.95)),
                BorderColor::all(Color::srgba(0.2, 0.3, 0.45, 0.8)),
            ))
            .with_children(|panel| {
                panel.spawn((
                    Text::new("Character Select"),
                    TextFont {
                        font: fonts.bold.clone(),
                        font_size: 34.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.85, 0.92, 1.0)),
                ));
                panel.spawn((
                    Text::new("Choose a character, then Enter World."),
                    TextFont {
                        font: fonts.regular.clone(),
                        font_size: 15.0,
                        ..default()
                    },
                    TextColor(Color::srgba(0.78, 0.84, 0.92, 0.95)),
                ));

                for player_entity_id in &character_selection.characters {
                    panel
                        .spawn((
                            Button,
                            CharacterSelectButton {
                                player_entity_id: player_entity_id.clone(),
                            },
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Px(38.0),
                                justify_content: JustifyContent::FlexStart,
                                align_items: AlignItems::Center,
                                padding: UiRect::axes(Val::Px(10.0), Val::Px(0.0)),
                                border_radius: BorderRadius::all(Val::Px(7.0)),
                                ..default()
                            },
                            Transform::default(),
                            GlobalTransform::default(),
                            BackgroundColor(Color::srgba(0.14, 0.18, 0.24, 0.9)),
                        ))
                        .with_children(|button| {
                            button.spawn((
                                Text::new(player_entity_id.clone()),
                                TextFont {
                                    font: fonts.regular.clone(),
                                    font_size: 14.0,
                                    ..default()
                                },
                                TextColor(Color::srgb(0.92, 0.95, 1.0)),
                            ));
                        });
                }

                panel
                    .spawn((
                        Button,
                        CharacterSelectEnterButton,
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Px(44.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            border_radius: BorderRadius::all(Val::Px(8.0)),
                            ..default()
                        },
                        Transform::default(),
                        GlobalTransform::default(),
                        BackgroundColor(Color::srgb(0.2, 0.46, 0.85)),
                    ))
                    .with_children(|button| {
                        button.spawn((
                            Text::new("Enter World"),
                            TextFont {
                                font: fonts.bold.clone(),
                                font_size: 17.0,
                                ..default()
                            },
                            TextColor(Color::WHITE),
                        ));
                    });

                panel.spawn((
                    Text::new(""),
                    TextFont {
                        font: fonts.regular.clone(),
                        font_size: 13.0,
                        ..default()
                    },
                    TextColor(Color::srgba(0.75, 0.83, 0.9, 0.95)),
                    CharacterSelectStatusText,
                ));
            });
        });
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
            &mut BackgroundColor,
        ),
        Changed<Interaction>,
    >,
    mut session: ResMut<'_, ClientSession>,
    mut character_selection: ResMut<'_, CharacterSelectionState>,
    mut request_state: ResMut<'_, auth_net::GatewayRequestState>,
    gateway_http: Res<'_, super::resources::GatewayHttpAdapter>,
    mut status_texts: Query<'_, '_, &mut Text, With<CharacterSelectStatusText>>,
) {
    if !app_state
        .as_ref()
        .is_some_and(|state| **state == ClientAppState::CharacterSelect)
    {
        return;
    }
    for (interaction, select_button, enter_button, mut bg) in &mut interactions {
        match *interaction {
            Interaction::Pressed => {
                if let Some(select_button) = select_button {
                    character_selection.selected_player_entity_id =
                        Some(select_button.player_entity_id.clone());
                    *bg = BackgroundColor(Color::srgba(0.22, 0.3, 0.42, 0.98));
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
                    *bg = BackgroundColor(Color::srgb(0.16, 0.38, 0.74));
                }
            }
            Interaction::Hovered => {
                if enter_button.is_some() {
                    *bg = BackgroundColor(Color::srgb(0.24, 0.5, 0.9));
                } else {
                    *bg = BackgroundColor(Color::srgba(0.18, 0.24, 0.33, 0.95));
                }
            }
            Interaction::None => {
                if enter_button.is_some() {
                    *bg = BackgroundColor(Color::srgb(0.2, 0.46, 0.85));
                } else {
                    *bg = BackgroundColor(Color::srgba(0.14, 0.18, 0.24, 0.9));
                }
            }
        }
    }
    for mut text in &mut status_texts {
        text.0 = session.status.clone();
    }
}
