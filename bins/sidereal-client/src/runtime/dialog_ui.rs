use super::ecs_util::queue_despawn_if_exists;
use bevy::log::info;
use bevy::prelude::*;
use sidereal_ui::layout;
use sidereal_ui::theme::{ActiveUiTheme, UiVisualSettings, theme_definition};
use sidereal_ui::typography::text_font;
use sidereal_ui::widgets::{
    UiButtonVariant, UiInteractionState, button_surface, panel_surface, spawn_hud_frame_chrome,
};

/// Dialog UI System for client-side error/info/warning modals
///
/// # Overview
///
/// Provides persistent modal dialogs for error handling and user notifications.
/// Matches the Sidereal auth screen aesthetic with space-themed styling.
///
/// # Usage
///
/// ```ignore
/// use bevy::prelude::ResMut;
/// use sidereal_client::runtime::dialog_ui::DialogQueue;
///
/// fn my_system(mut dialog_queue: ResMut<DialogQueue>) {
///     // Show error dialog (red theme)
///     dialog_queue.push_error(
///         "Operation Failed",
///         "Detailed error message with context.\n\nTroubleshooting hints."
///     );
///
///     // Show warning dialog (yellow/orange theme)
///     dialog_queue.push_warning(
///         "Caution Required",
///         "Something needs attention but isn't blocking."
///     );
///
///     // Show info dialog (blue theme)
///     dialog_queue.push_info(
///         "Success",
///         "Operation completed successfully."
///     );
/// }
/// ```
///
/// # Behavior
///
/// - Dialogs queue if multiple are pushed (shown one at a time)
/// - Dismissal: Click OKAY button, press Enter, or press Escape
/// - Backdrop click does NOT dismiss (requires explicit acknowledgment)
/// - Dialogs persist until explicitly dismissed (no auto-hide)
///
/// # Design
///
/// See `docs/ui_design_guide.md` for full design specifications including:
/// - Color palette and severity theming
/// - Spacing and layout measurements
/// - Typography and font sizes
/// - Component hierarchy and z-index layering
///
/// # Registration
///
/// Call `register_dialog_ui(&mut app)` during app setup to add systems.

#[derive(Component)]
struct DialogRoot;

#[derive(Component)]
struct DialogBackdrop;

#[derive(Component)]
struct DialogOkayButton;

#[derive(Resource, Default)]
pub struct DialogQueue {
    pending: Vec<DialogMessage>,
    current: Option<DialogMessage>,
}

#[derive(Debug, Clone)]
pub struct DialogMessage {
    pub title: String,
    pub message: String,
    pub severity: DialogSeverity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum DialogSeverity {
    Info,
    Warning,
    Error,
}

#[allow(dead_code)]
impl DialogQueue {
    pub fn push_error(&mut self, title: impl Into<String>, message: impl Into<String>) {
        self.pending.push(DialogMessage {
            title: title.into(),
            message: message.into(),
            severity: DialogSeverity::Error,
        });
    }

    pub fn push_warning(&mut self, title: impl Into<String>, message: impl Into<String>) {
        self.pending.push(DialogMessage {
            title: title.into(),
            message: message.into(),
            severity: DialogSeverity::Warning,
        });
    }

    pub fn push_info(&mut self, title: impl Into<String>, message: impl Into<String>) {
        self.pending.push(DialogMessage {
            title: title.into(),
            message: message.into(),
            severity: DialogSeverity::Info,
        });
    }

