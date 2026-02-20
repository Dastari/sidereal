use bevy::prelude::*;

/// Dialog UI System for client-side error/info/warning modals
///
/// # Overview
///
/// Provides persistent modal dialogs for error handling and user notifications.
/// Matches the Sidereal auth screen aesthetic with space-themed styling.
///
/// # Usage
///
/// ```rust
/// use crate::dialog_ui::DialogQueue;
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
    fonts: Res<crate::EmbeddedFonts>,
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

    dialog_queue.current = Some(dialog.clone());

    let font_bold = fonts.bold.clone();
    let font_regular = fonts.regular.clone();

    // Get severity-specific colors
    let (title_color, border_color) = match dialog.severity {
        DialogSeverity::Info => (Color::srgb(0.6, 0.8, 1.0), Color::srgba(0.3, 0.5, 0.7, 0.8)),
        DialogSeverity::Warning => (Color::srgb(1.0, 0.8, 0.3), Color::srgba(0.8, 0.6, 0.2, 0.8)),
        DialogSeverity::Error => (
            Color::srgb(1.0, 0.4, 0.35),
            Color::srgba(0.8, 0.2, 0.2, 0.8),
        ),
    };

    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            DialogRoot,
            ZIndex(1000),
        ))
        .with_children(|root| {
            // Semi-transparent backdrop
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)),
                DialogBackdrop,
            ));

            // Dialog panel
            root.spawn((
                Node {
                    width: Val::Px(600.0),
                    max_width: Val::Percent(90.0),
                    padding: UiRect::all(Val::Px(28.0)),
                    border: UiRect::all(Val::Px(2.0)),
                    border_radius: BorderRadius::all(Val::Px(12.0)),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(18.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.06, 0.08, 0.12, 0.96)),
                BorderColor::all(border_color),
            ))
            .with_children(|panel| {
                // Title
                panel.spawn((
                    Text::new(&dialog.title),
                    TextFont {
                        font: font_bold.clone(),
                        font_size: 28.0,
                        ..default()
                    },
                    TextColor(title_color),
                ));

                // Message body
                panel.spawn((
                    Text::new(&dialog.message),
                    TextFont {
                        font: font_regular.clone(),
                        font_size: 16.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.85, 0.9, 0.95)),
                    Node {
                        max_width: Val::Percent(100.0),
                        ..default()
                    },
                ));

                // Okay button
                panel
                    .spawn((
                        Button,
                        Node {
                            width: Val::Px(120.0),
                            height: Val::Px(44.0),
                            margin: UiRect::top(Val::Px(8.0)),
                            align_self: AlignSelf::FlexEnd,
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            border: UiRect::all(Val::Px(1.0)),
                            border_radius: BorderRadius::all(Val::Px(6.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.15, 0.2, 0.3, 0.9)),
                        BorderColor::all(Color::srgba(0.3, 0.4, 0.55, 0.9)),
                        DialogOkayButton,
                    ))
                    .with_children(|button| {
                        button.spawn((
                            Text::new("OKAY"),
                            TextFont {
                                font: font_bold.clone(),
                                font_size: 16.0,
                                ..default()
                            },
                            TextColor(Color::srgb(0.85, 0.92, 1.0)),
                        ));
                    });
            });
        });
}

#[allow(clippy::type_complexity)]
fn handle_dialog_interactions(
    mut commands: Commands,
    mut dialog_queue: ResMut<DialogQueue>,
    mut interaction_query: Query<
        (&Interaction, &mut BackgroundColor, &mut BorderColor),
        (Changed<Interaction>, With<DialogOkayButton>),
    >,
    dialog_root: Query<Entity, With<DialogRoot>>,
    keyboard: Res<ButtonInput<KeyCode>>,
) {
    // Handle button interaction
    for (interaction, mut bg_color, mut border_color) in &mut interaction_query {
        match *interaction {
            Interaction::Pressed => {
                // Clear current dialog and despawn UI
                dialog_queue.current = None;
                for entity in &dialog_root {
                    commands.entity(entity).despawn();
                }
            }
            Interaction::Hovered => {
                *bg_color = BackgroundColor(Color::srgba(0.2, 0.25, 0.35, 0.9));
                *border_color = BorderColor::all(Color::srgba(0.4, 0.5, 0.65, 1.0));
            }
            Interaction::None => {
                *bg_color = BackgroundColor(Color::srgba(0.15, 0.2, 0.3, 0.9));
                *border_color = BorderColor::all(Color::srgba(0.3, 0.4, 0.55, 0.9));
            }
        }
    }

    // Also allow Enter or Escape to dismiss
    if !dialog_root.is_empty()
        && (keyboard.just_pressed(KeyCode::Enter) || keyboard.just_pressed(KeyCode::Escape))
    {
        dialog_queue.current = None;
        for entity in &dialog_root {
            commands.entity(entity).despawn();
        }
    }
}