    fn next_dialog(&mut self) -> Option<DialogMessage> {
        if self.pending.is_empty() {
            None
        } else {
            Some(self.pending.remove(0))
        }
    }
}

pub fn register_dialog_ui(app: &mut App) {
    app.init_resource::<DialogQueue>();
    app.add_systems(Update, (show_next_dialog, handle_dialog_interactions));
}

fn show_next_dialog(
    mut commands: Commands,
    mut dialog_queue: ResMut<DialogQueue>,
    fonts: Res<super::EmbeddedFonts>,
    active_theme: Res<ActiveUiTheme>,
    visual_settings: Res<UiVisualSettings>,
    existing: Query<Entity, With<DialogRoot>>,
) {
    if !existing.is_empty() {
        return;
    }

    let dialog = match dialog_queue.current.take() {
        Some(d) => Some(d),
        None => dialog_queue.next_dialog(),
    };

    let Some(dialog) = dialog else {
        return;
    };

    info!(
        "client dialog shown title='{}' severity={:?}",
        dialog.title, dialog.severity
    );
    dialog_queue.current = Some(dialog.clone());
    let theme = theme_definition(active_theme.0);
    let glow_intensity = visual_settings.glow_intensity();
    let (panel_bg, _, panel_shadow) = panel_surface(theme, glow_intensity);
    let (button_bg, button_border, button_shadow) = button_surface(
        theme,
        UiButtonVariant::Outline,
        UiInteractionState::Idle,
        glow_intensity,
    );

    let (title_color, border_color) = match dialog.severity {
        DialogSeverity::Info => (Color::srgb(0.6, 0.8, 1.0), Color::srgba(0.3, 0.5, 0.7, 0.8)),
        DialogSeverity::Warning => (Color::srgb(1.0, 0.8, 0.3), Color::srgba(0.8, 0.6, 0.2, 0.8)),
        DialogSeverity::Error => (
            Color::srgb(1.0, 0.4, 0.35),
            Color::srgba(0.8, 0.2, 0.2, 0.8),
        ),
    };

    commands
        .spawn((layout::fullscreen_centered_root(), DialogRoot, ZIndex(1000)))
        .with_children(|root| {
            root.spawn((
                layout::fullscreen_backdrop(),
                BackgroundColor(theme.colors.overlay_color()),
                DialogBackdrop,
            ));

            root.spawn((
                Node {
                    max_width: Val::Percent(90.0),
                    ..layout::panel(
                        Val::Px(600.0),
                        theme.metrics.panel_padding_px,
                        18.0,
                        theme.metrics.panel_radius_px,
                        theme.metrics.panel_border_px,
                    )
                },
                panel_bg,
                panel_shadow,
                BorderColor::all(border_color),
            ))
            .with_children(|panel| {
                spawn_hud_frame_chrome(
                    panel,
                    theme,
                    Some(severity_label(dialog.severity)),
                    &fonts.mono,
                    glow_intensity,
                );

                panel.spawn((
                    Text::new(&dialog.title),
                    text_font(fonts.display.clone(), 28.0),
                    TextColor(title_color),
                ));

                panel.spawn((
                    Text::new(&dialog.message),
                    text_font(fonts.regular.clone(), 16.0),
                    TextColor(theme.colors.foreground_color()),
                    Node {
                        max_width: Val::Percent(100.0),
                        ..default()
                    },
                ));

                panel
                    .spawn((
                        Button,
                        DialogOkayButton,
                        Node {
                            margin: UiRect::top(Val::Px(8.0)),
                            align_self: AlignSelf::FlexEnd,
                            ..layout::button(
                                Val::Px(120.0),
                                44.0,
                                theme.metrics.control_radius_px,
                                theme.metrics.control_border_px,
                            )
                        },
                        button_bg,
                        button_border,
                        button_shadow,
                    ))
                    .with_children(|button| {
                        button.spawn((
                            Text::new("OKAY"),
                            text_font(fonts.mono_bold.clone(), 18.0),
                            TextColor(theme.colors.panel_foreground_color()),
                        ));
                    });
            });
        });
}

#[allow(clippy::type_complexity)]
fn handle_dialog_interactions(
    mut commands: Commands,
    mut dialog_queue: ResMut<DialogQueue>,
    active_theme: Res<ActiveUiTheme>,
    visual_settings: Res<UiVisualSettings>,
    mut interaction_query: Query<
        (
            &Interaction,
            &mut BackgroundColor,
            &mut BorderColor,
            &mut BoxShadow,
        ),
        (Changed<Interaction>, With<DialogOkayButton>),
    >,
    dialog_root: Query<Entity, With<DialogRoot>>,
    keyboard: Res<ButtonInput<KeyCode>>,
) {
    let theme = theme_definition(active_theme.0);
    let glow_intensity = visual_settings.glow_intensity();
    for (interaction, mut bg_color, mut border_color, mut shadow) in &mut interaction_query {
        match *interaction {
            Interaction::Pressed => {
                info!("client dialog dismissed via button");
                dialog_queue.current = None;
                for entity in &dialog_root {
                    queue_despawn_if_exists(&mut commands, entity);
                }
            }
            Interaction::Hovered | Interaction::None => {}
        }
        let state = match *interaction {
            Interaction::Pressed => UiInteractionState::Pressed,
            Interaction::Hovered => UiInteractionState::Hovered,
            Interaction::None => UiInteractionState::Idle,
        };
        let (next_bg, next_border, next_shadow) =
            button_surface(theme, UiButtonVariant::Outline, state, glow_intensity);
        *bg_color = next_bg;
        *border_color = next_border;
        *shadow = next_shadow;
    }

    if !dialog_root.is_empty()
        && (keyboard.just_pressed(KeyCode::Enter) || keyboard.just_pressed(KeyCode::Escape))
    {
        info!("client dialog dismissed via keyboard");
        dialog_queue.current = None;
        for entity in &dialog_root {
            queue_despawn_if_exists(&mut commands, entity);
        }
    }
}

fn severity_label(severity: DialogSeverity) -> &'static str {
    match severity {
        DialogSeverity::Info => "Info",
        DialogSeverity::Warning => "Warning",
        DialogSeverity::Error => "Error",
    }
}
